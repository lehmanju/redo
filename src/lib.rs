//! An undo/redo library with static dispatch and manual command merging.
//! It uses the [Command Pattern] where the user modifies a receiver by
//! applying `Command`s on it.
//!
//! The library has currently two data structures that can be used to modify the receiver:
//!
//! * A simple `Stack` that pushes and pops commands to modify the receiver.
//! * A more advanced `Record` that can roll the state of the receiver forwards and backwards.
//!
//! [Command Pattern]: https://en.wikipedia.org/wiki/Command_pattern

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
