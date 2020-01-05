use crate::{Command, Entry, History, Queue, Record, Result, Signal, Timeline};
use alloc::collections::VecDeque;
use alloc::vec::Vec;

/// A checkpoint wrapper.
///
/// Wraps a record or history and gives it checkpoint functionality.
/// This allows the record or history to cancel all changes made since creating the checkpoint.
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
/// #         self.0 = s.pop().ok_or("`s` is empty")?;
/// #         Ok(())
/// #     }
/// # }
/// # fn main() -> redo::Result<Add> {
/// let mut record = Record::default();
/// let mut cp = record.checkpoint();
/// cp.apply(Add('a'))?;
/// cp.apply(Add('b'))?;
/// cp.apply(Add('c'))?;
/// assert_eq!(cp.target(), "abc");
/// cp.cancel()?;
/// assert_eq!(record.target(), "");
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct Checkpoint<'a, T: Timeline> {
    inner: &'a mut T,
    actions: Vec<Action<T::Command>>,
}

impl<'a, T: Timeline> Checkpoint<'a, T> {
    /// Returns a checkpoint.
    pub fn new(inner: &'a mut T) -> Checkpoint<'a, T> {
        Checkpoint {
            inner,
            actions: Vec::new(),
        }
    }

    /// Reserves capacity for at least `additional` more commands in the checkpoint.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    pub fn reserve(&mut self, additional: usize) {
        self.actions.reserve(additional);
    }

    /// Returns the capacity of the checkpoint.
    pub fn capacity(&self) -> usize {
        self.actions.capacity()
    }

    /// Shrinks the capacity of the checkpoint as much as possible.
    pub fn shrink_to_fit(&mut self) {
        self.actions.shrink_to_fit();
    }

    /// Returns the number of commands in the checkpoint.
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Returns `true` if the checkpoint is empty.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Calls the `undo` method.
    pub fn undo(&mut self) -> Option<Result<T::Command>> {
        let undo = self.inner.undo();
        if let Some(Ok(_)) = undo {
            self.actions.push(Action::Undo);
        }
        undo
    }

    /// Calls the `redo` method.
    pub fn redo(&mut self) -> Option<Result<T::Command>> {
        let redo = self.inner.redo();
        if let Some(Ok(_)) = redo {
            self.actions.push(Action::Redo);
        }
        redo
    }

    /// Commits the changes and consumes the checkpoint.
    pub fn commit(self) {}
}

impl<C: Command, F: FnMut(Signal)> Checkpoint<'_, Record<C, F>> {
    /// Calls the [`apply`] method.
    ///
    /// [`apply`]: struct.Record.html#method.apply
    pub fn apply(&mut self, command: C) -> Result<C> {
        let saved = self.inner.saved;
        let (_, commands) = self.inner.__apply(Entry::from(command))?;
        self.actions.push(Action::Apply(saved, commands));
        Ok(())
    }

    /// Calls the [`go_to`] method.
    ///
    /// [`go_to`]: struct.Record.html#method.go_to
    pub fn go_to(&mut self, current: usize) -> Option<Result<C>> {
        let old = self.inner.current();
        let go_to = self.inner.go_to(current);
        if let Some(Ok(_)) = go_to {
            self.actions.push(Action::GoTo(0, old));
        }
        go_to
    }

    /// Calls the [`extend`] method.
    ///
    /// [`extend`]: struct.Record.html#method.extend
    pub fn extend(&mut self, commands: impl IntoIterator<Item = C>) -> Result<C> {
        for command in commands {
            self.apply(command)?;
        }
        Ok(())
    }

    /// Cancels the changes and consumes the checkpoint.
    ///
    /// # Errors
    /// If an error occur when canceling the changes, the error is returned
    /// and the remaining commands are not canceled.
    pub fn cancel(self) -> Result<C> {
        for action in self.actions.into_iter().rev() {
            match action {
                Action::Apply(saved, mut commands) => {
                    self.inner.undo().unwrap()?;
                    self.inner.commands.pop_back();
                    self.inner.commands.append(&mut commands);
                    self.inner.saved = saved;
                }
                Action::Branch(_, _) => unreachable!(),
                Action::Undo => self.inner.redo().unwrap()?,
                Action::Redo => self.inner.undo().unwrap()?,
                Action::GoTo(_, current) => self.inner.go_to(current).unwrap()?,
            }
        }
        Ok(())
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<Record<C, F>> {
        self.inner.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<Record<C, F>> {
        self.inner.checkpoint()
    }

    /// Returns a reference to the `target`.
    pub fn target(&self) -> &C::Target {
        self.inner.target()
    }

    /// Returns a mutable reference to the `target`.
    ///
    /// This method should **only** be used when doing changes that should not be able to be undone.
    pub fn target_mut(&mut self) -> &mut C::Target {
        self.inner.target_mut()
    }
}

impl<C: Command, F: FnMut(Signal)> Checkpoint<'_, History<C, F>> {
    /// Calls the [`apply`] method.
    ///
    /// [`apply`]: struct.History.html#method.apply
    pub fn apply(&mut self, command: C) -> Result<C> {
        let branch = self.inner.branch();
        let current = self.inner.current();
        self.inner.apply(command)?;
        self.actions.push(Action::Branch(branch, current));
        Ok(())
    }

    /// Calls the [`go_to`] method.
    ///
    /// [`go_to`]: struct.History.html#method.go_to
    pub fn go_to(&mut self, branch: usize, current: usize) -> Option<Result<C>> {
        let root = self.inner.branch();
        let old = self.inner.current();
        let go_to = self.inner.go_to(branch, current);
        if let Some(Ok(_)) = go_to {
            self.actions.push(Action::GoTo(root, old));
        }
        go_to
    }

    /// Calls the [`extend`] method.
    ///
    /// [`extend`]: struct.History.html#method.extend
    pub fn extend(&mut self, commands: impl IntoIterator<Item = C>) -> Result<C> {
        for command in commands {
            self.apply(command)?;
        }
        Ok(())
    }

    /// Cancels the changes and consumes the checkpoint.
    ///
    /// # Errors
    /// If an error occur when canceling the changes, the error is returned
    /// and the remaining commands are not canceled.
    pub fn cancel(self) -> Result<C> {
        for action in self.actions.into_iter().rev() {
            match action {
                Action::Apply(_, _) => unreachable!(),
                Action::Branch(branch, current) => {
                    let root = self.inner.branch();
                    self.inner.go_to(branch, current).unwrap()?;
                    if root == branch {
                        self.inner.record.commands.pop_back();
                    } else {
                        self.inner.branches.remove(&root).unwrap();
                    }
                }
                Action::Undo => self.inner.redo().unwrap()?,
                Action::Redo => self.inner.undo().unwrap()?,
                Action::GoTo(branch, current) => self.inner.go_to(branch, current).unwrap()?,
            }
        }
        Ok(())
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<History<C, F>> {
        self.inner.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<History<C, F>> {
        self.inner.checkpoint()
    }

    /// Returns a reference to the `target`.
    pub fn target(&self) -> &C::Target {
        self.inner.target()
    }

    /// Returns a mutable reference to the `target`.
    ///
    /// This method should **only** be used when doing changes that should not be able to be undone.
    pub fn target_mut(&mut self) -> &mut C::Target {
        self.inner.target_mut()
    }
}

impl<C: Command, F: FnMut(Signal)> Timeline for Checkpoint<'_, Record<C, F>> {
    type Command = C;

    fn apply(&mut self, command: C) -> Result<C> {
        self.apply(command)
    }

    fn undo(&mut self) -> Option<Result<C>> {
        self.undo()
    }

    fn redo(&mut self) -> Option<Result<C>> {
        self.redo()
    }
}

impl<C: Command, F: FnMut(Signal)> Timeline for Checkpoint<'_, History<C, F>> {
    type Command = C;

    fn apply(&mut self, command: C) -> Result<C> {
        self.apply(command)
    }

    fn undo(&mut self) -> Option<Result<C>> {
        self.undo()
    }

    fn redo(&mut self) -> Option<Result<C>> {
        self.redo()
    }
}

impl<'a, T: Timeline> From<&'a mut T> for Checkpoint<'a, T> {
    fn from(inner: &'a mut T) -> Self {
        Checkpoint::new(inner)
    }
}

/// An action that can be applied to a Record or History.
#[derive(Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
enum Action<C> {
    Apply(Option<usize>, VecDeque<Entry<C>>),
    Branch(usize, usize),
    Undo,
    Redo,
    GoTo(usize, usize),
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
            self.0 = s.pop().ok_or("`s` is empty")?;
            Ok(())
        }
    }

    #[test]
    fn commit() {
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
    fn cancel() {
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
    fn saved() {
        let mut record = Record::default();
        record.apply(Add('a')).unwrap();
        record.apply(Add('b')).unwrap();
        record.apply(Add('c')).unwrap();
        record.set_saved(true);
        record.undo().unwrap().unwrap();
        record.undo().unwrap().unwrap();
        record.undo().unwrap().unwrap();
        let mut cp = record.checkpoint();
        cp.apply(Add('d')).unwrap();
        cp.apply(Add('e')).unwrap();
        cp.apply(Add('f')).unwrap();
        assert_eq!(cp.target(), "def");
        cp.cancel().unwrap();
        assert_eq!(record.target(), "");
        record.redo().unwrap().unwrap();
        record.redo().unwrap().unwrap();
        record.redo().unwrap().unwrap();
        assert!(record.is_saved());
        assert_eq!(record.target(), "abc");
    }
}
