use crate::{Checkpoint, Command, Display, History, Merge, Meta, Queue, Signal};
#[cfg(feature = "chrono")]
use chrono::{DateTime, TimeZone, Utc};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "chrono")]
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::fmt;
use std::marker::PhantomData;
use std::num::NonZeroUsize;

#[allow(unsafe_code)]
const MAX_LIMIT: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(usize::max_value()) };

/// A record of commands.
///
/// The record can roll the receivers state backwards and forwards by using
/// the undo and redo methods. In addition, the record can notify the user
/// about changes to the stack or the receiver through [signal]. The user
/// can give the record a function that is called each time the state changes
/// by using the [`builder`].
///
/// # Examples
/// ```
/// # use redo::{Command, Record};
/// # struct Add(char);
/// # impl Command<String> for Add {
/// #     type Error = &'static str;
/// #     fn apply(&mut self, s: &mut String) -> Result<(), Self::Error> {
/// #         s.push(self.0);
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self, s: &mut String) -> Result<(), Self::Error> {
/// #         self.0 = s.pop().ok_or("`s` is empty")?;
/// #         Ok(())
/// #     }
/// # }
/// # fn main() -> Result<(), &'static str> {
/// let mut record = Record::default();
/// record.apply(Add('a'))?;
/// record.apply(Add('b'))?;
/// record.apply(Add('c'))?;
/// assert_eq!(record.as_receiver(), "abc");
/// record.undo().unwrap()?;
/// record.undo().unwrap()?;
/// record.undo().unwrap()?;
/// assert_eq!(record.as_receiver(), "");
/// record.redo().unwrap()?;
/// record.redo().unwrap()?;
/// record.redo().unwrap()?;
/// assert_eq!(record.as_receiver(), "abc");
/// # Ok(())
/// # }
/// ```
///
/// [`builder`]: struct.RecordBuilder.html
/// [signal]: enum.Signal.html
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct Record<R, C: Command<R>, F = fn(Signal)> {
    pub(crate) commands: VecDeque<Meta<C>>,
    receiver: R,
    current: usize,
    limit: NonZeroUsize,
    pub(crate) saved: Option<usize>,
    pub(crate) slot: Option<F>,
}

impl<R, C: Command<R>> Record<R, C> {
    /// Returns a new record.
    #[inline]
    pub fn new(receiver: impl Into<R>) -> Record<R, C> {
        Record {
            commands: VecDeque::new(),
            receiver: receiver.into(),
            current: 0,
            limit: MAX_LIMIT,
            saved: Some(0),
            slot: None,
        }
    }
}

impl<R, C: Command<R>, F: FnMut(Signal)> Record<R, C, F> {
    /// Returns a builder for a record.
    #[inline]
    pub fn builder() -> RecordBuilder<R, C, F> {
        RecordBuilder {
            commands: PhantomData,
            receiver: PhantomData,
            capacity: 0,
            limit: MAX_LIMIT,
            saved: true,
            slot: PhantomData,
        }
    }

    /// Reserves capacity for at least `additional` more commands.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.commands.reserve(additional);
    }

    /// Returns the capacity of the record.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.commands.capacity()
    }

    /// Returns the number of commands in the record.
    #[inline]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns `true` if the record is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Returns the limit of the record.
    #[inline]
    pub fn limit(&self) -> usize {
        self.limit.get()
    }

    /// Sets the limit of the record and returns the new limit.
    ///
    /// If this limit is reached it will start popping of commands at the beginning
    /// of the record when new commands are applied. No limit is set by
    /// default which means it may grow indefinitely.
    ///
    /// If `0 < limit < len` the first commands will be removed until `len == limit`.
    /// However, if the current active command is going to be removed, the limit is instead
    /// adjusted to `len - active` so the active command is not removed.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    #[inline]
    pub fn set_limit(&mut self, limit: usize) -> usize {
        self.limit = NonZeroUsize::new(limit).expect("limit can not be `0`");
        if limit < self.len() {
            let old = self.current();
            let could_undo = self.can_undo();
            let was_saved = self.is_saved();
            let begin = old.min(self.len() - limit);
            self.commands = self.commands.split_off(begin);
            self.limit = NonZeroUsize::new(self.len()).unwrap();
            self.current -= begin;
            // Check if the saved state has been removed.
            self.saved = self.saved.and_then(|saved| saved.checked_sub(begin));
            let new = self.current();
            let can_undo = self.can_undo();
            let is_saved = self.is_saved();
            if let Some(ref mut slot) = self.slot {
                if old != new {
                    slot(Signal::Current { old, new });
                }
                if could_undo != can_undo {
                    slot(Signal::Undo(can_undo));
                }
                if was_saved != is_saved {
                    slot(Signal::Saved(is_saved));
                }
            }
        }
        self.limit()
    }

    /// Sets how the signal should be handled when the state changes.
    ///
    /// The previous slot is returned if it exists.
    #[inline]
    pub fn connect(&mut self, slot: F) -> Option<F> {
        self.slot.replace(slot)
    }

    /// Creates a new record that uses the provided slot.
    #[inline]
    pub fn set_and_connect<G>(self, slot: G) -> Record<R, C, G> {
        Record {
            commands: self.commands,
            receiver: self.receiver,
            current: self.current,
            limit: self.limit,
            saved: self.saved,
            slot: Some(slot),
        }
    }

    /// Creates a new record by taking a closure that maps the current slot.
    #[inline]
    pub fn map_and_connect<G>(self, f: impl FnOnce(F) -> G) -> Record<R, C, G> {
        Record {
            commands: self.commands,
            receiver: self.receiver,
            current: self.current,
            limit: self.limit,
            saved: self.saved,
            slot: self.slot.map(f),
        }
    }

    /// Removes and returns the slot.
    #[inline]
    pub fn disconnect(&mut self) -> Option<F> {
        self.slot.take()
    }

    /// Returns `true` if the record can undo.
    #[inline]
    pub fn can_undo(&self) -> bool {
        self.current() > 0
    }

    /// Returns `true` if the record can redo.
    #[inline]
    pub fn can_redo(&self) -> bool {
        self.current() < self.len()
    }

    /// Marks the receiver as currently being in a saved or unsaved state.
    #[inline]
    pub fn set_saved(&mut self, saved: bool) {
        let was_saved = self.is_saved();
        if saved {
            self.saved = Some(self.current());
            if let Some(ref mut slot) = self.slot {
                if !was_saved {
                    slot(Signal::Saved(true));
                }
            }
        } else {
            self.saved = None;
            if let Some(ref mut slot) = self.slot {
                if was_saved {
                    slot(Signal::Saved(false));
                }
            }
        }
    }

    /// Returns `true` if the receiver is in a saved state, `false` otherwise.
    #[inline]
    pub fn is_saved(&self) -> bool {
        self.saved.map_or(false, |saved| saved == self.current())
    }

    /// Revert the changes done to the receiver since the saved state.
    #[inline]
    pub fn revert(&mut self) -> Option<Result<(), C::Error>> {
        self.saved.and_then(|saved| self.go_to(saved))
    }

    /// Returns the position of the current command.
    #[inline]
    pub fn current(&self) -> usize {
        self.current
    }

    /// Removes all commands from the record without undoing them.
    #[inline]
    pub fn clear(&mut self) {
        let old = self.current();
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        self.commands.clear();
        self.saved = if self.is_saved() { Some(0) } else { None };
        self.current = 0;
        if let Some(ref mut slot) = self.slot {
            if old != 0 {
                slot(Signal::Current { old, new: 0 });
            }
            if could_undo {
                slot(Signal::Undo(false));
            }
            if could_redo {
                slot(Signal::Redo(false));
            }
        }
    }

    /// Pushes the command on top of the record and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned
    /// and the state of the record is left unchanged.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    #[inline]
    pub fn apply(&mut self, command: C) -> Result<(), C::Error> {
        self.__apply(Meta::from(command)).map(|_| ())
    }

    #[inline]
    pub(crate) fn __apply(
        &mut self,
        mut meta: Meta<C>,
    ) -> Result<(bool, VecDeque<Meta<C>>), C::Error> {
        if meta.is_dead() {
            return Ok((false, VecDeque::new()));
        }
        if let Err(error) = meta.apply(&mut self.receiver) {
            return Err(error);
        }
        let current = self.current();
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        // Pop off all elements after len from record.
        let v = self.commands.split_off(current);
        debug_assert_eq!(current, self.len());
        // Check if the saved state was popped off.
        self.saved = self.saved.filter(|&saved| saved <= current);
        // Try to merge commands unless the receiver is in a saved state.
        let merged = match self.commands.back_mut() {
            Some(ref mut last) if !was_saved => last.merge(meta),
            _ => Merge::No(meta),
        };
        let merged_or_annulled = match merged {
            Merge::Yes => true,
            Merge::Annul => {
                self.commands.pop_back();
                true
            }
            // If commands are not merged or annulled push it onto the record.
            Merge::No(meta) => {
                // If limit is reached, pop off the first command.
                if self.limit() == self.current() {
                    self.commands.pop_front();
                    self.saved = self.saved.and_then(|saved| saved.checked_sub(1));
                } else {
                    self.current += 1;
                }
                self.commands.push_back(meta);
                false
            }
        };
        debug_assert_eq!(self.current(), self.len());
        if let Some(ref mut slot) = self.slot {
            // We emit this signal even if the commands might have been merged.
            slot(Signal::Current {
                old: current,
                new: self.current,
            });
            if could_redo {
                slot(Signal::Redo(false));
            }
            if !could_undo {
                slot(Signal::Undo(true));
            }
            if was_saved {
                slot(Signal::Saved(false));
            }
        }
        Ok((merged_or_annulled, v))
    }

    /// Calls the [`undo`] method for the active command and sets
    /// the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned
    /// and the state of the record is left unchanged.
    ///
    /// [`undo`]: ../trait.Command.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result<(), C::Error>> {
        let was_saved = self.is_saved();
        let old = self.current();
        loop {
            if !self.can_undo() {
                return None;
            } else if self.commands[self.current - 1].is_dead() {
                self.current -= 1;
                self.commands.remove(self.current).unwrap();
            } else {
                break;
            }
        }
        if let Err(error) = self.commands[self.current - 1].undo(&mut self.receiver) {
            return Some(Err(error));
        }
        self.current -= 1;
        let len = self.len();
        let is_saved = self.is_saved();
        if let Some(ref mut slot) = self.slot {
            slot(Signal::Current {
                old,
                new: self.current,
            });
            if old == len {
                slot(Signal::Redo(true));
            }
            if old == 1 {
                slot(Signal::Undo(false));
            }
            if was_saved != is_saved {
                slot(Signal::Saved(is_saved));
            }
        }
        Some(Ok(()))
    }

    /// Calls the [`redo`] method for the active command and sets
    /// the next one as the new active one.
    ///
    /// # Errors
    /// If an error occur when applying [`redo`] the error is returned
    /// and the state of the record is left unchanged.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result<(), C::Error>> {
        let was_saved = self.is_saved();
        let old = self.current();
        loop {
            if !self.can_redo() {
                return None;
            } else if self.commands[self.current].is_dead() {
                self.commands.remove(self.current).unwrap();
            } else {
                break;
            }
        }
        if let Err(error) = self.commands[self.current].redo(&mut self.receiver) {
            return Some(Err(error));
        }
        self.current += 1;
        let len = self.len();
        let is_saved = self.is_saved();
        if let Some(ref mut slot) = self.slot {
            slot(Signal::Current {
                old,
                new: self.current,
            });
            if old == len - 1 {
                slot(Signal::Redo(false));
            }
            if old == 0 {
                slot(Signal::Undo(true));
            }
            if was_saved != is_saved {
                slot(Signal::Saved(is_saved));
            }
        }
        Some(Ok(()))
    }

    /// Repeatedly calls [`undo`] or [`redo`] until the command at `current` is reached.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned
    /// and the state of the record is left unchanged.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    pub fn go_to(&mut self, current: usize) -> Option<Result<(), C::Error>> {
        if current > self.len() {
            return None;
        }
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        let old = self.current();
        // Temporarily remove slot so they are not called each iteration.
        let slot = self.slot.take();
        while self.current() != current {
            // Decide if we need to undo or redo to reach current.
            let f = if current > self.current() {
                Record::redo
            } else {
                Record::undo
            };
            if let Err(err) = f(self).unwrap() {
                return Some(Err(err));
            }
        }
        // Add slot back.
        self.slot = slot;
        let can_undo = self.can_undo();
        let can_redo = self.can_redo();
        let is_saved = self.is_saved();
        if let Some(ref mut slot) = self.slot {
            if old != self.current {
                slot(Signal::Current {
                    old,
                    new: self.current,
                });
            }
            if could_undo != can_undo {
                slot(Signal::Undo(can_undo));
            }
            if could_redo != can_redo {
                slot(Signal::Redo(can_redo));
            }
            if was_saved != is_saved {
                slot(Signal::Saved(is_saved));
            }
        }
        Some(Ok(()))
    }

    /// Go back or forward in time.
    #[inline]
    #[cfg(feature = "chrono")]
    pub fn time_travel<Tz: TimeZone>(&mut self, to: &DateTime<Tz>) -> Option<Result<(), C::Error>> {
        let to = Utc.from_utc_datetime(&to.naive_utc());
        let current = match self.commands.as_slices() {
            ([], []) => return None,
            (start, []) => match start.binary_search_by(|meta| meta.timestamp.cmp(&to)) {
                Ok(current) | Err(current) => current,
            },
            ([], end) => match end.binary_search_by(|meta| meta.timestamp.cmp(&to)) {
                Ok(current) | Err(current) => current,
            },
            (start, end) => match start.last().unwrap().timestamp.cmp(&to) {
                Ordering::Less => match start.binary_search_by(|meta| meta.timestamp.cmp(&to)) {
                    Ok(current) | Err(current) => current,
                },
                Ordering::Equal => start.len(),
                Ordering::Greater => match end.binary_search_by(|meta| meta.timestamp.cmp(&to)) {
                    Ok(current) | Err(current) => start.len() + current,
                },
            },
        };
        self.go_to(current)
    }

    /// Applies each command in the iterator.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned
    /// and the remaining commands in the iterator are discarded.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    #[inline]
    pub fn extend(&mut self, commands: impl IntoIterator<Item = C>) -> Result<(), C::Error> {
        for command in commands {
            self.apply(command)?;
        }
        Ok(())
    }

    /// Returns a checkpoint.
    #[inline]
    pub fn checkpoint(&mut self) -> Checkpoint<Record<R, C, F>, C> {
        Checkpoint::from(self)
    }

    /// Returns a queue.
    #[inline]
    pub fn queue(&mut self) -> Queue<Record<R, C, F>, C> {
        Queue::from(self)
    }

    /// Returns a reference to the `receiver`.
    #[inline]
    pub fn as_receiver(&self) -> &R {
        &self.receiver
    }

    /// Returns a mutable reference to the `receiver`.
    ///
    /// This method should **only** be used when doing changes that should not be able to be undone.
    #[inline]
    pub fn as_mut_receiver(&mut self) -> &mut R {
        &mut self.receiver
    }

    /// Consumes the record, returning the `receiver`.
    #[inline]
    pub fn into_receiver(self) -> R {
        self.receiver
    }

    /// Returns an iterator over the commands in the record.
    #[inline]
    pub fn commands(&self) -> impl Iterator<Item = &C> {
        self.commands.iter().map(|meta| &meta.command)
    }
}

impl<R, C: Command<R> + ToString, F: FnMut(Signal)> Record<R, C, F> {
    /// Returns the string of the command which will be undone in the next call to [`undo`].
    ///
    /// [`undo`]: struct.Record.html#method.undo
    #[inline]
    pub fn to_undo_string(&self) -> Option<String> {
        if self.can_undo() {
            Some(self.commands[self.current - 1].command.to_string())
        } else {
            None
        }
    }

    /// Returns the string of the command which will be redone in the next call to [`redo`].
    ///
    /// [`redo`]: struct.Record.html#method.redo
    #[inline]
    pub fn to_redo_string(&self) -> Option<String> {
        if self.can_redo() {
            Some(self.commands[self.current].command.to_string())
        } else {
            None
        }
    }

    /// Returns a structure for configurable formatting of the record.
    #[inline]
    pub fn display(&self) -> Display<Self> {
        Display::from(self)
    }
}

impl<R: Default, C: Command<R>> Default for Record<R, C> {
    #[inline]
    fn default() -> Record<R, C> {
        Record::new(R::default())
    }
}

impl<R, C: Command<R>, F: FnMut(Signal)> AsRef<R> for Record<R, C, F> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.as_receiver()
    }
}

impl<R, C: Command<R>, F: FnMut(Signal)> AsMut<R> for Record<R, C, F> {
    #[inline]
    fn as_mut(&mut self) -> &mut R {
        self.as_mut_receiver()
    }
}

impl<R, C: Command<R>> From<R> for Record<R, C> {
    #[inline]
    fn from(receiver: R) -> Record<R, C> {
        Record::new(receiver)
    }
}

impl<R, C: Command<R>, F> From<History<R, C, F>> for Record<R, C, F> {
    #[inline]
    fn from(history: History<R, C, F>) -> Record<R, C, F> {
        history.record
    }
}

impl<R, C: Command<R> + fmt::Display, F: FnMut(Signal)> fmt::Display for Record<R, C, F> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.display() as &dyn fmt::Display).fmt(f)
    }
}

/// Builder for a record.
///
/// # Examples
/// ```
/// # use redo::{Command, Record};
/// # struct Add(char);
/// # impl Command<String> for Add {
/// #     type Error = ();
/// #     fn apply(&mut self, s: &mut String) -> Result<(), Self::Error> { Ok(()) }
/// #     fn undo(&mut self, s: &mut String) -> Result<(), Self::Error> { Ok(()) }
/// # }
/// # fn foo() -> Record<String, Add> {
/// Record::builder()
///     .capacity(100)
///     .limit(100)
///     .saved(false)
///     .default()
/// # }
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct RecordBuilder<R, C: Command<R>, F = fn(Signal)> {
    commands: PhantomData<C>,
    receiver: PhantomData<R>,
    capacity: usize,
    limit: NonZeroUsize,
    saved: bool,
    slot: PhantomData<F>,
}

impl<R, C: Command<R>> RecordBuilder<R, C> {
    /// Builds the record.
    #[inline]
    pub fn build(self, receiver: impl Into<R>) -> Record<R, C> {
        Record {
            commands: VecDeque::with_capacity(self.capacity),
            receiver: receiver.into(),
            current: 0,
            limit: self.limit,
            saved: if self.saved { Some(0) } else { None },
            slot: None,
        }
    }
}

impl<R, C: Command<R>, F> RecordBuilder<R, C, F> {
    /// Sets the capacity for the record.
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> RecordBuilder<R, C, F> {
        self.capacity = capacity;
        self
    }

    /// Sets the `limit` of the record.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    #[inline]
    pub fn limit(mut self, limit: usize) -> RecordBuilder<R, C, F> {
        self.limit = NonZeroUsize::new(limit).expect("limit can not be `0`");
        self
    }

    /// Sets if the receiver is initially in a saved state.
    /// By default the receiver is in a saved state.
    #[inline]
    pub fn saved(mut self, saved: bool) -> RecordBuilder<R, C, F> {
        self.saved = saved;
        self
    }

    /// Builds the record with the slot.
    #[inline]
    pub fn build_and_connect(self, receiver: impl Into<R>, slot: F) -> Record<R, C, F> {
        Record {
            commands: VecDeque::with_capacity(self.capacity),
            receiver: receiver.into(),
            current: 0,
            limit: self.limit,
            saved: if self.saved { Some(0) } else { None },
            slot: Some(slot),
        }
    }
}

impl<R: Default, C: Command<R>> RecordBuilder<R, C> {
    /// Creates the record with a default `receiver`.
    #[inline]
    pub fn default(self) -> Record<R, C> {
        self.build(R::default())
    }
}

impl<R: Default, C: Command<R>, F> RecordBuilder<R, C, F> {
    /// Creates the record with a default `receiver`.
    #[inline]
    pub fn default_and_connect(self, slot: F) -> Record<R, C, F> {
        self.build_and_connect(R::default(), slot)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Command, Record};

    #[derive(Debug)]
    struct Add(char);

    impl Command<String> for Add {
        type Error = &'static str;

        fn apply(&mut self, s: &mut String) -> Result<(), Self::Error> {
            s.push(self.0);
            Ok(())
        }

        fn undo(&mut self, s: &mut String) -> Result<(), Self::Error> {
            self.0 = s.pop().ok_or("`s` is empty")?;
            Ok(())
        }
    }

    #[test]
    fn set_limit() {
        let mut record = Record::default();
        record.apply(Add('a')).unwrap();
        record.apply(Add('b')).unwrap();
        record.apply(Add('c')).unwrap();
        record.apply(Add('d')).unwrap();
        record.apply(Add('e')).unwrap();

        record.set_limit(3);
        assert_eq!(record.current(), 3);
        assert_eq!(record.limit(), 3);
        assert_eq!(record.len(), 3);
        assert!(record.can_undo());
        assert!(!record.can_redo());

        record.clear();
        assert_eq!(record.set_limit(5), 5);
        record.apply(Add('a')).unwrap();
        record.apply(Add('b')).unwrap();
        record.apply(Add('c')).unwrap();
        record.apply(Add('d')).unwrap();
        record.apply(Add('e')).unwrap();

        record.undo().unwrap().unwrap();
        record.undo().unwrap().unwrap();
        record.undo().unwrap().unwrap();

        record.set_limit(2);
        assert_eq!(record.current(), 0);
        assert_eq!(record.limit(), 3);
        assert_eq!(record.len(), 3);
        assert!(!record.can_undo());
        assert!(record.can_redo());

        record.redo().unwrap().unwrap();
        record.redo().unwrap().unwrap();
        record.redo().unwrap().unwrap();

        record.clear();
        assert_eq!(record.set_limit(5), 5);
        record.apply(Add('a')).unwrap();
        record.apply(Add('b')).unwrap();
        record.apply(Add('c')).unwrap();
        record.apply(Add('d')).unwrap();
        record.apply(Add('e')).unwrap();

        record.undo().unwrap().unwrap();
        record.undo().unwrap().unwrap();
        record.undo().unwrap().unwrap();
        record.undo().unwrap().unwrap();
        record.undo().unwrap().unwrap();

        record.set_limit(2);
        assert_eq!(record.current(), 0);
        assert_eq!(record.limit(), 5);
        assert_eq!(record.len(), 5);
        assert!(!record.can_undo());
        assert!(record.can_redo());

        record.redo().unwrap().unwrap();
        record.redo().unwrap().unwrap();
        record.redo().unwrap().unwrap();
        record.redo().unwrap().unwrap();
        record.redo().unwrap().unwrap();
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
        assert_eq!(record.as_receiver(), "");
        record.go_to(5).unwrap().unwrap();
        assert_eq!(record.current(), 5);
        assert_eq!(record.as_receiver(), "abcde");
        record.go_to(1).unwrap().unwrap();
        assert_eq!(record.current(), 1);
        assert_eq!(record.as_receiver(), "a");
        record.go_to(4).unwrap().unwrap();
        assert_eq!(record.current(), 4);
        assert_eq!(record.as_receiver(), "abcd");
        record.go_to(2).unwrap().unwrap();
        assert_eq!(record.current(), 2);
        assert_eq!(record.as_receiver(), "ab");
        record.go_to(3).unwrap().unwrap();
        assert_eq!(record.current(), 3);
        assert_eq!(record.as_receiver(), "abc");
        assert!(record.go_to(6).is_none());
        assert_eq!(record.current(), 3);
    }
}
