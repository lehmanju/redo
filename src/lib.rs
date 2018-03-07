//! An undo-redo library with static dispatch and manual command merging.
//! It uses the [command pattern](https://en.wikipedia.org/wiki/Command_pattern)
//! where the user modifies a receiver by applying commands on it.
//!
//! The library has currently two data structures that can be used to modify the receiver:
//!
//! * A stack that can push and pop commands to modify the receiver.
//! * A record that can roll the state of the receiver forwards and backwards.

#![forbid(unstable_features, bad_style)]
#![deny(missing_debug_implementations, unused_import_braces, unused_qualifications, unsafe_code)]

mod group;
mod record;
mod stack;

use std::error;
use std::fmt::{self, Debug, Display, Formatter};

pub use group::{Group, GroupBuilder};
pub use record::{Commands, Record, RecordBuilder, Signal};
pub use stack::Stack;

/// Base functionality for all commands.
pub trait Command<R> {
    /// The error type.
    type Error;

    /// Executes the desired command and returns `Ok` if everything went fine, and `Err` if
    /// something went wrong.
    fn exec(&mut self, receiver: &mut R) -> Result<(), Self::Error>;

    /// Restores the state as it was before [`exec`] was called and returns `Ok` if everything
    /// went fine, and `Err` if something went wrong.
    ///
    /// [`exec`]: trait.Command.html#tymethod.exec
    fn undo(&mut self, receiver: &mut R) -> Result<(), Self::Error>;

    /// Used for manual merging of two commands.
    ///
    /// Returns `Ok` if commands was merged and `Err` if not.
    ///
    /// # Examples
    /// ```
    /// use redo::{Command, Error, Stack};
    ///
    /// #[derive(Debug)]
    /// struct Add(String);
    ///
    /// impl Command<String> for Add {
    ///     type Error = ();
    ///
    ///     fn exec(&mut self, s: &mut String) -> Result<(), ()> {
    ///         s.push_str(&self.0);
    ///         Ok(())
    ///     }
    ///
    ///     fn undo(&mut self, s: &mut String) -> Result<(), ()> {
    ///         let len = s.len() - self.0.len();
    ///         s.truncate(len);
    ///         Ok(())
    ///     }
    ///
    ///     fn merge(&mut self, Add(s): Self) -> Result<(), Self> {
    ///         self.0.push_str(&s);
    ///         Ok(())
    ///     }
    /// }
    ///
    /// fn foo() -> Result<(), Error<String, Add>> {
    ///     let mut stack = Stack::default();
    ///
    ///     // "a", "b", and "c" are merged.
    ///     stack.push(Add("a".into()))?;
    ///     stack.push(Add("b".into()))?;
    ///     stack.push(Add("c".into()))?;
    ///     assert_eq!(stack.len(), 1);
    ///     assert_eq!(stack.as_receiver(), "abc");
    ///
    ///     // Calling `pop` once will undo all merged commands.
    ///     let abc = stack.pop().unwrap()?;
    ///     assert_eq!(stack.as_receiver(), "");
    ///
    ///     // Pushing the merged commands back on the stack will redo them.
    ///     stack.push(abc)?;
    ///     assert_eq!(stack.into_receiver(), "abc");
    ///
    ///     Ok(())
    /// }
    /// # foo().unwrap();
    /// ```
    #[inline]
    fn merge(&mut self, cmd: Self) -> Result<(), Self> where Self: Sized {
        Err(cmd)
    }
}

/// The error type.
///
/// The error contains the error itself and the command that caused the error.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Error<R, C: Command<R>>(pub C, pub C::Error);

impl<R, C: Command<R>> Display for Error<R, C> where C::Error: Display {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.1)
    }
}

impl<R, C: Command<R>> error::Error for Error<R, C>
where
    R: Debug,
    C: Debug,
    C::Error: error::Error,
{
    #[inline]
    fn description(&self) -> &str {
        self.1.description()
    }

    #[inline]
    fn cause(&self) -> Option<&error::Error> {
        self.1.cause()
    }
}
