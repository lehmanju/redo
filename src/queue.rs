use {Checkpoint, Command, Error, History, Record};

/// An action that can be applied to a Record or History.
#[derive(Debug)]
enum Action<C> {
    Apply(C),
    Undo,
    Redo,
    GoTo(usize, usize),
}

/// A command queue wrapper.
///
/// Wraps a Record or History and gives it batch queue functionality.
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
///     let mut record = Record::default();
///     {
///         let mut queue = record.queue();
///         queue.apply(Add('a'));
///         queue.apply(Add('b'));
///         queue.apply(Add('c'));
///         assert_eq!(queue.as_receiver(), "");
///         queue.commit()?;
///     }
///     assert_eq!(record.as_receiver(), "abc");
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct Queue<'a, T: 'a, C> {
    inner: &'a mut T,
    queue: Vec<Action<C>>,
}

impl<'a, T: 'a, C> From<&'a mut T> for Queue<'a, T, C> {
    #[inline]
    fn from(inner: &'a mut T) -> Self {
        Queue {
            inner,
            queue: Vec::new(),
        }
    }
}

impl<'a, T: 'a, C> Queue<'a, T, C> {
    /// Queues an `apply` action.
    #[inline]
    pub fn apply(&mut self, command: C) {
        self.queue.push(Action::Apply(command));
    }

    /// Queues an `undo` action.
    #[inline]
    pub fn undo(&mut self) {
        self.queue.push(Action::Undo);
    }

    /// Queues a `redo` action.
    #[inline]
    pub fn redo(&mut self) {
        self.queue.push(Action::Redo);
    }

    /// Cancels the queued actions.
    #[inline]
    pub fn cancel(self) {}
}

impl<'a, R, C: Command<R>> Queue<'a, Record<R, C>, C> {
    /// Queues a `go_to` action.
    #[inline]
    pub fn go_to(&mut self, cursor: usize) {
        self.queue.push(Action::GoTo(0, cursor));
    }

    /// Applies the actions that is queued.
    ///
    /// # Errors
    /// If an error occurs, it stops applying the actions and returns the error.
    #[inline]
    pub fn commit(self) -> Result<(), Error<R, C>> {
        for action in self.queue {
            match action {
                Action::Apply(command) => self.inner.apply(command)?,
                Action::Undo => {
                    if let Some(Err(error)) = self.inner.undo() {
                        return Err(error);
                    }
                }
                Action::Redo => {
                    if let Some(Err(error)) = self.inner.redo() {
                        return Err(error);
                    }
                }
                Action::GoTo(_, cursor) => {
                    if let Some(Err(error)) = self.inner.go_to(cursor) {
                        return Err(error);
                    }
                }
            }
        }
        Ok(())
    }

    /// Returns a checkpoint.
    #[inline]
    pub fn checkpoint(&mut self) -> Checkpoint<Record<R, C>, C> {
        self.inner.checkpoint()
    }

    /// Returns a queue.
    #[inline]
    pub fn queue(&mut self) -> Queue<Record<R, C>, C> {
        self.inner.queue()
    }

    /// Returns a reference to the `receiver`.
    #[inline]
    pub fn as_receiver(&self) -> &R {
        self.inner.as_receiver()
    }
}

impl<'a, R, C: Command<R>> AsRef<R> for Queue<'a, Record<R, C>, C> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.inner.as_ref()
    }
}

impl<'a, R, C: Command<R>> Queue<'a, History<R, C>, C> {
    /// Queues a `go_to` action.
    #[inline]
    pub fn go_to(&mut self, branch: usize, cursor: usize) {
        self.queue.push(Action::GoTo(branch, cursor));
    }

    /// Applies the actions that is queued.
    ///
    /// # Errors
    /// If an error occurs, it stops applying the actions and returns the error.
    #[inline]
    pub fn commit(self) -> Result<(), Error<R, C>> {
        for action in self.queue {
            match action {
                Action::Apply(command) => self.inner.apply(command)?,
                Action::Undo => {
                    if let Some(Err(error)) = self.inner.undo() {
                        return Err(error);
                    }
                }
                Action::Redo => {
                    if let Some(Err(error)) = self.inner.redo() {
                        return Err(error);
                    }
                }
                Action::GoTo(branch, cursor) => {
                    if let Some(Err(error)) = self.inner.go_to(branch, cursor) {
                        return Err(error);
                    }
                }
            }
        }
        Ok(())
    }

    /// Returns a checkpoint.
    #[inline]
    pub fn checkpoint(&mut self) -> Checkpoint<History<R, C>, C> {
        self.inner.checkpoint()
    }

    /// Returns a queue.
    #[inline]
    pub fn queue(&mut self) -> Queue<History<R, C>, C> {
        self.inner.queue()
    }

    /// Returns a reference to the `receiver`.
    #[inline]
    pub fn as_receiver(&self) -> &R {
        self.inner.as_receiver()
    }
}

impl<'a, R, C: Command<R>> AsRef<R> for Queue<'a, History<R, C>, C> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.inner.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use std::error;
    use {Command, Record};

    #[derive(Debug)]
    struct Add(char);

    impl Command<String> for Add {
        type Error = Box<dyn error::Error>;

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
    fn commit() {
        let mut record = Record::default();
        {
            let mut queue = record.queue();
            queue.redo();
            queue.redo();
            queue.redo();
            {
                let mut queue = queue.queue();
                queue.undo();
                queue.undo();
                queue.undo();
                {
                    let mut queue = queue.queue();
                    queue.apply(Add('a'));
                    queue.apply(Add('b'));
                    queue.apply(Add('c'));
                    assert_eq!(queue.as_receiver(), "");
                    queue.commit().unwrap();
                }
                assert_eq!(queue.as_receiver(), "abc");
                queue.commit().unwrap();
            }
            assert_eq!(queue.as_receiver(), "");
            queue.commit().unwrap();
        }
        assert_eq!(record.as_receiver(), "abc");
    }
}
