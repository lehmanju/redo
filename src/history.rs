use crate::{At, Checkpoint, Command, Display, Meta, Queue, Record, RecordBuilder, Signal};
#[cfg(feature = "chrono")]
use chrono::{DateTime, TimeZone};
use rustc_hash::FxHashMap;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt;

/// A history of commands.
///
/// Unlike [Record] which maintains a linear undo history, History maintains an undo tree
/// containing every edit made to the receiver. By switching between different branches in the
/// tree, the user can get to any previous state of the receiver.
///
/// # Examples
/// ```
/// # use redo::{Command, History};
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
/// let mut history = History::default();
/// history.apply(Add('a'))?;
/// history.apply(Add('b'))?;
/// history.apply(Add('c'))?;
/// let abc = history.root();
/// history.go_to(abc, 1).unwrap()?;
/// history.apply(Add('f'))?;
/// history.apply(Add('g'))?;
/// assert_eq!(history.as_receiver(), "afg");
/// history.go_to(abc, 3).unwrap()?;
/// assert_eq!(history.as_receiver(), "abc");
/// # Ok(())
/// # }
/// ```
///
/// [Record]: struct.Record.html
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct History<R, C, F = fn(Signal)> {
    root: usize,
    next: usize,
    pub(crate) saved: Option<At>,
    pub(crate) record: Record<R, C, F>,
    pub(crate) branches: FxHashMap<usize, Branch<C>>,
}

impl<R, C> History<R, C> {
    /// Returns a new history.
    #[inline]
    pub fn new(receiver: impl Into<R>) -> History<R, C> {
        History {
            root: 0,
            next: 1,
            saved: None,
            record: Record::new(receiver),
            branches: FxHashMap::default(),
        }
    }
}

impl<R, C: Command<R>, F: FnMut(Signal)> History<R, C, F> {
    /// Returns a builder for a history.
    #[inline]
    pub fn builder() -> HistoryBuilder<R, C, F> {
        HistoryBuilder {
            inner: Record::builder(),
        }
    }

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
        let root = self.root();
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
    pub fn set_and_connect<G>(self, slot: G) -> History<R, C, G> {
        History {
            root: self.root,
            next: self.next,
            saved: self.saved,
            record: self.record.set_and_connect(slot),
            branches: self.branches,
        }
    }

    /// Creates a new history by taking a closure that maps the current slot.
    #[inline]
    pub fn map_and_connect<G>(self, f: impl FnOnce(F) -> G) -> History<R, C, G> {
        History {
            root: self.root,
            next: self.next,
            saved: self.saved,
            record: self.record.map_and_connect(f),
            branches: self.branches,
        }
    }

    /// Removes and returns the slot.
    #[inline]
    pub fn disconnect(&mut self) -> Option<F> {
        self.record.disconnect()
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

    /// Marks the receiver as currently being in a saved or unsaved state.
    #[inline]
    pub fn set_saved(&mut self, saved: bool) {
        self.record.set_saved(saved);
        self.saved = None;
    }

    /// Returns `true` if the receiver is in a saved state, `false` otherwise.
    #[inline]
    pub fn is_saved(&self) -> bool {
        self.record.is_saved()
    }

    /// Revert the changes done to the receiver since the saved state.
    #[inline]
    pub fn revert(&mut self) -> Option<Result<(), C::Error>> {
        if self.record.saved.is_some() {
            self.record.revert()
        } else {
            self.saved
                .and_then(|saved| self.go_to(saved.branch, saved.current))
        }
    }

    /// Returns the current branch.
    #[inline]
    pub fn root(&self) -> usize {
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
        let old = self.root();
        self.root = 0;
        self.next = 1;
        self.saved = None;
        self.record.clear();
        self.branches.clear();
        if let Some(ref mut slot) = self.record.slot {
            slot(Signal::Root { old, new: 0 });
        }
    }

    /// Pushes the command to the top of the history and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned
    /// and the state of the history is left unchanged.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    #[inline]
    pub fn apply(&mut self, command: C) -> Result<(), C::Error> {
        let current = self.current();
        let saved = self.record.saved.filter(|&saved| saved > current);
        let (merged, commands) = self.record.__apply(Meta::from(command))?;
        // Check if the limit has been reached.
        if !merged && current == self.current() {
            let root = self.root();
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
            let old = self.root();
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
                slot(Signal::Root { old, new });
            }
        }
        Ok(())
    }

    /// Calls the [`undo`] method for the active command
    /// and sets the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned
    /// and the state of the history is left unchanged.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result<(), C::Error>> {
        self.record.undo()
    }

    /// Calls the [`redo`] method for the active command
    /// and sets the next one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the error is returned
    /// and the state of the history is left unchanged.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result<(), C::Error>> {
        self.record.redo()
    }

    /// Repeatedly calls [`undo`] or [`redo`] until the command in `branch` at `current` is reached.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned
    /// and the state of the history is left unchanged.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    pub fn go_to(&mut self, branch: usize, current: usize) -> Option<Result<(), C::Error>> {
        let root = self.root;
        if root == branch {
            return self.record.go_to(current);
        }
        // Walk the path from `root` to `branch`.
        for (new, branch) in self.mk_path(branch)? {
            let old = self.root();
            // Walk to `branch.current` either by undoing or redoing.
            if let Err(err) = self.record.go_to(branch.parent.current).unwrap() {
                return Some(Err(err));
            }
            // Apply the commands in the branch and move older commands into their own branch.
            for meta in branch.commands {
                let current = self.current();
                let saved = self.record.saved.filter(|&saved| saved > current);
                let commands = match self.record.__apply(meta) {
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
            slot(Signal::Root {
                old: root,
                new: self.root,
            });
        }
        Some(Ok(()))
    }

    /// Go back or forward in time.
    #[inline]
    #[cfg(feature = "chrono")]
    pub fn time_travel<Tz: TimeZone>(&mut self, to: &DateTime<Tz>) -> Option<Result<(), C::Error>> {
        self.record.time_travel(to)
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
    pub fn checkpoint(&mut self) -> Checkpoint<History<R, C, F>, C> {
        Checkpoint::from(self)
    }

    /// Returns a queue.
    #[inline]
    pub fn queue(&mut self) -> Queue<History<R, C, F>, C> {
        Queue::from(self)
    }

    /// Returns a reference to the `receiver`.
    #[inline]
    pub fn as_receiver(&self) -> &R {
        self.record.as_receiver()
    }

    /// Returns a mutable reference to the `receiver`.
    ///
    /// This method should **only** be used when doing changes that should not be able to be undone.
    #[inline]
    pub fn as_mut_receiver(&mut self) -> &mut R {
        self.record.as_mut_receiver()
    }

    /// Consumes the history, returning the `receiver`.
    #[inline]
    pub fn into_receiver(self) -> R {
        self.record.into_receiver()
    }

    /// Returns an iterator over the commands in the current branch.
    #[inline]
    pub fn commands(&self) -> impl Iterator<Item = &C> {
        self.record.commands()
    }

    /// Sets the `root`.
    #[inline]
    fn set_root(&mut self, root: usize, current: usize) {
        let old = self.root();
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
        debug_assert_ne!(self.root(), to);
        let mut dest = self.branches.remove(&to)?;
        let mut i = dest.parent.branch;
        let mut path = vec![(to, dest)];
        while i != self.root() {
            dest = self.branches.remove(&i).unwrap();
            to = i;
            i = dest.parent.branch;
            path.push((to, dest));
        }
        Some(path.into_iter().rev())
    }
}

impl<R, C: Command<R> + ToString, F: FnMut(Signal)> History<R, C, F> {
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
    #[inline]
    pub fn display(&self) -> Display<Self> {
        Display::from(self)
    }
}

impl<R: Default, C: Command<R>> Default for History<R, C> {
    #[inline]
    fn default() -> History<R, C> {
        History::new(R::default())
    }
}

impl<R, C: Command<R>, F: FnMut(Signal)> AsRef<R> for History<R, C, F> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.as_receiver()
    }
}

impl<R, C: Command<R>, F: FnMut(Signal)> AsMut<R> for History<R, C, F> {
    #[inline]
    fn as_mut(&mut self) -> &mut R {
        self.as_mut_receiver()
    }
}

impl<R, C: Command<R>> From<R> for History<R, C> {
    #[inline]
    fn from(receiver: R) -> Self {
        History::new(receiver)
    }
}

impl<R, C: Command<R>, F> From<Record<R, C, F>> for History<R, C, F> {
    #[inline]
    fn from(record: Record<R, C, F>) -> Self {
        History {
            root: 0,
            next: 1,
            saved: None,
            record,
            branches: FxHashMap::default(),
        }
    }
}

impl<R, C: Command<R> + fmt::Display, F: FnMut(Signal)> fmt::Display for History<R, C, F> {
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
    pub(crate) commands: VecDeque<Meta<C>>,
}

/// Builder for a History.
///
/// # Examples
/// ```
/// # use redo::{Command, History};
/// # struct Add(char);
/// # impl Command<String> for Add {
/// #     type Error = ();
/// #     fn apply(&mut self, s: &mut String) -> Result<(), Self::Error> { Ok(()) }
/// #     fn undo(&mut self, s: &mut String) -> Result<(), Self::Error> { Ok(()) }
/// # }
/// # fn foo() -> History<String, Add> {
/// History::builder()
///     .capacity(100)
///     .limit(100)
///     .saved(false)
///     .default()
/// # }
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct HistoryBuilder<R, C: Command<R>, F = fn(Signal)> {
    inner: RecordBuilder<R, C, F>,
}

impl<R, C: Command<R>> HistoryBuilder<R, C> {
    /// Builds the history.
    #[inline]
    pub fn build(self, receiver: impl Into<R>) -> History<R, C> {
        History {
            root: 0,
            next: 1,
            saved: None,
            record: self.inner.build(receiver),
            branches: FxHashMap::default(),
        }
    }
}

impl<R, C: Command<R>, F> HistoryBuilder<R, C, F> {
    /// Sets the capacity for the history.
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> HistoryBuilder<R, C, F> {
        self.inner = self.inner.capacity(capacity);
        self
    }

    /// Sets the `limit` for the history.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    #[inline]
    pub fn limit(mut self, limit: usize) -> HistoryBuilder<R, C, F> {
        self.inner = self.inner.limit(limit);
        self
    }

    /// Sets if the receiver is initially in a saved state.
    /// By default the receiver is in a saved state.
    #[inline]
    pub fn saved(mut self, saved: bool) -> HistoryBuilder<R, C, F> {
        self.inner = self.inner.saved(saved);
        self
    }

    /// Builds the history with the slot.
    #[inline]
    pub fn build_and_connect(self, receiver: impl Into<R>, slot: F) -> History<R, C, F> {
        History {
            root: 0,
            next: 1,
            saved: None,
            record: self.inner.build_and_connect(receiver, slot),
            branches: FxHashMap::default(),
        }
    }
}

impl<R: Default, C: Command<R>> HistoryBuilder<R, C> {
    /// Creates the history with a default `receiver`.
    #[inline]
    pub fn default(self) -> History<R, C> {
        self.build(R::default())
    }
}

impl<R: Default, C: Command<R>, F> HistoryBuilder<R, C, F> {
    /// Creates the history with a default `receiver`.
    #[inline]
    pub fn default_and_connect(self, slot: F) -> History<R, C, F> {
        self.build_and_connect(R::default(), slot)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Command, History};

    #[derive(Debug)]
    struct Add(char);

    impl Command<String> for Add {
        type Error = &'static str;

        fn apply(&mut self, receiver: &mut String) -> Result<(), Self::Error> {
            receiver.push(self.0);
            Ok(())
        }

        fn undo(&mut self, receiver: &mut String) -> Result<(), Self::Error> {
            self.0 = receiver.pop().ok_or("`receiver` is empty")?;
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
        assert_eq!(history.as_receiver(), "abcde");
        history.undo().unwrap().unwrap();
        history.undo().unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abc");
        let abcde = history.root();
        history.apply(Add('f')).unwrap();
        history.apply(Add('g')).unwrap();
        assert_eq!(history.as_receiver(), "abcfg");
        history.undo().unwrap().unwrap();
        let abcfg = history.root();
        history.apply(Add('h')).unwrap();
        history.apply(Add('i')).unwrap();
        history.apply(Add('j')).unwrap();
        assert_eq!(history.as_receiver(), "abcfhij");
        history.undo().unwrap().unwrap();
        let abcfhij = history.root();
        history.apply(Add('k')).unwrap();
        assert_eq!(history.as_receiver(), "abcfhik");
        history.undo().unwrap().unwrap();
        let abcfhik = history.root();
        history.apply(Add('l')).unwrap();
        assert_eq!(history.as_receiver(), "abcfhil");
        history.apply(Add('m')).unwrap();
        assert_eq!(history.as_receiver(), "abcfhilm");
        let abcfhilm = history.root();
        history.go_to(abcde, 2).unwrap().unwrap();
        history.apply(Add('n')).unwrap();
        history.apply(Add('o')).unwrap();
        assert_eq!(history.as_receiver(), "abno");
        history.undo().unwrap().unwrap();
        let abno = history.root();
        history.apply(Add('p')).unwrap();
        history.apply(Add('q')).unwrap();
        assert_eq!(history.as_receiver(), "abnpq");

        let abnpq = history.root();
        history.go_to(abcde, 5).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abcde");
        history.go_to(abcfg, 5).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abcfg");
        history.go_to(abcfhij, 7).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abcfhij");
        history.go_to(abcfhik, 7).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abcfhik");
        history.go_to(abcfhilm, 8).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abcfhilm");
        history.go_to(abno, 4).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abno");
        history.go_to(abnpq, 5).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abnpq");
    }
}
