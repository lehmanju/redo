//! An undo-redo library with static dispatch and manual command merging.
//!
//! It uses the [command pattern] where the user modifies the receiver by
//! applying commands on it. Since each command knows how to undo and redo
//! the changes it applies to the receiver, the state of the receiver can
//! be rolled forwards or backwards by calling undo or redo in the correct order.
//!
//! The [Record] and [History] provides functionality to store and keep track
//! of the applied commands, and makes it easy to undo and redo changes.
//! The Record provides a stack based undo-redo functionality, while the
//! History provides a tree based undo-redo functionality where you can
//! jump between different branches.
//!
//! Commands can be merged using the [`merge!`] macro or the [`merge`] method.
//! When two commands are merged, undoing and redoing them are done in a single step.
//!
//! [command pattern]: https://en.wikipedia.org/wiki/Command_pattern
//! [Record]: struct.Record.html
//! [History]: struct.History.html
//! [`merge!`]: macro.merge.html
//! [`merge`]: trait.Command.html#method.merge

#![deny(
    bad_style,
    bare_trait_objects,
    missing_debug_implementations,
    unused_import_braces,
    unused_qualifications,
    unsafe_code,
    unstable_features
)]

extern crate fnv;
#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;

mod history;
mod merge;
mod record;
mod signal;

use std::{
    error::Error as StdError,
    fmt::{self, Debug, Display, Formatter},
};

pub use history::{History, HistoryBuilder};
pub use record::{Record, RecordBuilder};
pub use signal::Signal;

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
    /// fn main() -> Result<(), Error<String, Add>> {
    ///     let mut record = Record::default();
    ///
    ///     // The `a`, `b`, and `c` commands are merged.
    ///     record.apply(Add("a".into()))?;
    ///     record.apply(Add("b".into()))?;
    ///     record.apply(Add("c".into()))?;
    ///     assert_eq!(record.len(), 1);
    ///     assert_eq!(record.as_receiver(), "abc");
    ///
    ///     // Calling `undo` once will undo all the merged commands.
    ///     record.undo().unwrap()?;
    ///     assert_eq!(record.as_receiver(), "");
    ///
    ///     // Calling `redo` once will redo all the merged commands.
    ///     record.redo().unwrap()?;
    ///     assert_eq!(record.as_receiver(), "abc");
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    fn merge(&mut self, cmd: Self) -> Result<(), Self>
    where
        Self: Sized,
    {
        Err(cmd)
    }
}

/// An error which holds the command that caused it.
pub struct Error<R, C: Command<R>>(pub C, pub C::Error);

impl<R, C: Command<R> + Debug> Debug for Error<R, C>
where
    C::Error: Debug,
{
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_tuple("Error")
            .field(&self.0)
            .field(&self.1)
            .finish()
    }
}

impl<R, C: Command<R>> Display for Error<R, C>
where
    C::Error: Display,
{
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        (&self.1 as &dyn Display).fmt(f)
    }
}

impl<R, C: Command<R>> StdError for Error<R, C>
where
    C: Debug,
    C::Error: StdError,
{
    #[inline]
    fn description(&self) -> &str {
        self.1.description()
    }

    #[inline]
    fn cause(&self) -> Option<&dyn StdError> {
        self.1.cause()
    }
}
