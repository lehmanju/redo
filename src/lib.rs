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
//! Commands can be merged using the [`merge`] method.
//! When two commands are merged, undoing and redoing them are done in a single step.
//!
//! [command pattern]: https://en.wikipedia.org/wiki/Command_pattern
//! [Record]: struct.Record.html
//! [History]: struct.History.html
//! [`merge`]: trait.Command.html#method.merge

#![doc(html_root_url = "https://docs.rs/redo/0.28.0")]
#![deny(
    bad_style,
    bare_trait_objects,
    missing_debug_implementations,
    missing_docs,
    unused_import_braces,
    unused_qualifications,
    unsafe_code,
    unstable_features
)]

#[macro_use]
extern crate bitflags;
#[cfg(feature = "chrono")]
extern crate chrono;
extern crate colored;
extern crate rustc_hash;
#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;

mod display;
mod history;
mod record;
mod signal;

#[cfg(feature = "chrono")]
use chrono::{DateTime, Utc};
use std::{error::Error as StdError, fmt};

pub use display::Display;
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
    ///     fn merge(&mut self, Add(s): Self) -> Merge<Self> {
    ///         self.0.push_str(&s);
    ///         Merge::Yes
    ///     }
    /// }
    ///
    /// fn main() -> Result<(), Error<String, Add>> {
    ///     let mut record = Record::default();
    ///     // The `a`, `b`, and `c` commands are merged.
    ///     record.apply(Add("a".into()))?;
    ///     record.apply(Add("b".into()))?;
    ///     record.apply(Add("c".into()))?;
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
    fn merge(&mut self, command: Self) -> Merge<Self>
    where
        Self: Sized,
    {
        Merge::No(command)
    }
}

/// The result of merging two commands.
#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum Merge<C> {
    /// The commands have been merged.
    Yes,
    /// The commands have not been merged.
    No(C),
    /// The two commands cancels each other out. This removes both commands.
    Annul,
}

/// A position in a history tree.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub(crate) struct At {
    pub(crate) branch: usize,
    pub(crate) cursor: usize,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug)]
struct Meta<C> {
    command: C,
    #[cfg(feature = "chrono")]
    timestamp: DateTime<Utc>,
}

impl<C> From<C> for Meta<C> {
    #[inline]
    fn from(command: C) -> Self {
        Meta {
            command,
            #[cfg(feature = "chrono")]
            timestamp: Utc::now(),
        }
    }
}

impl<R, C: Command<R>> Command<R> for Meta<C> {
    type Error = C::Error;

    #[inline]
    fn apply(&mut self, receiver: &mut R) -> Result<(), <Self as Command<R>>::Error> {
        self.command.apply(receiver)
    }

    #[inline]
    fn undo(&mut self, receiver: &mut R) -> Result<(), <Self as Command<R>>::Error> {
        self.command.undo(receiver)
    }

    #[inline]
    fn redo(&mut self, receiver: &mut R) -> Result<(), <Self as Command<R>>::Error> {
        self.command.redo(receiver)
    }

    #[inline]
    fn merge(&mut self, command: Self) -> Merge<Self>
    where
        Self: Sized,
    {
        match self.command.merge(command.command) {
            Merge::Yes => Merge::Yes,
            Merge::No(command) => Merge::No(Meta::from(command)),
            Merge::Annul => Merge::Annul,
        }
    }
}

impl<C: fmt::Display> fmt::Display for Meta<C> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.command as &dyn fmt::Display).fmt(f)
    }
}

/// An error which holds the command that caused it.
pub struct Error<R, C: Command<R>> {
    meta: Meta<C>,
    error: C::Error,
}

impl<R, C: Command<R>> Error<R, C> {
    /// Returns a new error.
    #[inline]
    fn new(meta: Meta<C>, error: C::Error) -> Error<R, C> {
        Error { meta, error }
    }
}

impl<R, C: Command<R>> Error<R, C> {
    /// Returns a reference to the command that caused the error.
    #[inline]
    pub fn command(&self) -> &C {
        &self.meta.command
    }

    /// Returns the command that caused the error.
    #[inline]
    pub fn into_command(self) -> C {
        self.meta.command
    }
}

impl<R, C: Command<R> + fmt::Debug> fmt::Debug for Error<R, C>
where
    C::Error: fmt::Debug,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Error")
            .field("meta", &self.meta)
            .field("error", &self.error)
            .finish()
    }
}

impl<R, C: Command<R>> fmt::Display for Error<R, C>
where
    C::Error: fmt::Display,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.error as &dyn fmt::Display).fmt(f)
    }
}

impl<R, C: Command<R>> StdError for Error<R, C>
where
    C: fmt::Debug,
    C::Error: StdError,
{
    #[inline]
    fn description(&self) -> &str {
        self.error.description()
    }

    #[inline]
    fn cause(&self) -> Option<&dyn StdError> {
        self.error.cause()
    }
}
