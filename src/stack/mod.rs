// mod stacks;

// pub use self::stacks::Stacks;
use Command;

/// A stack of commands.
///
/// The `Stack` is the simplest data structure and works by pushing and
/// popping off `Command`s that modifies the `receiver`.
///
/// # Examples
/// ```
/// use redo::{Command, Stack};
///
/// #[derive(Debug)]
/// struct Push(char);
///
/// impl Command<String> for Push {
///     type Err = &'static str;
///
///     fn redo(&mut self, s: &mut String) -> Result<(), &'static str> {
///         s.push(self.0);
///         Ok(())
///     }
///
///     fn undo(&mut self, s: &mut String) -> Result<(), &'static str> {
///         self.0 = s.pop().ok_or("`String` is unexpectedly empty")?;
///         Ok(())
///     }
/// }
///
/// fn foo() -> Result<(), (Push, &'static str)> {
///     let mut stack = Stack::default();
///
///     stack.push(Push('a'))?;
///     stack.push(Push('b'))?;
///     stack.push(Push('c'))?;
///
///     assert_eq!(stack.as_receiver(), "abc");
///
///     let c = stack.pop().unwrap()?;
///     let b = stack.pop().unwrap()?;
///     let a = stack.pop().unwrap()?;
///
///     assert_eq!(stack.as_receiver(), "");
///
///     stack.push(a)?;
///     stack.push(b)?;
///     stack.push(c)?;
///
///     assert_eq!(stack.into_receiver(), "abc");
///
///     Ok(())
/// }
/// # foo().unwrap();
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Stack<T, C: Command<T>> {
    commands: Vec<C>,
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

    /// Creates a new `Stack` with the given `capacity`.
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

    /// Returns the number of `Command`s in the `Stack`.
    #[inline]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns `true` if the `Stack` is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Returns a reference to the `receiver`.
    #[inline]
    pub fn as_receiver(&self) -> &T {
        &self.receiver
    }

    /// Consumes the `Stack`, returning the `receiver`.
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
    /// [`redo`]: ../trait.Command.html#tymethod.redo
    /// [`merge`]: ../trait.Command.html#method.merge
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

    /// Calls the top commands [`undo`] method and pops it off the stack.
    /// Returns `None` if the stack is empty.
    ///
    /// # Errors
    /// If an error occur when executing `undo` the error is returned together with the `Command`.
    ///
    /// [`undo`]: ../trait.Command.html#tymethod.undo
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

impl<T: Default, C: Command<T>> Default for Stack<T, C> {
    #[inline]
    fn default() -> Stack<T, C> {
        Stack::new(Default::default())
    }
}

impl<T, C: Command<T>> AsRef<T> for Stack<T, C> {
    #[inline]
    fn as_ref(&self) -> &T {
        self.as_receiver()
    }
}
