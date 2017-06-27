//! An undo/redo library with static dispatch, state handling and manual command merging.
//!
//! # About
//! It uses the [Command Pattern] where the user implements the `Command` trait for a command.
//!
//! The `Stack` has two states, clean and dirty. The stack is clean when no more commands can
//! be redone, otherwise it is dirty. When it's state changes to either dirty or clean, it calls
//! the user defined method set in [`on_state_change`]. This is useful if you want to trigger some
//! event when the state changes, eg. enabling and disabling undo and redo buttons.
//!
//! It also supports merging of commands by implementing the [`merge`][manual] method for a command.
//!
//! # Redo vs Undo
//! |                 | Redo             | Undo            |
//! |-----------------|------------------|-----------------|
//! | Dispatch        | [Static]         | [Dynamic]       |
//! | State Handling  | Yes              | Yes             |
//! | Command Merging | [Manual][manual] | [Auto][auto]    |
//!
//! Both supports command merging but `undo` will automatically merge commands with the same id
//! while in `redo` you need to implement the merge method yourself.
//!
//! # Examples
//! ```
//! use std::borrow::Borrow;
//! use redo::{self, Command, Stack};
//!
//! #[derive(Clone, Copy)]
//! struct Pop(Option<u8>);
//!
//! impl Command<Vec<u8>> for Pop {
//!     type Err = &'static str;
//!
//!     fn redo(&mut self, vec: &mut Vec<u8>) -> redo::Result<&'static str> {
//!         self.0 = vec.pop();
//!         Ok(())
//!     }
//!
//!     fn undo(&mut self, vec: &mut Vec<u8>) -> redo::Result<&'static str> {
//!         let e = self.0.ok_or("`e` is invalid")?;
//!         vec.push(e);
//!         Ok(())
//!     }
//! }
//!
//! fn foo() -> redo::Result<&'static str> {
//!     let mut stack = Stack::new(vec![1, 2, 3]);
//!     let cmd = Pop(None);
//!
//!     stack.push(cmd)?;
//!     stack.push(cmd)?;
//!     stack.push(cmd)?;
//!
//!     assert!({
//!         let stack: &Vec<_> = stack.borrow();
//!         stack.is_empty()
//!     });
//!
//!     stack.undo()?;
//!     stack.undo()?;
//!     stack.undo()?;
//!
//!     assert_eq!(stack.into_receiver(), vec![1, 2, 3]);
//!     Ok(())
//! }
//! # foo().unwrap();
//! ```
//!
//! [Command Pattern]: https://en.wikipedia.org/wiki/Command_pattern
//! [`on_state_change`]: struct.StackBuilder.html#method.on_state_change
//! [`merge`]: trait.Command.html#method.merge
//! [auto]: https://docs.rs/undo/0.8.1/undo/trait.UndoCmd.html#method.id
//! [manual]: trait.Command.html#method.merge
//! [Static]: https://doc.rust-lang.org/stable/book/trait-objects.html#static-dispatch
//! [Dynamic]: https://doc.rust-lang.org/stable/book/trait-objects.html#dynamic-dispatch
//! [`undo`]: https://crates.io/crates/undo

#![forbid(unstable_features, bad_style)]
#![deny(missing_docs,
        missing_debug_implementations,
        unused_import_braces,
        unused_qualifications)]

extern crate fnv;

mod group;
mod stack;

pub use group::Group;
pub use stack::Stack;

use std::result;

/// A key for a `Stack` in a `Group`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Key(u32);

/// A specialized `Result` that does not carry any data on success.
pub type Result<E> = result::Result<(), E>;

/// Trait that defines the functionality of a command.
///
/// Every command needs to implement this trait to be able to be used with the `Stack`.
pub trait Command<T> {
    /// The error type.
    type Err;

    /// Executes the desired command and returns `Ok` if everything went fine, and `Err` if
    /// something went wrong.
    fn redo(&mut self, receiver: &mut T) -> Result<Self::Err>;

    /// Restores the state as it was before [`redo`] was called and returns `Ok` if everything
    /// went fine, and `Err` if something went wrong.
    ///
    /// [`redo`]: trait.Command.html#tymethod.redo
    fn undo(&mut self, receiver: &mut T) -> Result<Self::Err>;

    /// Used for manual merging of two `Command`s.
    ///
    /// Returns `Some(Ok)` if the merging was successful, `Some(Err)` if something went wrong when
    /// trying to merge, and `None` if it did not try to merge.
    /// This method is always called by the [`push`] method in `Stack`, with `self` being the top
    /// command on the stack and `cmd` being the new command. If `None` is returned from this
    /// method, `cmd` will be pushed on the stack as normal. However, if the return value is
    /// `Some(x)` it will not push the command on to the stack since either it was merged or an
    /// error has occurred, and then the stack returns the `x` value.
    ///
    /// Default implementation returns `None`.
    ///
    /// [`push`]: struct.Stack.html#method.push
    #[inline]
    fn merge(&mut self, _: &Self) -> Option<Result<Self::Err>> {
        None
    }
}
