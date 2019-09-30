//! Provides undo-redo functionality with static dispatch and manual command merging.
//!
//! It is an implementation of the command pattern, where all modifications are done
//! by creating objects of commands that applies the modifications. All commands knows
//! how to undo the changes it applies, and by using the provided data structures
//! it is easy to apply, undo, and redo changes made to a receiver.
//! Both linear and non-linear undo-redo functionality is provided through
//! the [Record] and [History] data structures.
//! This library provides more or less the same functionality as the [undo] library
//! but is more focused on performance and control instead of ease of use.
//!
//! # Contents
//!
//! * [Command] provides the base functionality for all commands.
//! * [Record] provides linear undo-redo functionality.
//! * [History] provides non-linear undo-redo functionality that allows you to jump between different branches.
//! * [Queue] wraps a [Record] or [History] and extends them with queue functionality.
//! * [Checkpoint] wraps a [Record] or [History] and extends them with checkpoint functionality.
//! * Configurable display formatting is provided when the `display` feature is enabled.
//! * Time stamps and time travel is provided when the `chrono` feature is enabled.
//! * Serialization and deserialization is provided when the `serde` feature is enabled.
//!
//! # Concepts
//!
//! * Commands can be merged into a single command by implementing the [merge] method on the command.
//!   This allows smaller commands to be used to build more complex operations, or smaller incremental changes to be
//!   merged into larger changes that can be undone and redone in a single step.
//! * The receiver can be marked as being saved to disk and the data-structures can track the saved state and tell the user
//!   when it changes.
//! * The amount of changes being tracked can be configured by the user so only the `n` most recent changes are stored.
//!
//! # Examples
//!
//! Add this to `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! redo = "0.37"
//! ```
//!
//! And this to `main.rs`:
//!
//! ```
//! use redo::{Command, Record};
//!
//! struct Add(char);
//!
//! impl Command<String> for Add {
//!     type Error = &'static str;
//!
//!     fn apply(&mut self, s: &mut String) -> Result<(), Self::Error> {
//!         s.push(self.0);
//!         Ok(())
//!     }
//!
//!     fn undo(&mut self, s: &mut String) -> Result<(), Self::Error> {
//!         self.0 = s.pop().ok_or("`s` is empty")?;
//!         Ok(())
//!     }
//! }
//!
//! fn main() -> Result<(), &'static str> {
//!     let mut record = Record::default();
//!     record.apply(Add('a'))?;
//!     record.apply(Add('b'))?;
//!     record.apply(Add('c'))?;
//!     assert_eq!(record.as_receiver(), "abc");
//!     record.undo().unwrap()?;
//!     record.undo().unwrap()?;
//!     record.undo().unwrap()?;
//!     assert_eq!(record.as_receiver(), "");
//!     record.redo().unwrap()?;
//!     record.redo().unwrap()?;
//!     record.redo().unwrap()?;
//!     assert_eq!(record.as_receiver(), "abc");
//!     Ok(())
//! }
//! ```
//!
//! [Command]: trait.Command.html
//! [Record]: struct.Record.html
//! [Timeline]: struct.Timeline.html
//! [History]: struct.History.html
//! [Queue]: struct.Queue.html
//! [Checkpoint]: struct.Checkpoint.html
//! [merge]: trait.Command.html#method.merge
//! [undo]: https://github.com/evenorog/undo

#![doc(html_root_url = "https://docs.rs/redo/latest")]
#![deny(
    bad_style,
    bare_trait_objects,
    missing_debug_implementations,
    missing_docs,
    unused_import_braces,
    unsafe_code,
    unstable_features
)]

mod checkpoint;
#[cfg(feature = "display")]
mod display;
mod history;
mod queue;
mod record;

#[cfg(feature = "chrono")]
use chrono::{DateTime, Utc};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::fmt;

#[cfg(feature = "display")]
pub use self::display::Display;
pub use self::{
    checkpoint::Checkpoint,
    history::{History, HistoryBuilder},
    queue::Queue,
    record::{Record, RecordBuilder},
};

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
    /// # use redo::{Command, Merge, Record};
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
    /// fn main() -> Result<(), ()> {
    ///     let mut record = Record::default();
    ///     // The `a`, `b`, and `c` commands are merged.
    ///     record.apply(Add("a".into()))?;
    ///     record.apply(Add("b".into()))?;
    ///     record.apply(Add("c".into()))?;
    ///     assert_eq!(record.as_receiver(), "abc");
    ///     // Calling `undo` once will undo all the merged commands.
    ///     record.undo().unwrap()?;
    ///     assert_eq!(record.as_receiver(), "");
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

    /// Says if the command is dead.
    ///
    /// A dead command will be removed the next time it becomes the current command.
    /// This can be used to remove command if for example executing it caused an error,
    /// and it needs to be removed.
    #[inline]
    fn is_dead(&self) -> bool {
        false
    }
}

/// The signal sent when the record, the history, or the receiver changes.
///
/// When one of these states changes, they will send a corresponding signal to the user.
/// For example, if the record can no longer redo any commands, it sends a `Redo(false)`
/// signal to tell the user.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum Signal {
    /// Says if the record can undo.
    ///
    /// This signal will be emitted when the records ability to undo changes.
    Undo(bool),
    /// Says if the record can redo.
    ///
    /// This signal will be emitted when the records ability to redo changes.
    Redo(bool),
    /// Says if the receiver is in a saved state.
    ///
    /// This signal will be emitted when the record enters or leaves its receivers saved state.
    Saved(bool),
    /// Says if the current command has changed.
    ///
    /// This signal will be emitted when the current command has changed. This includes
    /// when two commands have been merged, in which case `old == new`.
    Current {
        /// The old current command.
        old: usize,
        /// The new current command.
        new: usize,
    },
    /// Says if the current branch has changed.
    ///
    /// This is only emitted from `History`.
    Branch {
        /// The old root.
        old: usize,
        /// The new root.
        new: usize,
    },
}

/// The result of merging two commands.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
struct At {
    branch: usize,
    current: usize,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
struct Entry<C> {
    command: C,
    #[cfg(feature = "chrono")]
    timestamp: DateTime<Utc>,
}

impl<C> From<C> for Entry<C> {
    #[inline]
    fn from(command: C) -> Self {
        Entry {
            command,
            #[cfg(feature = "chrono")]
            timestamp: Utc::now(),
        }
    }
}

impl<R, C: Command<R>> Command<R> for Entry<C> {
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
            Merge::No(command) => Merge::No(Entry::from(command)),
            Merge::Annul => Merge::Annul,
        }
    }

    #[inline]
    fn is_dead(&self) -> bool {
        self.command.is_dead()
    }
}

impl<C: fmt::Display> fmt::Display for Entry<C> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.command as &dyn fmt::Display).fmt(f)
    }
}
