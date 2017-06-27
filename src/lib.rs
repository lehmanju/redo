//! An undo/redo library with static dispatch and manual command merging.
//! It uses the [Command Pattern] where the user implements the `Command` trait for a command.
//!
//! # Redo vs Undo
//! |                 | Redo             | Undo            |
//! |-----------------|------------------|-----------------|
//! | Dispatch        | [Static]         | [Dynamic]       |
//! | Command Merging | [Manual][manual] | [Auto][auto]    |
//!
//! Both supports command merging but [`undo`] will automatically merge commands with the same id
//! while in `redo` you need to implement the merge method yourself.
//!
//! # Examples
//! ```
//! # #![allow(unused_variables)]
//! use redo::Command;
//! use redo::stack::Stack;
//!
//! #[derive(Debug)]
//! struct Push(char);
//!
//! impl Command<String> for Push {
//!     type Err = &'static str;
//!
//!     fn redo(&mut self, s: &mut String) -> Result<(), &'static str> {
//!         s.push(self.0);
//!         Ok(())
//!     }
//!
//!     fn undo(&mut self, s: &mut String) -> Result<(), &'static str> {
//!         self.0 = s.pop().ok_or("`String` is unexpectedly empty")?;
//!         Ok(())
//!     }
//! }
//!
//! fn foo() -> Result<(), (Push, &'static str)> {
//!     let mut stack = Stack::default();
//!
//!     stack.push(Push('a'))?;
//!     stack.push(Push('b'))?;
//!     stack.push(Push('c'))?;
//!
//!     assert_eq!(stack.as_receiver(), "abc");
//!
//!     let c = stack.pop().unwrap()?;
//!     let b = stack.pop().unwrap()?;
//!     let a = stack.pop().unwrap()?;
//!
//!     assert_eq!(stack.into_receiver(), "");
//!     Ok(())
//! }
//! # foo().unwrap();
//! ```
//!
//! [Command Pattern]: https://en.wikipedia.org/wiki/Command_pattern
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

pub mod record;
pub mod stack;

/// A key used in the `Group`s.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Key(u32);

/// Trait that defines the functionality of a command.
///
/// Every command needs to implement this trait to be able to be used with the `Stack`.
pub trait Command<T> {
    /// The error type.
    type Err;

    /// Executes the desired command and returns `Ok` if everything went fine, and `Err` if
    /// something went wrong.
    fn redo(&mut self, receiver: &mut T) -> Result<(), Self::Err>;

    /// Restores the state as it was before [`redo`] was called and returns `Ok` if everything
    /// went fine, and `Err` if something went wrong.
    ///
    /// [`redo`]: trait.Command.html#tymethod.redo
    fn undo(&mut self, receiver: &mut T) -> Result<(), Self::Err>;

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
    fn merge(&mut self, _: &Self) -> Option<Result<(), Self::Err>> {
        None
    }
}
