#[cfg(feature = "display")]
use crate::Display;
use crate::{
    At, Checkpoint, Command, Entry, Queue, Record, RecordBuilder, Result, Signal, Timeline,
};
use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
#[cfg(feature = "chrono")]
use chrono::{DateTime, TimeZone};
use core::fmt;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A history of commands.
///
/// Unlike [Record] which maintains a linear undo history, History maintains an undo tree
/// containing every edit made to the target. By switching between different branches in the
/// tree, the user can get to any previous state of the target.
///
/// # Examples
/// ```
/// # use redo::{Command, History};
/// # struct Add(char);
/// # impl Command for Add {
/// #     type Target = String;
/// #     type Error = &'static str;
/// #     fn apply(&mut self, s: &mut String) -> redo::Result<Add> {
/// #         s.push(self.0);
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self, s: &mut String) -> redo::Result<Add> {
/// #         self.0 = s.pop().ok_or("`s` is empty")?;
/// #         Ok(())
/// #     }
/// # }
/// # fn main() -> redo::Result<Add> {
/// let mut history = History::default();
/// history.apply(Add('a'))?;
/// history.apply(Add('b'))?;
/// history.apply(Add('c'))?;
/// let abc = history.branch();
/// history.go_to(abc, 1).unwrap()?;
/// history.apply(Add('f'))?;
/// history.apply(Add('g'))?;
/// assert_eq!(history.target(), "afg");
/// history.go_to(abc, 3).unwrap()?;
/// assert_eq!(history.target(), "abc");
/// # Ok(())
/// # }
/// ```
///
/// [Record]: struct.Record.html
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(bound(
        serialize = "C: Command + Serialize, C::Target: Serialize",
        deserialize = "C: Command + Deserialize<'de>, C::Target: Deserialize<'de>"
    ))
)]
pub struct History<C: Command, F = fn(Signal)> {
    root: usize,
    next: usize,
    pub(crate) saved: Option<At>,
    pub(crate) record: Record<C, F>,
    pub(crate) branches: BTreeMap<usize, Branch<C>>,
}

impl<C: Command> History<C> {
    /// Returns a new history.
    #[inline]
    pub fn new(target: C::Target) -> History<C> {
        History::from(Record::new(target))
    }
}

impl<C: Command, F: FnMut(Signal)> History<C, F> {
    /// Reserves capacity for at least `additional` more commands.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.record.reserve(additional);
    }

    /// Returns the capacity of the history.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.record.capacity()
    }

    /// Shrinks the capacity of the history as much as possible.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.record.shrink_to_fit();
    }

    /// Returns the number of commands in the current branch of the history.
    #[inline]
    pub fn len(&self) -> usize {
        self.record.len()
    }

    /// Returns `true` if the current branch of the history is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.record.is_empty()
    }

    /// Returns the limit of the history.
    #[inline]
    pub fn limit(&self) -> usize {
        self.record.limit()
    }

    /// Sets the limit of the history and returns the new limit.
    ///
    /// If this limit is reached it will start popping of commands at the beginning
    /// of the history when new commands are applied. No limit is set by
    /// default which means it may grow indefinitely.
    ///
    /// If `limit < len` the first commands will be removed until `len == limit`.
    /// However, if the current active command is going to be removed, the limit is instead
    /// adjusted to `len - active` so the active command is not removed.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    #[inline]
    pub fn set_limit(&mut self, limit: usize) -> usize {
        let len = self.len();
        let limit = self.record.set_limit(limit);
        let diff = len - self.len();
        let root = self.branch();
        for current in 0..diff {
            self.rm_child(root, current);
        }
        for branch in self
            .branches
            .values_mut()
            .filter(|branch| branch.parent.branch == root)
        {
            branch.parent.current -= diff;
        }
        limit
    }

    /// Sets how the signal should be handled when the state changes.
    ///
    /// The previous slot is returned if it exists.
    #[inline]
    pub fn connect(&mut self, slot: F) -> Option<F> {
        self.record.connect(slot)
    }

    /// Creates a new history that uses the provided slot.
    #[inline]
    pub fn connect_with<G: FnMut(Signal)>(self, slot: G) -> History<C, G> {
        History {
            root: self.root,
            next: self.next,
            saved: self.saved,
            record: self.record.connect_with(slot),
            branches: self.branches,
        }
    }

    /// Removes and returns the slot.
    #[inline]
    pub fn disconnect(&mut self) -> Option<F> {
        self.record.disconnect()
    }

    /// Returns `true` if the target is in a saved state, `false` otherwise.
    #[inline]
    pub fn is_saved(&self) -> bool {
        self.record.is_saved()
    }

    /// Marks the target as currently being in a saved or unsaved state.
    #[inline]
    pub fn set_saved(&mut self, saved: bool) {
        self.record.set_saved(saved);
        self.saved = None;
    }

    /// Revert the changes done to the target since the saved state.
    #[inline]
    pub fn revert(&mut self) -> Option<Result<C>> {
        if self.record.saved.is_some() {
            self.record.revert()
        } else {
            self.saved
                .and_then(|saved| self.go_to(saved.branch, saved.current))
        }
    }

    /// Returns `true` if the history can undo.
    #[inline]
    pub fn can_undo(&self) -> bool {
        self.record.can_undo()
    }

    /// Returns `true` if the history can redo.
    #[inline]
    pub fn can_redo(&self) -> bool {
        self.record.can_redo()
    }

    /// Returns the current branch.
    #[inline]
    pub fn branch(&self) -> usize {
        self.root
    }

    /// Returns the position of the current command.
    #[inline]
    pub fn current(&self) -> usize {
        self.record.current()
    }

    /// Removes all commands from the history without undoing them.
    #[inline]
    pub fn clear(&mut self) {
        let old = self.branch();
        self.root = 0;
        self.next = 1;
        self.saved = None;
        self.record.clear();
        self.branches.clear();
        if let Some(ref mut slot) = self.record.slot {
            slot(Signal::Branch { old, new: 0 });
        }
    }

    /// Pushes the command to the top of the history and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    #[inline]
    pub fn apply(&mut self, command: C) -> Result<C> {
        let current = self.current();
        let saved = self.record.saved.filter(|&saved| saved > current);
        let (merged, commands) = self.record.__apply(Entry::from(command))?;
        // Check if the limit has been reached.
        if !merged && current == self.current() {
            let root = self.branch();
            self.rm_child(root, 0);
            for branch in self
                .branches
                .values_mut()
                .filter(|branch| branch.parent.branch == root)
            {
                branch.parent.current -= 1;
            }
        }
        // Handle new branch.
        if !commands.is_empty() {
            let old = self.branch();
            let new = self.next;
            self.next += 1;
            self.branches.insert(
                old,
                Branch {
                    parent: At {
                        branch: new,
                        current,
                    },
                    commands,
                },
            );
            self.set_root(new, current);
            match (self.record.saved, saved, self.saved) {
                (Some(_), None, None) | (None, None, Some(_)) => self.swap_saved(new, old, current),
                (None, Some(_), None) => {
                    self.record.saved = saved;
                    self.swap_saved(old, new, current);
                }
                (None, None, None) => (),
                _ => unreachable!(),
            }
            if let Some(ref mut slot) = self.record.slot {
                slot(Signal::Branch { old, new });
            }
        }
        Ok(())
    }

    /// Calls the [`undo`] method for the active command
    /// and sets the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result<C>> {
        self.record.undo()
    }

    /// Calls the [`redo`] method for the active command
    /// and sets the next one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the error is returned.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result<C>> {
        self.record.redo()
    }

    /// Repeatedly calls [`undo`] or [`redo`] until the command in `branch` at `current` is reached.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    pub fn go_to(&mut self, branch: usize, current: usize) -> Option<Result<C>> {
        let root = self.root;
        if root == branch {
            return self.record.go_to(current);
        }
        // Walk the path from `root` to `branch`.
        for (new, branch) in self.mk_path(branch)? {
            let old = self.branch();
            // Walk to `branch.current` either by undoing or redoing.
            if let Err(err) = self.record.go_to(branch.parent.current).unwrap() {
                return Some(Err(err));
            }
            // Apply the commands in the branch and move older commands into their own branch.
            for entry in branch.commands {
                let current = self.current();
                let saved = self.record.saved.filter(|&saved| saved > current);
                let commands = match self.record.__apply(entry) {
                    Ok((_, commands)) => commands,
                    Err(err) => return Some(Err(err)),
                };
                // Handle new branch.
                if !commands.is_empty() {
                    self.branches.insert(
                        self.root,
                        Branch {
                            parent: At {
                                branch: new,
                                current,
                            },
                            commands,
                        },
                    );
                    self.set_root(new, current);
                    match (self.record.saved, saved, self.saved) {
                        (Some(_), None, None) | (None, None, Some(_)) => {
                            self.swap_saved(new, old, current);
                        }
                        (None, Some(_), None) => {
                            self.record.saved = saved;
                            self.swap_saved(old, new, current);
                        }
                        (None, None, None) => (),
                        _ => unreachable!(),
                    }
                }
            }
        }
        if let Err(err) = self.record.go_to(current)? {
            return Some(Err(err));
        } else if let Some(ref mut slot) = self.record.slot {
            slot(Signal::Branch {
                old: root,
                new: self.root,
            });
        }
        Some(Ok(()))
    }

    /// Go back or forward in the history to the command that was made closest to the datetime provided.
    ///
    /// This method does not jump across branches.
    #[inline]
    #[cfg(feature = "chrono")]
    pub fn time_travel(&mut self, to: &DateTime<impl TimeZone>) -> Option<Result<C>> {
        self.record.time_travel(to)
    }

    /// Applies each command in the iterator.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    #[inline]
    pub fn extend(&mut self, commands: impl IntoIterator<Item = C>) -> Result<C> {
        for command in commands {
            self.apply(command)?;
        }
        Ok(())
    }

    /// Returns a queue.
    #[inline]
    pub fn queue(&mut self) -> Queue<History<C, F>> {
        Queue::from(self)
    }

    /// Returns a checkpoint.
    #[inline]
    pub fn checkpoint(&mut self) -> Checkpoint<History<C, F>> {
        Checkpoint::from(self)
    }

    /// Returns a reference to the `target`.
    #[inline]
    pub fn target(&self) -> &C::Target {
        self.record.target()
    }

    /// Returns a mutable reference to the `target`.
    ///
    /// This method should **only** be used when doing changes that should not be able to be undone.
    #[inline]
    pub fn target_mut(&mut self) -> &mut C::Target {
        self.record.target_mut()
    }

    /// Consumes the history, returning the `target`.
    #[inline]
    pub fn into_target(self) -> C::Target {
        self.record.into_target()
    }

    /// Sets the `root`.
    #[inline]
    fn set_root(&mut self, root: usize, current: usize) {
        let old = self.branch();
        self.root = root;
        debug_assert_ne!(old, root);
        // Handle the child branches.
        for branch in self
            .branches
            .values_mut()
            .filter(|branch| branch.parent.branch == old && branch.parent.current <= current)
        {
            branch.parent.branch = root;
        }
    }

    /// Swap the saved state if needed.
    #[inline]
    fn swap_saved(&mut self, old: usize, new: usize, current: usize) {
        debug_assert_ne!(old, new);
        if let Some(At { current: saved, .. }) = self
            .saved
            .filter(|at| at.branch == new && at.current <= current)
        {
            self.saved = None;
            self.record.saved = Some(saved);
            if let Some(ref mut slot) = self.record.slot {
                slot(Signal::Saved(true));
            }
        } else if let Some(saved) = self.record.saved {
            self.saved = Some(At {
                branch: old,
                current: saved,
            });
            self.record.saved = None;
            if let Some(ref mut slot) = self.record.slot {
                slot(Signal::Saved(false));
            }
        }
    }

    /// Remove all children of the command at the given position.
    #[inline]
    fn rm_child(&mut self, branch: usize, current: usize) {
        // We need to check if any of the branches had the removed node as root.
        let mut dead: Vec<_> = self
            .branches
            .iter()
            .filter(|&(_, child)| child.parent == At { branch, current })
            .map(|(&id, _)| id)
            .collect();
        while let Some(parent) = dead.pop() {
            // Remove the dead branch.
            self.branches.remove(&parent).unwrap();
            self.saved = self.saved.filter(|saved| saved.branch != parent);
            // Add the children of the dead branch so they are removed too.
            dead.extend(
                self.branches
                    .iter()
                    .filter(|&(_, child)| child.parent.branch == parent)
                    .map(|(&id, _)| id),
            )
        }
    }

    /// Create a path between the current branch and the `to` branch.
    #[inline]
    fn mk_path(&mut self, mut to: usize) -> Option<impl Iterator<Item = (usize, Branch<C>)>> {
        debug_assert_ne!(self.branch(), to);
        let mut dest = self.branches.remove(&to)?;
        let mut i = dest.parent.branch;
        let mut path = vec![(to, dest)];
        while i != self.branch() {
            dest = self.branches.remove(&i).unwrap();
            to = i;
            i = dest.parent.branch;
            path.push((to, dest));
        }
        Some(path.into_iter().rev())
    }
}

impl<C: Command + ToString, F: FnMut(Signal)> History<C, F> {
    /// Returns the string of the command which will be undone in the next call to [`undo`].
    ///
    /// [`undo`]: struct.History.html#method.undo
    #[inline]
    pub fn to_undo_string(&self) -> Option<String> {
        self.record.to_undo_string()
    }

    /// Returns the string of the command which will be redone in the next call to [`redo`].
    ///
    /// [`redo`]: struct.History.html#method.redo
    #[inline]
    pub fn to_redo_string(&self) -> Option<String> {
        self.record.to_redo_string()
    }

    /// Returns a structure for configurable formatting of the record.
    ///
    /// Requires the `display` feature to be enabled.
    #[inline]
    #[cfg(feature = "display")]
    pub fn display(&self) -> Display<Self> {
        Display::from(self)
    }
}

impl<C: Command, F: FnMut(Signal)> Timeline for History<C, F> {
    type Command = C;

    #[inline]
    fn apply(&mut self, command: Self::Command) -> Result<C> {
        self.apply(command)
    }

    #[inline]
    fn undo(&mut self) -> Option<Result<C>> {
        self.undo()
    }

    #[inline]
    fn redo(&mut self) -> Option<Result<C>> {
        self.redo()
    }
}

impl<C: Command> Default for History<C>
where
    C::Target: Default,
{
    #[inline]
    fn default() -> History<C> {
        History::new(Default::default())
    }
}

impl<C: Command, F: FnMut(Signal)> From<Record<C, F>> for History<C, F> {
    #[inline]
    fn from(record: Record<C, F>) -> Self {
        History {
            root: 0,
            next: 1,
            saved: None,
            record,
            branches: BTreeMap::default(),
        }
    }
}

impl<C: Command, F: FnMut(Signal)> fmt::Debug for History<C, F>
where
    C: fmt::Debug,
    C::Target: fmt::Debug,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("History")
            .field("root", &self.root)
            .field("next", &self.next)
            .field("saved", &self.saved)
            .field("record", &self.record)
            .field("branches", &self.branches)
            .finish()
    }
}

#[cfg(feature = "display")]
impl<C: Command, F: FnMut(Signal)> fmt::Display for History<C, F>
where
    C: fmt::Display,
    C::Target: fmt::Display,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.display() as &dyn fmt::Display).fmt(f)
    }
}

/// A branch in the history.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub(crate) struct Branch<C> {
    pub(crate) parent: At,
    pub(crate) commands: VecDeque<Entry<C>>,
}

/// Builder for a History.
///
/// # Examples
/// ```
/// # use redo::{self, Command, History, HistoryBuilder};
/// # struct Add(char);
/// # impl Command for Add {
/// #     type Target = String;
/// #     type Error = ();
/// #     fn apply(&mut self, s: &mut String) -> redo::Result<Add> { Ok(()) }
/// #     fn undo(&mut self, s: &mut String) -> redo::Result<Add> { Ok(()) }
/// # }
/// # fn foo() -> History<Add> {
/// HistoryBuilder::new()
///     .capacity(100)
///     .limit(100)
///     .saved(false)
///     .default()
/// # }
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct HistoryBuilder {
    inner: RecordBuilder,
}

impl HistoryBuilder {
    /// Returns a builder for a history.
    #[inline]
    pub fn new() -> HistoryBuilder {
        HistoryBuilder {
            inner: RecordBuilder::new(),
        }
    }

    /// Sets the capacity for the history.
    #[inline]
    pub fn capacity(&mut self, capacity: usize) -> &mut HistoryBuilder {
        self.inner.capacity(capacity);
        self
    }

    /// Sets the `limit` for the history.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    #[inline]
    pub fn limit(&mut self, limit: usize) -> &mut HistoryBuilder {
        self.inner.limit(limit);
        self
    }

    /// Sets if the target is initially in a saved state.
    /// By default the target is in a saved state.
    #[inline]
    pub fn saved(&mut self, saved: bool) -> &mut HistoryBuilder {
        self.inner.saved(saved);
        self
    }

    /// Builds the history.
    #[inline]
    pub fn build<C: Command>(&self, target: C::Target) -> History<C> {
        History::from(self.inner.build(target))
    }

    /// Builds the history with the slot.
    #[inline]
    pub fn build_with<C: Command, F: FnMut(Signal)>(
        &self,
        target: C::Target,
        slot: F,
    ) -> History<C, F> {
        History::from(self.inner.build_with(target, slot))
    }

    /// Creates the history with a default `target`.
    #[inline]
    pub fn default<C: Command>(&self) -> History<C>
    where
        C::Target: Default,
    {
        self.build(Default::default())
    }

    /// Creates the history with a default `target` and with the slot.
    #[inline]
    pub fn default_with<C: Command, F: FnMut(Signal)>(&self, slot: F) -> History<C, F>
    where
        C::Target: Default,
    {
        self.build_with(Default::default(), slot)
    }
}

impl Default for HistoryBuilder {
    #[inline]
    fn default() -> Self {
        HistoryBuilder::new()
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

        fn apply(&mut self, target: &mut String) -> Result<Add> {
            target.push(self.0);
            Ok(())
        }

        fn undo(&mut self, target: &mut String) -> Result<Add> {
            self.0 = target.pop().ok_or("`target` is empty")?;
            Ok(())
        }
    }

    #[test]
    fn go_to() {
        //          m
        //          |
        //    j  k  l
        //     \ | /
        //       i
        //       |
        // e  g  h
        // |  | /
        // d  f  p - q *
        // | /  /
        // c  n - o
        // | /
        // b
        // |
        // a
        let mut history = History::default();
        history.apply(Add('a')).unwrap();
        history.apply(Add('b')).unwrap();
        history.apply(Add('c')).unwrap();
        history.apply(Add('d')).unwrap();
        history.apply(Add('e')).unwrap();
        assert_eq!(history.target(), "abcde");
        history.undo().unwrap().unwrap();
        history.undo().unwrap().unwrap();
        assert_eq!(history.target(), "abc");
        let abcde = history.branch();
        history.apply(Add('f')).unwrap();
        history.apply(Add('g')).unwrap();
        assert_eq!(history.target(), "abcfg");
        history.undo().unwrap().unwrap();
        let abcfg = history.branch();
        history.apply(Add('h')).unwrap();
        history.apply(Add('i')).unwrap();
        history.apply(Add('j')).unwrap();
        assert_eq!(history.target(), "abcfhij");
        history.undo().unwrap().unwrap();
        let abcfhij = history.branch();
        history.apply(Add('k')).unwrap();
        assert_eq!(history.target(), "abcfhik");
        history.undo().unwrap().unwrap();
        let abcfhik = history.branch();
        history.apply(Add('l')).unwrap();
        assert_eq!(history.target(), "abcfhil");
        history.apply(Add('m')).unwrap();
        assert_eq!(history.target(), "abcfhilm");
        let abcfhilm = history.branch();
        history.go_to(abcde, 2).unwrap().unwrap();
        history.apply(Add('n')).unwrap();
        history.apply(Add('o')).unwrap();
        assert_eq!(history.target(), "abno");
        history.undo().unwrap().unwrap();
        let abno = history.branch();
        history.apply(Add('p')).unwrap();
        history.apply(Add('q')).unwrap();
        assert_eq!(history.target(), "abnpq");

        let abnpq = history.branch();
        history.go_to(abcde, 5).unwrap().unwrap();
        assert_eq!(history.target(), "abcde");
        history.go_to(abcfg, 5).unwrap().unwrap();
        assert_eq!(history.target(), "abcfg");
        history.go_to(abcfhij, 7).unwrap().unwrap();
        assert_eq!(history.target(), "abcfhij");
        history.go_to(abcfhik, 7).unwrap().unwrap();
        assert_eq!(history.target(), "abcfhik");
        history.go_to(abcfhilm, 8).unwrap().unwrap();
        assert_eq!(history.target(), "abcfhilm");
        history.go_to(abno, 4).unwrap().unwrap();
        assert_eq!(history.target(), "abno");
        history.go_to(abnpq, 5).unwrap().unwrap();
        assert_eq!(history.target(), "abnpq");
    }
}
