use fnv::{FnvHashMap, FnvHashSet};
use std::collections::VecDeque;
use std::fmt;
use {Command, Display, Error, Record, RecordBuilder, Signal};

/// A history of commands.
///
/// A history works like the [Record] but also provides branching, like [vim]'s undo-tree.
///
/// # Examples
/// ```
/// # use std::error;
/// # use redo::*;
/// #[derive(Debug)]
/// struct Add(char);
///
/// impl Command<String> for Add {
///     type Error = Box<dyn error::Error>;
///
///     fn apply(&mut self, s: &mut String) -> Result<(), Self::Error> {
///         s.push(self.0);
///         Ok(())
///     }
///
///     fn undo(&mut self, s: &mut String) -> Result<(), Self::Error> {
///         self.0 = s.pop().ok_or("`s` is empty")?;
///         Ok(())
///     }
/// }
///
/// fn main() -> Result<(), Error<String, Add>> {
///     let mut history = History::default();
///     history.apply(Add('a'))?;
///     history.apply(Add('b'))?;
///     history.apply(Add('c'))?;
///     assert_eq!(history.as_receiver(), "abc");
///
///     let root = history.root();
///     history.go_to(root, 1).unwrap()?;
///     assert_eq!(history.as_receiver(), "a");
///
///     let abc = history.apply(Add('f'))?.unwrap();
///     history.apply(Add('g'))?;
///     assert_eq!(history.as_receiver(), "afg");
///
///     history.go_to(abc, 3).unwrap()?;
///     assert_eq!(history.as_receiver(), "abc");
///     Ok(())
/// }
/// ```
///
/// [Record]: struct.Record.html
/// [Vim]: https://www.vim.org/
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug)]
pub struct History<R, C: Command<R>> {
    root: usize,
    next: usize,
    pub(crate) saved: Option<At>,
    pub(crate) record: Record<R, C>,
    pub(crate) branches: FnvHashMap<usize, Branch<C>>,
}

impl<R, C: Command<R>> History<R, C> {
    /// Returns a new history.
    #[inline]
    pub fn new(receiver: impl Into<R>) -> History<R, C> {
        History {
            root: 0,
            next: 1,
            saved: None,
            record: Record::new(receiver),
            branches: FnvHashMap::default(),
        }
    }

    /// Returns a builder for a history.
    #[inline]
    pub fn builder() -> HistoryBuilder<R, C> {
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
        for cursor in 0..diff {
            self.remove_children(At {
                branch: root,
                cursor,
            });
        }
        for branch in self
            .branches
            .values_mut()
            .filter(|branch| branch.parent.branch == root)
        {
            branch.parent.cursor -= diff;
        }
        limit
    }

    /// Sets how the signal should be handled when the state changes.
    #[inline]
    pub fn set_signal(&mut self, f: impl FnMut(Signal) + Send + Sync + 'static) {
        self.record.set_signal(f);
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

    /// Returns the current branch.
    #[inline]
    pub fn root(&self) -> usize {
        self.root
    }

    /// Returns the position of the current command.
    #[inline]
    pub fn cursor(&self) -> usize {
        self.record.cursor()
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

        if let Some(ref mut f) = self.record.signal {
            f(Signal::Branch { old, new: 0 })
        }
    }

    /// Pushes the command to the top of the history and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned together with the command.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    #[inline]
    pub fn apply(&mut self, cmd: C) -> Result<Option<usize>, Error<R, C>> {
        let cursor = self.cursor();
        let saved = self.record.saved.filter(|&saved| saved > cursor);
        let (merged, commands) = self.record.__apply(cmd)?;
        // Check if the limit has been reached.
        if !merged && cursor == self.cursor() {
            let root = self.root();
            self.remove_children(At {
                branch: root,
                cursor: 0,
            });
            for branch in self
                .branches
                .values_mut()
                .filter(|branch| branch.parent.branch == root)
            {
                branch.parent.cursor -= 1;
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
                        cursor,
                    },
                    commands,
                },
            );
            self.record.saved = self.record.saved.or(saved);
            self.set_root(new, cursor);
            match (self.record.saved, saved, self.saved) {
                (Some(_), None, None) | (None, None, Some(_)) => self.swap_saved(new, old, cursor),
                (Some(_), Some(_), None) => self.swap_saved(old, new, cursor),
                (None, None, None) => (),
                _ => unreachable!(),
            }
            if let Some(ref mut f) = self.record.signal {
                f(Signal::Branch { old, new })
            }
            Ok(Some(old))
        } else {
            Ok(None)
        }
    }

    /// Calls the [`undo`] method for the active command and sets the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned together with the command.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    #[inline]
    #[must_use]
    pub fn undo(&mut self) -> Option<Result<(), Error<R, C>>> {
        self.record.undo()
    }

    /// Calls the [`redo`] method for the active command and sets the next one as the
    /// new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the error is returned together with the command.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    #[must_use]
    pub fn redo(&mut self) -> Option<Result<(), Error<R, C>>> {
        self.record.redo()
    }

    /// Repeatedly calls [`undo`] or [`redo`] until the command in `branch` at `cursor` is reached.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned together with the command.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    #[must_use]
    pub fn go_to(&mut self, branch: usize, cursor: usize) -> Option<Result<usize, Error<R, C>>> {
        let root = self.root;
        if root == branch {
            return self.record.go_to(cursor).map(|r| r.map(|_| root));
        }

        // Walk the path from `start` to `dest`.
        for (new, branch) in self.create_path(branch)? {
            let old = self.root();
            // Walk to `branch.cursor` either by undoing or redoing.
            if let Err(err) = self.record.go_to(branch.parent.cursor).unwrap() {
                return Some(Err(err));
            }
            // Apply the commands in the branch and move older commands into their own branch.
            for cmd in branch.commands {
                let cursor = self.cursor();
                let saved = self.record.saved.filter(|&saved| saved > cursor);
                let commands = match self.record.__apply(cmd) {
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
                                cursor,
                            },
                            commands,
                        },
                    );
                    self.record.saved = self.record.saved.or(saved);
                    self.set_root(new, cursor);
                    match (self.record.saved, saved, self.saved) {
                        (Some(_), None, None) | (None, None, Some(_)) => {
                            self.swap_saved(new, old, cursor);
                        }
                        (Some(_), Some(_), None) => self.swap_saved(old, new, cursor),
                        (None, None, None) => (),
                        _ => unreachable!(),
                    }
                }
            }
        }

        if let Err(err) = self.record.go_to(cursor)? {
            return Some(Err(err));
        }

        if let Some(ref mut f) = self.record.signal {
            f(Signal::Branch {
                old: root,
                new: self.root,
            });
        }
        Some(Ok(root))
    }

    /// Jump directly to the command in `branch` at `cursor` and executes its [`undo`] or [`redo`] method.
    ///
    /// This method can be used if the commands store the whole state of the receiver,
    /// and does not require the commands in between to be called to get the same result.
    /// Use [`go_to`] otherwise.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned together with the command.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    /// [`redo`]: trait.Command.html#method.redo
    /// [`go_to`]: struct.History.html#method.go_to
    #[inline]
    #[must_use]
    pub fn jump_to(&mut self, branch: usize, cursor: usize) -> Option<Result<usize, Error<R, C>>> {
        let root = self.root;
        if root == branch {
            return self.record.jump_to(cursor).map(|r| r.map(|_| root));
        }

        // Jump the path from `start` to `dest`.
        for (new, mut branch) in self.create_path(branch)? {
            let old = self.root();
            // Jump to `branch.cursor` either by undoing or redoing.
            if let Err(err) = self.record.jump_to(branch.parent.cursor).unwrap() {
                return Some(Err(err));
            }

            let cursor = self.cursor();
            let saved = self.record.saved.filter(|&saved| saved > cursor);
            let mut commands = self.record.commands.split_off(cursor);
            self.record.commands.append(&mut branch.commands);
            // Handle new branch.
            if !commands.is_empty() {
                self.branches.insert(
                    self.root,
                    Branch {
                        parent: At {
                            branch: new,
                            cursor,
                        },
                        commands,
                    },
                );
                self.record.saved = self.record.saved.or(saved);
                self.set_root(new, cursor);
                match (self.record.saved, saved, self.saved) {
                    (Some(_), None, None) | (None, None, Some(_)) => {
                        self.swap_saved(new, old, cursor);
                    }
                    (Some(_), Some(_), None) => self.swap_saved(old, new, cursor),
                    (None, None, None) => (),
                    _ => unreachable!(),
                }
            }
        }

        if let Err(err) = self.record.jump_to(cursor)? {
            return Some(Err(err));
        }

        if let Some(ref mut f) = self.record.signal {
            f(Signal::Branch {
                old: root,
                new: self.root,
            });
        }
        Some(Ok(root))
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
    fn set_root(&mut self, root: usize, cursor: usize) {
        let old = self.root;
        self.root = root;
        debug_assert_ne!(old, root);
        // Handle the child branches.
        for branch in self
            .branches
            .values_mut()
            .filter(|branch| branch.parent.branch == old && branch.parent.cursor <= cursor)
        {
            branch.parent.branch = root;
        }
    }

    /// Swap the saved state if needed.
    #[inline]
    fn swap_saved(&mut self, old: usize, new: usize, cursor: usize) {
        debug_assert_ne!(old, new);
        if let Some(At { cursor: saved, .. }) = self
            .saved
            .filter(|at| at.branch == new && at.cursor <= cursor)
        {
            self.saved = None;
            self.record.saved = Some(saved);
            if let Some(ref mut f) = self.record.signal {
                f(Signal::Saved(true));
            }
        } else if let Some(saved) = self.record.saved {
            self.saved = Some(At {
                branch: old,
                cursor: saved,
            });
            self.record.saved = None;
            if let Some(ref mut f) = self.record.signal {
                f(Signal::Saved(false));
            }
        }
    }

    /// Remove all children of the command at position `at`.
    #[inline]
    fn remove_children(&mut self, at: At) {
        let mut dead = FnvHashSet::default();
        // We need to check if any of the branches had the removed node as root.
        let mut children = self
            .branches
            .iter()
            .filter(|&(&id, child)| child.parent == at && dead.insert(id))
            .map(|(&id, _)| id)
            .collect::<Vec<_>>();
        // Add all the children of dead branches so they are removed too.
        while let Some(parent) = children.pop() {
            for (&id, _) in self
                .branches
                .iter()
                .filter(|&(&id, child)| child.parent.branch == parent && dead.insert(id))
            {
                children.push(id);
            }
        }
        // Remove all dead branches.
        for id in dead {
            self.branches.remove(&id);
            self.saved = self.saved.filter(|saved| saved.branch != id);
        }
    }

    /// Create a path between the current branch and the `to` branch.
    #[inline]
    #[must_use]
    fn create_path(&mut self, to: usize) -> Option<Vec<(usize, Branch<C>)>> {
        let mut path = vec![];
        let dest = self.branches.remove(&to)?;
        let mut i = dest.parent.branch;
        while i != self.root() {
            let branch = self.branches.remove(&i).unwrap();
            let j = i;
            i = branch.parent.branch;
            path.push((j, branch));
        }
        path.push((to, dest));
        Some(path)
    }
}

impl<R, C: Command<R> + ToString> History<R, C> {
    /// Returns the string of the command which will be undone in the next call to [`undo`].
    ///
    /// [`undo`]: struct.History.html#method.undo
    #[inline]
    #[must_use]
    pub fn to_undo_string(&self) -> Option<String> {
        self.record.to_undo_string()
    }

    /// Returns the string of the command which will be redone in the next call to [`redo`].
    ///
    /// [`redo`]: struct.History.html#method.redo
    #[inline]
    #[must_use]
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

impl<R, C: Command<R>> AsRef<R> for History<R, C> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.as_receiver()
    }
}

impl<R, C: Command<R>> AsMut<R> for History<R, C> {
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

impl<R, C: Command<R>> From<Record<R, C>> for History<R, C> {
    #[inline]
    fn from(record: Record<R, C>) -> Self {
        History {
            root: 0,
            next: 1,
            saved: None,
            record,
            branches: FnvHashMap::default(),
        }
    }
}

impl<R, C: Command<R> + fmt::Display> fmt::Display for History<R, C> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.display() as &dyn fmt::Display).fmt(f)
    }
}

/// A branch in the history.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug)]
pub(crate) struct Branch<C> {
    pub(crate) parent: At,
    pub(crate) commands: VecDeque<C>,
}

/// The position in the tree.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub(crate) struct At {
    pub(crate) branch: usize,
    pub(crate) cursor: usize,
}

/// Builder for a History.
#[derive(Debug)]
pub struct HistoryBuilder<R, C: Command<R>> {
    inner: RecordBuilder<R, C>,
}

impl<R, C: Command<R>> HistoryBuilder<R, C> {
    /// Sets the capacity for the history.
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> HistoryBuilder<R, C> {
        self.inner = self.inner.capacity(capacity);
        self
    }

    /// Sets the `limit` for the history.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    #[inline]
    pub fn limit(mut self, limit: usize) -> HistoryBuilder<R, C> {
        self.inner = self.inner.limit(limit);
        self
    }

    /// Sets if the receiver is initially in a saved state.
    /// By default the receiver is in a saved state.
    #[inline]
    pub fn saved(mut self, saved: bool) -> HistoryBuilder<R, C> {
        self.inner = self.inner.saved(saved);
        self
    }

    /// Decides how the signal should be handled when the state changes.
    /// By default the history does not handle any signals.
    #[inline]
    pub fn signal(mut self, f: impl FnMut(Signal) + Send + Sync + 'static) -> HistoryBuilder<R, C> {
        self.inner = self.inner.signal(f);
        self
    }

    /// Builds the history.
    #[inline]
    pub fn build(self, receiver: impl Into<R>) -> History<R, C> {
        History {
            root: 0,
            next: 1,
            saved: None,
            record: self.inner.build(receiver),
            branches: FnvHashMap::default(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[derive(Debug)]
    struct Add(char);

    impl Command<String> for Add {
        type Error = Box<dyn Error>;

        fn apply(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error>> {
            receiver.push(self.0);
            Ok(())
        }

        fn undo(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error>> {
            self.0 = receiver.pop().ok_or("`receiver` is empty")?;
            Ok(())
        }
    }

    #[derive(Debug)]
    struct JumpAdd(char, String);

    impl From<char> for JumpAdd {
        fn from(c: char) -> JumpAdd {
            JumpAdd(c, Default::default())
        }
    }

    impl Command<String> for JumpAdd {
        type Error = Box<dyn Error>;

        fn apply(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error>> {
            self.1 = receiver.clone();
            receiver.push(self.0);
            Ok(())
        }

        fn undo(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error>> {
            *receiver = self.1.clone();
            Ok(())
        }

        fn redo(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error>> {
            *receiver = self.1.clone();
            receiver.push(self.0);
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
        assert!(history.apply(Add('a')).unwrap().is_none());
        assert!(history.apply(Add('b')).unwrap().is_none());
        assert!(history.apply(Add('c')).unwrap().is_none());
        assert!(history.apply(Add('d')).unwrap().is_none());
        assert!(history.apply(Add('e')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abcde");
        history.undo().unwrap().unwrap();
        history.undo().unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abc");
        let abcde = history.apply(Add('f')).unwrap().unwrap();
        assert!(history.apply(Add('g')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abcfg");
        history.undo().unwrap().unwrap();
        let abcfg = history.apply(Add('h')).unwrap().unwrap();
        assert!(history.apply(Add('i')).unwrap().is_none());
        assert!(history.apply(Add('j')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abcfhij");
        history.undo().unwrap().unwrap();
        let abcfhij = history.apply(Add('k')).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abcfhik");
        history.undo().unwrap().unwrap();
        let abcfhik = history.apply(Add('l')).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abcfhil");
        assert!(history.apply(Add('m')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abcfhilm");
        let abcfhilm = history.go_to(abcde, 2).unwrap().unwrap();
        history.apply(Add('n')).unwrap().unwrap();
        assert!(history.apply(Add('o')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abno");
        history.undo().unwrap().unwrap();
        let abno = history.apply(Add('p')).unwrap().unwrap();
        assert!(history.apply(Add('q')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abnpq");

        let abnpq = history.go_to(abcde, 5).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abcde");
        assert_eq!(history.go_to(abcfg, 5).unwrap().unwrap(), abcde);
        assert_eq!(history.as_receiver(), "abcfg");
        assert_eq!(history.go_to(abcfhij, 7).unwrap().unwrap(), abcfg);
        assert_eq!(history.as_receiver(), "abcfhij");
        assert_eq!(history.go_to(abcfhik, 7).unwrap().unwrap(), abcfhij);
        assert_eq!(history.as_receiver(), "abcfhik");
        assert_eq!(history.go_to(abcfhilm, 8).unwrap().unwrap(), abcfhik);
        assert_eq!(history.as_receiver(), "abcfhilm");
        assert_eq!(history.go_to(abno, 4).unwrap().unwrap(), abcfhilm);
        assert_eq!(history.as_receiver(), "abno");
        assert_eq!(history.go_to(abnpq, 5).unwrap().unwrap(), abno);
        assert_eq!(history.as_receiver(), "abnpq");
    }

    #[test]
    fn jump_to() {
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
        assert!(history.apply(JumpAdd::from('a')).unwrap().is_none());
        assert!(history.apply(JumpAdd::from('b')).unwrap().is_none());
        assert!(history.apply(JumpAdd::from('c')).unwrap().is_none());
        assert!(history.apply(JumpAdd::from('d')).unwrap().is_none());
        assert!(history.apply(JumpAdd::from('e')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abcde");
        history.undo().unwrap().unwrap();
        history.undo().unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abc");
        let abcde = history.apply(JumpAdd::from('f')).unwrap().unwrap();
        assert!(history.apply(JumpAdd::from('g')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abcfg");
        history.undo().unwrap().unwrap();
        let abcfg = history.apply(JumpAdd::from('h')).unwrap().unwrap();
        assert!(history.apply(JumpAdd::from('i')).unwrap().is_none());
        assert!(history.apply(JumpAdd::from('j')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abcfhij");
        history.undo().unwrap().unwrap();
        let abcfhij = history.apply(JumpAdd::from('k')).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abcfhik");
        history.undo().unwrap().unwrap();
        let abcfhik = history.apply(JumpAdd::from('l')).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abcfhil");
        assert!(history.apply(JumpAdd::from('m')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abcfhilm");
        let abcfhilm = history.go_to(abcde, 2).unwrap().unwrap();
        history.apply(JumpAdd::from('n')).unwrap().unwrap();
        assert!(history.apply(JumpAdd::from('o')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abno");
        history.undo().unwrap().unwrap();
        let abno = history.apply(JumpAdd::from('p')).unwrap().unwrap();
        assert!(history.apply(JumpAdd::from('q')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abnpq");

        let abnpq = history.jump_to(abcde, 5).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abcde");
        assert_eq!(history.jump_to(abcfg, 5).unwrap().unwrap(), abcde);
        assert_eq!(history.as_receiver(), "abcfg");
        assert_eq!(history.jump_to(abcfhij, 7).unwrap().unwrap(), abcfg);
        assert_eq!(history.as_receiver(), "abcfhij");
        assert_eq!(history.jump_to(abcfhik, 7).unwrap().unwrap(), abcfhij);
        assert_eq!(history.as_receiver(), "abcfhik");
        assert_eq!(history.jump_to(abcfhilm, 8).unwrap().unwrap(), abcfhik);
        assert_eq!(history.as_receiver(), "abcfhilm");
        assert_eq!(history.jump_to(abno, 4).unwrap().unwrap(), abcfhilm);
        assert_eq!(history.as_receiver(), "abno");
        assert_eq!(history.jump_to(abnpq, 5).unwrap().unwrap(), abno);
        assert_eq!(history.as_receiver(), "abnpq");
    }
}
