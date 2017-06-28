//! An undo/redo library with static dispatch and manual command merging.
//! It uses the [Command Pattern] where the user implements the `Command` trait for a command.
//!
//! |                 | Redo             | Undo            |
//! |-----------------|------------------|-----------------|
//! | Dispatch        | [Static]         | [Dynamic]       |
//! | Command Merging | [Manual][manual] | [Auto][auto]    |
//!
//! Both supports command merging but [`undo`] will automatically merge commands with the same id
//! while in `redo` you need to implement the merge method yourself.
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

mod record;
mod stack;

pub use record::Record;
pub use stack::Stack;

// /// A key used in the `Group`s.
// #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
// pub struct Key(u32);

/// Trait that defines the functionality of a command.
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
    /// This method is called with `self` being the top command and `cmd` being the
    /// new command. If `None` is returned from this method, `cmd` will be pushed
    /// as normal. However, if the return value is `Some(x)` it will not push the command on to
    /// the stack since either it was merged or an error has occurred, and then the stack returns
    /// the `x` value.
    ///
    /// Default implementation returns `None`.
    ///
    /// [`push`]: struct.Stack.html#method.push
    #[inline]
    fn merge(&mut self, _: &Self) -> Option<Result<(), Self::Err>> {
        None
    }
}
