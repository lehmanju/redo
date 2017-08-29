//! An undo/redo library with static dispatch and manual command merging.
//! It uses the [Command Pattern](https://en.wikipedia.org/wiki/Command_pattern)
//! where the user modifies a receiver by applying `Command`s on it.
//!
//! The library has currently two data structures that can be used to modify the receiver:
//!
//! * A simple `Stack` that pushes and pops commands to modify the receiver.
//! * A `Record` that can roll the state of the receiver forwards and backwards.

#![forbid(unstable_features, bad_style)]
#![deny(missing_debug_implementations,
        unused_import_braces,
        unused_qualifications)]

pub mod record;
mod stack;

use std::error;
use std::fmt::{self, Debug, Display, Formatter};

pub use record::Record;
pub use stack::Stack;

/// Base functionality for all commands.
pub trait Command<R> {
    /// The error type.
    type Err;

    /// Executes the desired command and returns `Ok` if everything went fine, and `Err` if
    /// something went wrong.
    fn redo(&mut self, receiver: &mut R) -> Result<(), Self::Err>;

    /// Restores the state as it was before [`redo`] was called and returns `Ok` if everything
    /// went fine, and `Err` if something went wrong.
    ///
    /// [`redo`]: trait.Command.html#tymethod.redo
    fn undo(&mut self, receiver: &mut R) -> Result<(), Self::Err>;

    /// Used for manual merging of two `Command`s.
    ///
    /// Returns `Ok` if commands was merged and `Err(cmd)` if not.
    #[inline]
    fn merge(&mut self, cmd: Self) -> Result<(), Self>
        where Self: Sized
    {
        Err(cmd)
    }
}

/// An error kind that holds the error and the command that caused the error.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Error<R, C: Command<R>>(pub C, pub C::Err);

impl<R, C: Command<R>> Display for Error<R, C>
    where C::Err: Display
{
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.1)
    }
}

impl<R, C: Command<R>> error::Error for Error<R, C>
    where R: Debug,
          C: Debug,
          C::Err: error::Error
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
