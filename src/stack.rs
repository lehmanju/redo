use std::fmt::{self, Display, Formatter};
use {Command, Error};

/// The command stack.
///
/// The stack is the simplest data structure and works by pushing and
/// popping off commands that modifies the `receiver`.
/// Unlike the record, it does not have a special state that can be used for callbacks.
///
/// # Examples
/// ```
/// # use redo::*;
/// #[derive(Debug)]
/// struct Add(char);
///
/// impl Command<String> for Add {
///     type Error = &'static str;
///
///     fn exec(&mut self, s: &mut String) -> Result<(), &'static str> {
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
/// fn foo() -> Result<(), Error<String, Add>> {
///     let mut stack = Stack::default();
///
///     stack.push(Add('a'))?;
///     stack.push(Add('b'))?;
///     stack.push(Add('c'))?;
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
pub struct Stack<R, C: Command<R>> {
    commands: Vec<C>,
    receiver: R,
}

impl<R, C: Command<R>> Stack<R, C> {
    /// Creates a new stack.
    #[inline]
    pub fn new<T: Into<R>>(receiver: T) -> Stack<R, C> {
        Stack {
            commands: Vec::new(),
            receiver: receiver.into(),
        }
    }

    /// Returns the number of commands in the stack.
    #[inline]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns `true` if the stack is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Removes all commands from the stack without undoing them.
    ///
    /// This resets the stack back to its initial state while leaving the receiver unmodified.
    #[inline]
    pub fn clear(&mut self) {
        self.commands.clear();
    }

    /// Pushes the command on the stack and executes its [`exec`] method. The command is merged with
    /// the previous top command if [`merge`] does not return `None`.
    ///
    /// # Errors
    /// If an error occur when executing `redo` or merging commands, the error is returned together
    /// with the command.
    ///
    /// [`exec`]: trait.Command.html#tymethod.exec
    /// [`merge`]: trait.Command.html#method.merge
    #[inline]
    pub fn push(&mut self, mut cmd: C) -> Result<(), Error<R, C>> {
        match cmd.exec(&mut self.receiver) {
            Ok(_) => {
                let cmd = match self.commands.last_mut() {
                    Some(last) => match last.merge(cmd) {
                        Ok(_) => return Ok(()),
                        Err(cmd) => cmd,
                    },
                    None => cmd,
                };
                self.commands.push(cmd);
                Ok(())
            }
            Err(e) => Err(Error(cmd, e)),
        }
    }

    /// Calls the top commands [`undo`] method and pops it off the stack.
    /// Returns `None` if the stack is empty.
    ///
    /// # Errors
    /// If an error occur when executing `undo` the error is returned together with the command.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    #[inline]
    pub fn pop(&mut self) -> Option<Result<C, Error<R, C>>> {
        self.commands
            .pop()
            .map(|mut cmd| match cmd.undo(&mut self.receiver) {
                Ok(_) => Ok(cmd),
                Err(e) => Err(Error(cmd, e)),
            })
    }

    /// Returns a reference to the `receiver`.
    #[inline]
    pub fn as_receiver(&self) -> &R {
        &self.receiver
    }

    /// Consumes the stack, returning the `receiver`.
    #[inline]
    pub fn into_receiver(self) -> R {
        self.receiver
    }
}

impl<R: Default, C: Command<R>> Default for Stack<R, C> {
    #[inline]
    fn default() -> Stack<R, C> {
        Stack {
            commands: Default::default(),
            receiver: Default::default(),
        }
    }
}

impl<R, C: Command<R>> AsRef<R> for Stack<R, C> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.as_receiver()
    }
}

impl<R, C: Command<R>> From<R> for Stack<R, C> {
    #[inline]
    fn from(receiver: R) -> Self {
        Stack::new(receiver)
    }
}

impl<R, C: Command<R> + Display> Display for Stack<R, C> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        if let Some(cmd) = self.commands.last() {
            writeln!(f, "* {}", cmd)?;
            for cmd in self.commands.iter().rev().skip(1) {
                writeln!(f, "  {}", cmd)?;
            }
        }
        Ok(())
    }
}
