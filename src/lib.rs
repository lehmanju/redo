//! An undo-redo library with static dispatch and manual command merging.
//! It uses the [command pattern](https://en.wikipedia.org/wiki/Command_pattern)
//! where the user modifies a receiver by applying commands on it.

#![forbid(unstable_features, bad_style)]
#![deny(missing_debug_implementations, unused_import_braces, unused_qualifications, unsafe_code)]

mod group;
mod record;

use std::error;
use std::fmt::{self, Debug, Display, Formatter};

pub use group::{Group, GroupBuilder};
pub use record::{Record, RecordBuilder, Signal};

/// Base functionality for all commands.
pub trait Command<R> {
    /// The error type.
    type Error;

    /// Applies the command on the receiver and returns `Ok` if everything went fine,
    /// and `Err` if something went wrong.
    fn apply(&mut self, receiver: &mut R) -> Result<(), Self::Error>;

    /// Restores the state of the receiver as it was before the command was applied
    /// and returns `Ok` if everything went fine, and `Err` if something went wrong.
    fn undo(&mut self, receiver: &mut R) -> Result<(), Self::Error>;

    /// Reapplies the command on the receiver and return `Ok` if everything went fine,
    /// and `Err` if something went wrong.
    ///
    /// The default implementation uses the [`apply`] implementation.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    #[inline]
    fn redo(&mut self, receiver: &mut R) -> Result<(), Self::Error> {
        self.apply(receiver)
    }

    /// Used for manual merging of two commands.
    ///
    /// Returns `Ok` if commands was merged and `Err` if not.
    ///
    /// # Examples
    /// ```
    /// # use redo::*;
    /// #[derive(Debug)]
    /// struct Add(String);
    ///
    /// impl Command<String> for Add {
    ///     type Error = ();
    ///
    ///     fn apply(&mut self, s: &mut String) -> Result<(), ()> {
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
    /// fn main() -> Result<(), ()> {
    ///     let mut record = Record::default();
    ///
    ///     // The `a`, `b`, and `c` commands are merged.
    ///     record.apply(Add("a".into())).unwrap();
    ///     record.apply(Add("b".into())).unwrap();
    ///     record.apply(Add("c".into())).unwrap();
    ///     assert_eq!(record.as_receiver(), "abc");
    ///
    ///     // Calling `undo` once will undo all the merged commands.
    ///     record.undo().unwrap()?;
    ///     assert_eq!(record.as_receiver(), "");
    ///
    ///     // Calling `redo` once will redo all the merged commands.
    ///     record.redo().unwrap()?;
    ///     assert_eq!(record.into_receiver(), "abc");
    ///
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    fn merge(&mut self, cmd: Self) -> Result<(), Self> where Self: Sized {
        Err(cmd)
    }
}

/// An error which holds the command that caused it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Error<R, C: Command<R>>(pub C, pub C::Error);

impl<R, C: Command<R>> Display for Error<R, C> where C::Error: Display {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{error}", error = self.1)
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
