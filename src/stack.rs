use Command;

/// A stack of `Command`s.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Stack<T, C: Command<T>> {
    // All commands on the stack.
    commands: Vec<C>,
    // The data being operated on.
    receiver: T,
}

impl<T, C: Command<T>> Stack<T, C> {
    /// Creates a new `Stack`.
    #[inline]
    pub fn new(receiver: T) -> Stack<T, C> {
        Stack {
            commands: Vec::new(),
            receiver,
        }
    }

    /// Creates a new stack with the given `capacity`.
    #[inline]
    pub fn with_capacity(receiver: T, capacity: usize) -> Stack<T, C> {
        Stack {
            commands: Vec::with_capacity(capacity),
            receiver,
        }
    }

    /// Returns the capacity of the `Stack`.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.commands.capacity()
    }

    /// Reserves capacity for at least `additional` more commands to be inserted in the given stack.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.commands.reserve(additional);
    }

    /// Shrinks the capacity of the `Stack` as much as possible.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.commands.shrink_to_fit();
    }

    /// Returns a reference to the receiver.
    #[inline]
    pub fn as_receiver(&self) -> &T {
        &self.receiver
    }

    /// Consumes the `Stack`, returning the receiver.
    #[inline]
    pub fn into_receiver(self) -> T {
        self.receiver
    }

    /// Pushes `cmd` on the stack and executes its [`redo`] method. The command is merged with
    /// the previous top `Command` if [`merge`] does not return `None`.
    ///
    /// # Errors
    /// If an error occur when executing `redo` or merging commands, the error is returned together
    /// with the `Command`.
    ///
    /// [`redo`]: trait.Command.html#tymethod.redo
    /// [`merge`]: trait.Command.html#method.merge
    #[inline]
    pub fn push(&mut self, mut cmd: C) -> Result<(), (C, C::Err)> {
        if let Err(e) = cmd.redo(&mut self.receiver) {
            return Err((cmd, e));
        }
        match self.commands.last_mut().and_then(|last| last.merge(&cmd)) {
            Some(x) => x.map_err(|e| (cmd, e))?,
            None => self.commands.push(cmd),
        }
        Ok(())
    }

    /// Calls the [`undo`] method for the active `Command` and sets the previous `Command` as the
    /// new active one. Returns `None` if the stack is empty.
    ///
    /// # Errors
    /// If an error occur when executing `undo` the error is returned together with the `Command`.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    #[inline]
    pub fn pop(&mut self) -> Option<Result<C, (C, C::Err)>> {
        let mut cmd = match self.commands.pop() {
            Some(cmd) => cmd,
            None => return None,
        };
        match cmd.undo(&mut self.receiver) {
            Ok(_) => Some(Ok(cmd)),
            Err(e) => Some(Err((cmd, e))),
        }
    }
}

impl<T, C: Command<T>> AsRef<T> for Stack<T, C> {
    #[inline]
    fn as_ref(&self) -> &T {
        self.as_receiver()
    }
}
