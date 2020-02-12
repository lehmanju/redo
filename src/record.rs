//! A record of commands.

use crate::{format::Format, At, Command, Entry, History, Merge, Result, Signal, Slot};
use alloc::{
    collections::VecDeque,
    string::{String, ToString},
    vec::Vec,
};
use core::{
    fmt::{self, Write},
    num::NonZeroUsize,
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "chrono")]
use {
    chrono::{DateTime, TimeZone, Utc},
    core::cmp::Ordering,
};

/// A record of commands.
///
/// The record can roll the targets state backwards and forwards by using
/// the undo and redo methods. In addition, the record can notify the user
/// about changes to the stack or the target through [signal]. The user
/// can give the record a function that is called each time the state changes
/// by using the [`builder`].
///
/// # Examples
/// ```
/// # use redo::{Command, Record};
/// # struct Add(char);
/// # impl Command for Add {
/// #     type Target = String;
/// #     type Error = &'static str;
/// #     fn apply(&mut self, s: &mut String) -> redo::Result<Add> {
/// #         s.push(self.0);
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self, s: &mut String) -> redo::Result<Add> {
/// #         self.0 = s.pop().ok_or("s is empty")?;
/// #         Ok(())
/// #     }
/// # }
/// # fn main() -> redo::Result<Add> {
/// let mut record = Record::default();
/// record.apply(Add('a'))?;
/// record.apply(Add('b'))?;
/// record.apply(Add('c'))?;
/// assert_eq!(record.target(), "abc");
/// record.undo()?;
/// record.undo()?;
/// record.undo()?;
/// assert_eq!(record.target(), "");
/// record.redo()?;
/// record.redo()?;
/// record.redo()?;
/// assert_eq!(record.target(), "abc");
/// # Ok(())
/// # }
/// ```
///
/// [`builder`]: struct.RecordBuilder.html
/// [signal]: enum.Signal.html
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(bound(
        serialize = "C: Command + Serialize, C::Target: Serialize",
        deserialize = "C: Command + Deserialize<'de>, C::Target: Deserialize<'de>"
    ))
)]
pub struct Record<C: Command, F = fn(Signal)> {
    pub(crate) entries: VecDeque<Entry<C>>,
    target: C::Target,
    current: usize,
    limit: NonZeroUsize,
    pub(crate) saved: Option<usize>,
    pub(crate) slot: Slot<F>,
}

impl<C: Command> Record<C> {
    /// Returns a new record.
    pub fn new(target: C::Target) -> Record<C> {
        Builder::new().build(target)
    }
}

impl<C: Command, F: FnMut(Signal)> Record<C, F> {
    /// Reserves capacity for at least `additional` more commands.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    pub fn reserve(&mut self, additional: usize) {
        self.entries.reserve(additional);
    }

    /// Returns the capacity of the record.
    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }

    /// Shrinks the capacity of the record as much as possible.
    pub fn shrink_to_fit(&mut self) {
        self.entries.shrink_to_fit();
    }

    /// Returns the number of commands in the record.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the record is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the limit of the record.
    pub fn limit(&self) -> usize {
        self.limit.get()
    }

    /// Sets how the signal should be handled when the state changes.
    ///
    /// The previous slot is returned if it exists.
    pub fn connect(&mut self, slot: F) -> Option<F> {
        self.slot.f.replace(slot)
    }

    /// Removes and returns the slot.
    pub fn disconnect(&mut self) -> Option<F> {
        self.slot.f.take()
    }

    /// Returns `true` if the record can undo.
    pub fn can_undo(&self) -> bool {
        self.current() > 0
    }

    /// Returns `true` if the record can redo.
    pub fn can_redo(&self) -> bool {
        self.current() < self.len()
    }

    /// Returns `true` if the target is in a saved state, `false` otherwise.
    pub fn is_saved(&self) -> bool {
        self.saved.map_or(false, |saved| saved == self.current())
    }

    /// Marks the target as currently being in a saved or unsaved state.
    pub fn set_saved(&mut self, saved: bool) {
        let was_saved = self.is_saved();
        if saved {
            self.saved = Some(self.current());
            self.slot.emit_if(!was_saved, Signal::Saved(true));
        } else {
            self.saved = None;
            self.slot.emit_if(was_saved, Signal::Saved(false));
        }
    }

    /// Revert the changes done to the target since the saved state.
    pub fn revert(&mut self) -> Option<Result<C>> {
        self.saved.and_then(|saved| self.go_to(saved))
    }

    /// Returns the position of the current command.
    pub fn current(&self) -> usize {
        self.current
    }

    /// Removes all commands from the record without undoing them.
    pub fn clear(&mut self) {
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        self.entries.clear();
        self.saved = if self.is_saved() { Some(0) } else { None };
        self.current = 0;
        self.slot.emit_if(could_undo, Signal::Undo(false));
        self.slot.emit_if(could_redo, Signal::Redo(false));
    }

    /// Pushes the command on top of the record and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    pub fn apply(&mut self, command: C) -> Result<C> {
        self.__apply(command).map(|_| ())
    }

    pub(crate) fn __apply(
        &mut self,
        mut command: C,
    ) -> core::result::Result<(bool, VecDeque<Entry<C>>), C::Error> {
        command.apply(&mut self.target)?;
        let current = self.current();
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        // Pop off all elements after len from record.
        let tail = self.entries.split_off(current);
        debug_assert_eq!(current, self.len());
        // Check if the saved state was popped off.
        self.saved = self.saved.filter(|&saved| saved <= current);
        // Try to merge commands unless the target is in a saved state.
        let merged = match self.entries.back_mut() {
            Some(ref mut last) if !was_saved => last.command.merge(command),
            _ => Merge::No(command),
        };
        let merged_or_annulled = match merged {
            Merge::Yes => true,
            Merge::Annul => {
                self.entries.pop_back();
                true
            }
            // If commands are not merged or annulled push it onto the record.
            Merge::No(command) => {
                // If limit is reached, pop off the first command.
                if self.limit() == self.current() {
                    self.entries.pop_front();
                    self.saved = self.saved.and_then(|saved| saved.checked_sub(1));
                } else {
                    self.current += 1;
                }
                self.entries.push_back(Entry::from(command));
                false
            }
        };
        debug_assert_eq!(self.current(), self.len());
        self.slot.emit_if(could_redo, Signal::Redo(false));
        self.slot.emit_if(!could_undo, Signal::Undo(true));
        self.slot.emit_if(was_saved, Signal::Saved(false));
        Ok((merged_or_annulled, tail))
    }

    /// Calls the [`undo`] method for the active command and sets
    /// the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned.
    ///
    /// [`undo`]: ../trait.Command.html#tymethod.undo
    pub fn undo(&mut self) -> Result<C> {
        if !self.can_undo() {
            return Ok(());
        }
        let was_saved = self.is_saved();
        let old = self.current();
        self.entries[self.current - 1].undo(&mut self.target)?;
        self.current -= 1;
        let len = self.len();
        let is_saved = self.is_saved();
        self.slot.emit_if(old == len, Signal::Redo(true));
        self.slot.emit_if(old == 1, Signal::Undo(false));
        self.slot
            .emit_if(was_saved != is_saved, Signal::Saved(is_saved));
        Ok(())
    }

    /// Calls the [`redo`] method for the active command and sets
    /// the next one as the new active one.
    ///
    /// # Errors
    /// If an error occur when applying [`redo`] the error is returned.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    pub fn redo(&mut self) -> Result<C> {
        if !self.can_redo() {
            return Ok(());
        }
        let was_saved = self.is_saved();
        let old = self.current();
        self.entries[self.current].redo(&mut self.target)?;
        self.current += 1;
        let len = self.len();
        let is_saved = self.is_saved();
        self.slot.emit_if(old == len - 1, Signal::Redo(false));
        self.slot.emit_if(old == 0, Signal::Undo(true));
        self.slot
            .emit_if(was_saved != is_saved, Signal::Saved(is_saved));
        Ok(())
    }

    /// Repeatedly calls [`undo`] or [`redo`] until the command at `current` is reached.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    /// [`redo`]: trait.Command.html#method.redo
    pub fn go_to(&mut self, current: usize) -> Option<Result<C>> {
        if current > self.len() {
            return None;
        }
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        // Temporarily remove slot so they are not called each iteration.
        let f = self.slot.f.take();
        // Decide if we need to undo or redo to reach current.
        let apply = if current > self.current() {
            Record::redo
        } else {
            Record::undo
        };
        while self.current() != current {
            if let Err(err) = apply(self) {
                self.slot.f = f;
                return Some(Err(err));
            }
        }
        // Add slot back.
        self.slot.f = f;
        let can_undo = self.can_undo();
        let can_redo = self.can_redo();
        let is_saved = self.is_saved();
        self.slot
            .emit_if(could_undo != can_undo, Signal::Undo(can_undo));
        self.slot
            .emit_if(could_redo != can_redo, Signal::Redo(can_redo));
        self.slot
            .emit_if(was_saved != is_saved, Signal::Saved(is_saved));
        Some(Ok(()))
    }

    /// Go back or forward in the record to the command that was made closest to the datetime provided.
    #[cfg(feature = "chrono")]
    pub fn time_travel(&mut self, to: &DateTime<impl TimeZone>) -> Option<Result<C>> {
        let to = to.with_timezone(&Utc);
        let current = match self.entries.as_slices() {
            ([], []) => return None,
            (start, []) => match start.binary_search_by(|entry| entry.timestamp.cmp(&to)) {
                Ok(current) | Err(current) => current,
            },
            ([], end) => match end.binary_search_by(|entry| entry.timestamp.cmp(&to)) {
                Ok(current) | Err(current) => current,
            },
            (start, end) => match start.last().unwrap().timestamp.cmp(&to) {
                Ordering::Less => match start.binary_search_by(|entry| entry.timestamp.cmp(&to)) {
                    Ok(current) | Err(current) => current,
                },
                Ordering::Equal => start.len(),
                Ordering::Greater => match end.binary_search_by(|entry| entry.timestamp.cmp(&to)) {
                    Ok(current) | Err(current) => start.len() + current,
                },
            },
        };
        self.go_to(current)
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<C, F> {
        Queue {
            record: self,
            commands: Vec::new(),
        }
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<C, F> {
        Checkpoint {
            record: self,
            commands: Vec::new(),
        }
    }

    /// Returns a reference to the `target`.
    pub fn target(&self) -> &C::Target {
        &self.target
    }

    /// Returns a mutable reference to the `target`.
    ///
    /// This method should **only** be used when doing changes that should not be able to be undone.
    pub fn target_mut(&mut self) -> &mut C::Target {
        &mut self.target
    }

    /// Consumes the record, returning the `target`.
    pub fn into_target(self) -> C::Target {
        self.target
    }
}

impl<C: Command + ToString, F: FnMut(Signal)> Record<C, F> {
    /// Returns the string of the command which will be undone in the next call to [`undo`].
    ///
    /// [`undo`]: struct.Record.html#method.undo
    pub fn undo_text(&self) -> Option<String> {
        if self.can_undo() {
            Some(self.entries[self.current - 1].command.to_string())
        } else {
            None
        }
    }

    /// Returns the string of the command which will be redone in the next call to [`redo`].
    ///
    /// [`redo`]: struct.Record.html#method.redo
    pub fn redo_text(&self) -> Option<String> {
        if self.can_redo() {
            Some(self.entries[self.current].command.to_string())
        } else {
            None
        }
    }

    /// Returns a structure for configurable formatting of the record.
    pub fn display(&self) -> Display<C, F> {
        Display {
            record: self,
            format: Format::default(),
        }
    }
}

impl<C: Command> Default for Record<C>
where
    C::Target: Default,
{
    fn default() -> Record<C> {
        Record::new(Default::default())
    }
}

impl<C: Command, F: FnMut(Signal)> From<History<C, F>> for Record<C, F> {
    fn from(history: History<C, F>) -> Record<C, F> {
        history.record
    }
}

impl<C: Command, F: FnMut(Signal)> fmt::Debug for Record<C, F>
where
    C: fmt::Debug,
    C::Target: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Record")
            .field("entries", &self.entries)
            .field("target", &self.target)
            .field("current", &self.current)
            .field("limit", &self.limit)
            .field("saved", &self.saved)
            .field("slot", &self.slot)
            .finish()
    }
}

/// Builder for a record.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct Builder {
    capacity: usize,
    limit: NonZeroUsize,
    saved: bool,
}

impl Builder {
    /// Returns a builder for a record.
    pub fn new() -> Builder {
        Builder {
            capacity: 0,
            limit: NonZeroUsize::new(usize::max_value()).unwrap(),
            saved: true,
        }
    }

    /// Sets the capacity for the record.
    pub fn capacity(&mut self, capacity: usize) -> &mut Builder {
        self.capacity = capacity;
        self
    }

    /// Sets the `limit` of the record.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    pub fn limit(&mut self, limit: usize) -> &mut Builder {
        self.limit = NonZeroUsize::new(limit).expect("limit can not be `0`");
        self
    }

    /// Sets if the target is initially in a saved state.
    /// By default the target is in a saved state.
    pub fn saved(&mut self, saved: bool) -> &mut Builder {
        self.saved = saved;
        self
    }

    /// Builds the record.
    pub fn build<C: Command>(&self, target: C::Target) -> Record<C> {
        Record {
            entries: VecDeque::with_capacity(self.capacity),
            target,
            current: 0,
            limit: self.limit,
            saved: if self.saved { Some(0) } else { None },
            slot: Slot::default(),
        }
    }

    /// Builds the record with the slot.
    pub fn build_with<C: Command, F: FnMut(Signal)>(
        &self,
        target: C::Target,
        slot: F,
    ) -> Record<C, F> {
        Record {
            entries: VecDeque::with_capacity(self.capacity),
            target,
            current: 0,
            limit: self.limit,
            saved: if self.saved { Some(0) } else { None },
            slot: Slot { f: Some(slot) },
        }
    }

    /// Creates the record with a default `target`.
    pub fn default<C: Command>(&self) -> Record<C>
    where
        C::Target: Default,
    {
        self.build(Default::default())
    }

    /// Creates the record with a default `target` and with the slot.
    pub fn default_with<C: Command, F: FnMut(Signal)>(&self, slot: F) -> Record<C, F>
    where
        C::Target: Default,
    {
        self.build_with(Default::default(), slot)
    }
}

impl Default for Builder {
    fn default() -> Self {
        Builder::new()
    }
}

#[derive(Debug)]
enum QueueCommand<C> {
    Apply(C),
    Undo,
    Redo,
}

/// Wraps a record and gives it batch queue functionality.
///
/// # Examples
/// ```
/// # use redo::{Command, Record};
/// # struct Add(char);
/// # impl Command for Add {
/// #     type Target = String;
/// #     type Error = &'static str;
/// #     fn apply(&mut self, s: &mut String) -> redo::Result<Add> {
/// #         s.push(self.0);
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self, s: &mut String) -> redo::Result<Add> {
/// #         self.0 = s.pop().ok_or("s is empty")?;
/// #         Ok(())
/// #     }
/// # }
/// # fn main() -> redo::Result<Add> {
/// let mut record = Record::default();
/// let mut queue = record.queue();
/// queue.apply(Add('a'));
/// queue.apply(Add('b'));
/// queue.apply(Add('c'));
/// assert_eq!(queue.target(), "");
/// queue.commit()?;
/// assert_eq!(record.target(), "abc");
/// # Ok(())
/// # }
/// ```
pub struct Queue<'a, C: Command, F> {
    record: &'a mut Record<C, F>,
    commands: Vec<QueueCommand<C>>,
}

impl<C: Command, F: FnMut(Signal)> Queue<'_, C, F> {
    /// Queues an `apply` action.
    pub fn apply(&mut self, command: C) {
        self.commands.push(QueueCommand::Apply(command));
    }

    /// Queues an `undo` action.
    pub fn undo(&mut self) {
        self.commands.push(QueueCommand::Undo);
    }

    /// Queues a `redo` action.
    pub fn redo(&mut self) {
        self.commands.push(QueueCommand::Redo);
    }

    /// Applies the queued commands.
    ///
    /// # Errors
    /// If an error occurs, it stops applying the commands and returns the error.
    pub fn commit(self) -> Result<C> {
        for command in self.commands {
            match command {
                QueueCommand::Apply(command) => self.record.apply(command)?,
                QueueCommand::Undo => self.record.undo()?,
                QueueCommand::Redo => self.record.redo()?,
            }
        }
        Ok(())
    }

    /// Cancels the queued actions.
    pub fn cancel(self) {}

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<C, F> {
        self.record.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<C, F> {
        self.record.checkpoint()
    }

    /// Returns a reference to the target.
    pub fn target(&self) -> &C::Target {
        self.record.target()
    }
}

#[derive(Debug)]
enum CheckpointCommand<C> {
    Apply(Option<usize>, VecDeque<Entry<C>>),
    Undo,
    Redo,
}

/// Wraps a record and gives it checkpoint functionality.
pub struct Checkpoint<'a, C: Command, F> {
    record: &'a mut Record<C, F>,
    commands: Vec<CheckpointCommand<C>>,
}

impl<C: Command, F: FnMut(Signal)> Checkpoint<'_, C, F> {
    /// Calls the `apply` method.
    pub fn apply(&mut self, command: C) -> Result<C> {
        let saved = self.record.saved;
        let (_, tail) = self.record.__apply(command)?;
        self.commands.push(CheckpointCommand::Apply(saved, tail));
        Ok(())
    }

    /// Calls the `undo` method.
    pub fn undo(&mut self) -> Result<C> {
        if self.record.can_undo() {
            self.record.undo()?;
            self.commands.push(CheckpointCommand::Undo);
        }
        Ok(())
    }

    /// Calls the `redo` method.
    pub fn redo(&mut self) -> Result<C> {
        if self.record.can_redo() {
            self.record.redo()?;
            self.commands.push(CheckpointCommand::Redo);
        }
        Ok(())
    }

    /// Commits the changes and consumes the checkpoint.
    pub fn commit(self) {}

    /// Cancels the changes and consumes the checkpoint.
    ///
    /// # Errors
    /// If an error occur when canceling the changes, the error is returned
    /// and the remaining commands are not canceled.
    pub fn cancel(self) -> Result<C> {
        for command in self.commands.into_iter().rev() {
            match command {
                CheckpointCommand::Apply(saved, mut entries) => {
                    self.record.undo()?;
                    self.record.entries.pop_back();
                    self.record.entries.append(&mut entries);
                    self.record.saved = saved;
                }
                CheckpointCommand::Undo => self.record.redo()?,
                CheckpointCommand::Redo => self.record.undo()?,
            }
        }
        Ok(())
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<C, F> {
        self.record.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<C, F> {
        self.record.checkpoint()
    }

    /// Returns a reference to the target.
    pub fn target(&self) -> &C::Target {
        self.record.target()
    }
}

/// Configurable display formatting for record.
#[derive(Copy, Clone)]
pub struct Display<'a, C: Command, F: FnMut(Signal)> {
    record: &'a Record<C, F>,
    format: crate::format::Format,
}

impl<C: Command, F: FnMut(Signal)> Display<'_, C, F> {
    /// Show colored output (on by default).
    ///
    /// Requires the `colored` feature to be enabled.
    #[cfg(feature = "colored")]
    pub fn colored(&mut self, on: bool) -> &mut Self {
        self.format.colored = on;
        self
    }

    /// Show the current position in the output (on by default).
    pub fn current(&mut self, on: bool) -> &mut Self {
        self.format.current = on;
        self
    }

    /// Show detailed output (on by default).
    pub fn detailed(&mut self, on: bool) -> &mut Self {
        self.format.detailed = on;
        self
    }

    /// Show the position of the command (on by default).
    pub fn position(&mut self, on: bool) -> &mut Self {
        self.format.position = on;
        self
    }

    /// Show the saved command (on by default).
    pub fn saved(&mut self, on: bool) -> &mut Self {
        self.format.saved = on;
        self
    }
}

impl<C: Command + fmt::Display, F: FnMut(Signal)> Display<'_, C, F> {
    fn fmt_list(&self, f: &mut fmt::Formatter, at: At, entry: &Entry<C>) -> fmt::Result {
        self.format.mark(f, 0)?;
        self.format.position(f, at, false)?;
        if self.format.detailed {
            #[cfg(feature = "chrono")]
            self.format.timestamp(f, &entry.timestamp)?;
        }
        self.format
            .current(f, at, At::new(0, self.record.current()))?;
        self.format
            .saved(f, at, self.record.saved.map(|saved| At::new(0, saved)))?;
        if self.format.detailed {
            writeln!(f)?;
            self.format.message(f, entry, 0)
        } else {
            f.write_char(' ')?;
            self.format.message(f, entry, 0)?;
            writeln!(f)
        }
    }
}

impl<C: Command + fmt::Display, F: FnMut(Signal)> fmt::Display for Display<'_, C, F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, entry) in self.record.entries.iter().enumerate().rev() {
            let at = At::new(0, i + 1);
            self.fmt_list(f, at, entry)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use alloc::string::String;

    struct Add(char);

    impl Command for Add {
        type Target = String;
        type Error = &'static str;

        fn apply(&mut self, s: &mut String) -> Result<Add> {
            s.push(self.0);
            Ok(())
        }

        fn undo(&mut self, s: &mut String) -> Result<Add> {
            self.0 = s.pop().ok_or("s is empty")?;
            Ok(())
        }
    }

    #[test]
    fn go_to() {
        let mut record = Record::default();
        record.apply(Add('a')).unwrap();
        record.apply(Add('b')).unwrap();
        record.apply(Add('c')).unwrap();
        record.apply(Add('d')).unwrap();
        record.apply(Add('e')).unwrap();

        record.go_to(0).unwrap().unwrap();
        assert_eq!(record.current(), 0);
        assert_eq!(record.target(), "");
        record.go_to(5).unwrap().unwrap();
        assert_eq!(record.current(), 5);
        assert_eq!(record.target(), "abcde");
        record.go_to(1).unwrap().unwrap();
        assert_eq!(record.current(), 1);
        assert_eq!(record.target(), "a");
        record.go_to(4).unwrap().unwrap();
        assert_eq!(record.current(), 4);
        assert_eq!(record.target(), "abcd");
        record.go_to(2).unwrap().unwrap();
        assert_eq!(record.current(), 2);
        assert_eq!(record.target(), "ab");
        record.go_to(3).unwrap().unwrap();
        assert_eq!(record.current(), 3);
        assert_eq!(record.target(), "abc");
        assert!(record.go_to(6).is_none());
        assert_eq!(record.current(), 3);
    }

    #[test]
    fn queue_commit() {
        let mut record = Record::default();
        let mut q1 = record.queue();
        q1.redo();
        q1.redo();
        q1.redo();
        let mut q2 = q1.queue();
        q2.undo();
        q2.undo();
        q2.undo();
        let mut q3 = q2.queue();
        q3.apply(Add('a'));
        q3.apply(Add('b'));
        q3.apply(Add('c'));
        assert_eq!(q3.target(), "");
        q3.commit().unwrap();
        assert_eq!(q2.target(), "abc");
        q2.commit().unwrap();
        assert_eq!(q1.target(), "");
        q1.commit().unwrap();
        assert_eq!(record.target(), "abc");
    }

    #[test]
    fn checkpoint_commit() {
        let mut record = Record::default();
        let mut cp1 = record.checkpoint();
        cp1.apply(Add('a')).unwrap();
        cp1.apply(Add('b')).unwrap();
        cp1.apply(Add('c')).unwrap();
        assert_eq!(cp1.target(), "abc");
        let mut cp2 = cp1.checkpoint();
        cp2.apply(Add('d')).unwrap();
        cp2.apply(Add('e')).unwrap();
        cp2.apply(Add('f')).unwrap();
        assert_eq!(cp2.target(), "abcdef");
        let mut cp3 = cp2.checkpoint();
        cp3.apply(Add('g')).unwrap();
        cp3.apply(Add('h')).unwrap();
        cp3.apply(Add('i')).unwrap();
        assert_eq!(cp3.target(), "abcdefghi");
        cp3.commit();
        cp2.commit();
        cp1.commit();
        assert_eq!(record.target(), "abcdefghi");
    }

    #[test]
    fn checkpoint_cancel() {
        let mut record = Record::default();
        let mut cp1 = record.checkpoint();
        cp1.apply(Add('a')).unwrap();
        cp1.apply(Add('b')).unwrap();
        cp1.apply(Add('c')).unwrap();
        let mut cp2 = cp1.checkpoint();
        cp2.apply(Add('d')).unwrap();
        cp2.apply(Add('e')).unwrap();
        cp2.apply(Add('f')).unwrap();
        let mut cp3 = cp2.checkpoint();
        cp3.apply(Add('g')).unwrap();
        cp3.apply(Add('h')).unwrap();
        cp3.apply(Add('i')).unwrap();
        assert_eq!(cp3.target(), "abcdefghi");
        cp3.cancel().unwrap();
        assert_eq!(cp2.target(), "abcdef");
        cp2.cancel().unwrap();
        assert_eq!(cp1.target(), "abc");
        cp1.cancel().unwrap();
        assert_eq!(record.target(), "");
    }

    #[test]
    fn checkpoint_saved() {
        let mut record = Record::default();
        record.apply(Add('a')).unwrap();
        record.apply(Add('b')).unwrap();
        record.apply(Add('c')).unwrap();
        record.set_saved(true);
        record.undo().unwrap();
        record.undo().unwrap();
        record.undo().unwrap();
        let mut cp = record.checkpoint();
        cp.apply(Add('d')).unwrap();
        cp.apply(Add('e')).unwrap();
        cp.apply(Add('f')).unwrap();
        assert_eq!(cp.target(), "def");
        cp.cancel().unwrap();
        assert_eq!(record.target(), "");
        record.redo().unwrap();
        record.redo().unwrap();
        record.redo().unwrap();
        assert!(record.is_saved());
        assert_eq!(record.target(), "abc");
    }
}
