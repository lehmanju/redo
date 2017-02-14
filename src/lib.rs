//! An undo/redo library.
//!
//! # About
//! It uses the [Command Pattern] where the user implements the `RedoCmd` trait for a command.
//!
//! The `RedoStack` has two states, clean and dirty. The stack is clean when no more commands can
//! be redone, otherwise it is dirty. The stack will notice when it's state changes to either dirty
//! or clean, and call the user defined methods set in [`on_clean`] and [`on_dirty`].
//! This is useful if you want to trigger some event when the state changes, eg. enabling and
//! disabling buttons in an ui.
//!
//! It also supports merging of commands by implementing the [`merge`] method for a command.
//!
//! # Redo vs Undo
//! |                 | Redo         | Undo            |
//! |-----------------|--------------|-----------------|
//! | Dispatch        | Static       | Dynamic         |
//! | State Handling  | Yes          | Yes             |
//! | Command Merging | Yes (manual) | Yes (automatic) |
//!
//! `redo` uses [static dispatch] instead of [dynamic dispatch] to store the commands, which means
//! it should be faster than [`undo`]. However, this means that you can only store one type of
//! command in a `RedoStack` at a time. Both supports state handling and command merging but
//! `undo` will automatically merge commands with the same id, while in `redo` you need to implement
//! the merge method yourself. If state handling is not needed, it can be disabled by setting the
//! `no_state` feature flag.
//!
//! I recommend using `undo` by default and to use `redo` when performance is important.
//! They have similar API, so it should be easy to switch between them if necessary.
//!
//! # Examples
//! ```
//! use redo::{self, RedoCmd, RedoStack};
//!
//! #[derive(Clone, Copy)]
//! struct PopCmd {
//!     vec: *mut Vec<i32>,
//!     e: Option<i32>,
//! }
//!
//! impl RedoCmd for PopCmd {
//!     type Err = ();
//!
//!     fn redo(&mut self) -> redo::Result<()> {
//!         self.e = unsafe {
//!             let ref mut vec = *self.vec;
//!             vec.pop()
//!         };
//!         Ok(())
//!     }
//!
//!     fn undo(&mut self) -> redo::Result<()> {
//!         unsafe {
//!             let ref mut vec = *self.vec;
//!             let e = self.e.ok_or(())?;
//!             vec.push(e);
//!         }
//!         Ok(())
//!     }
//! }
//!
//! fn foo() -> redo::Result<()> {
//!     let mut vec = vec![1, 2, 3];
//!     let mut stack = RedoStack::new();
//!     let cmd = PopCmd { vec: &mut vec, e: None };
//!
//!     stack.push(cmd)?;
//!     stack.push(cmd)?;
//!     stack.push(cmd)?;
//!
//!     assert!(vec.is_empty());
//!
//!     stack.undo()?;
//!     stack.undo()?;
//!     stack.undo()?;
//!
//!     assert_eq!(vec.len(), 3);
//!     Ok(())
//! }
//! # foo().unwrap();
//! ```
//!
//! *An unsafe implementation of `redo` and `undo` is used in examples since it is less verbose and
//! makes the examples easier to follow.*
//!
//! [Command Pattern]: https://en.wikipedia.org/wiki/Command_pattern
//! [`on_clean`]: struct.RedoStack.html#method.on_clean
//! [`on_dirty`]: struct.RedoStack.html#method.on_dirty
//! [static dispatch]: https://doc.rust-lang.org/stable/book/trait-objects.html#static-dispatch
//! [dynamic dispatch]: https://doc.rust-lang.org/stable/book/trait-objects.html#dynamic-dispatch
//! [`undo`]: https://crates.io/crates/undo
//! [`merge`]: trait.RedoCmd.html#method.merge

#![deny(missing_docs,
        missing_debug_implementations,
        unstable_features,
        unused_import_braces,
        unused_qualifications)]

extern crate fnv;

mod group;
mod stack;

pub use group::RedoGroup;
pub use stack::RedoStack;

use std::result;

type Key = u32;

/// An unique id for an `RedoStack`.
#[derive(Debug)]
pub struct Id(Key);

/// A specialized `Result` that does not carry any data on success.
pub type Result<E> = result::Result<(), E>;

/// Trait that defines the functionality of a command.
///
/// Every command needs to implement this trait to be able to be used with the `RedoStack`.
pub trait RedoCmd {
    /// The error type.
    type Err;

    /// Executes the desired command and returns `Ok` if everything went fine, and `Err` if
    /// something went wrong.
    fn redo(&mut self) -> Result<Self::Err>;

    /// Restores the state as it was before [`redo`] was called and returns `Ok` if everything
    /// went fine, and `Err` if something went wrong.
    ///
    /// [`redo`]: trait.RedoCmd.html#tymethod.redo
    fn undo(&mut self) -> Result<Self::Err>;

    /// Used for manual merging of two `RedoCmd`s.
    ///
    /// Returns `Some(Ok)` if the merging was successful, `Some(Err)` if something went wrong when
    /// trying to merge, and `None` if it did not try to merge.
    /// This method is always called by the [`push`] method in `RedoStack`, with `self` being the top
    /// command on the stack and `cmd` being the new command. If `None` is returned from this
    /// method, `cmd` will be pushed on the stack as normal. However, if the return value is
    /// `Some(x)` it will not push the command on to the stack since either it was merged or an
    /// error has occurred, and then the stack returns the `x` value.
    ///
    /// Default implementation returns `None`.
    ///
    /// # Examples
    /// ```
    /// use redo::{self, RedoCmd, RedoStack};
    ///
    /// #[derive(Debug)]
    /// struct TxtCmd {
    ///     txt: String,
    ///     c: char,
    /// }
    ///
    /// impl TxtCmd {
    ///     fn new(c: char) -> Self {
    ///         TxtCmd { c: c, txt: String::new() }
    ///     }
    /// }
    ///
    /// impl RedoCmd for TxtCmd {
    ///     type Err = ();
    ///
    ///     fn redo(&mut self) -> redo::Result<()> {
    ///         Ok(())
    ///     }
    ///
    ///     fn undo(&mut self) -> redo::Result<()> {
    ///         Ok(())
    ///     }
    ///
    ///     fn merge(&mut self, cmd: &Self) -> Option<redo::Result<()>> {
    ///         // Merge cmd if not a space.
    ///         if cmd.c != ' ' {
    ///             self.txt.push(cmd.c);
    ///             Some(Ok(()))
    ///         } else {
    ///             None
    ///         }
    ///     }
    /// }
    ///
    /// fn foo() -> redo::Result<()> {
    ///     let mut stack = RedoStack::new();
    ///     stack.push(TxtCmd::new('a'))?;
    ///     stack.push(TxtCmd::new('b'))?;
    ///     stack.push(TxtCmd::new('c'))?; // 'a', 'b' and 'c' is merged.
    ///     stack.push(TxtCmd::new(' '))?;
    ///     stack.push(TxtCmd::new('d'))?; // ' ' and 'd' is merged.
    ///
    ///     println!("{:#?}", stack);
    ///     Ok(())
    /// }
    /// # foo().unwrap();
    /// ```
    ///
    /// Output:
    ///
    /// ```txt
    /// RedoStack {
    ///     stack: [
    ///         TxtCmd {
    ///             txt: "bc",
    ///             c: 'a'
    ///         },
    ///         TxtCmd {
    ///             txt: "d",
    ///             c: ' '
    ///         }
    ///     ],
    ///     idx: 2,
    ///     limit: None
    /// }
    /// ```
    ///
    /// [`push`]: struct.RedoStack.html#method.push
    #[allow(unused_variables)]
    #[inline]
    fn merge(&mut self, cmd: &Self) -> Option<Result<Self::Err>> {
        None
    }
}
