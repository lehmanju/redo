//! # Redo
//! An undo/redo library.
//!
//! Redo does not use [dynamic dispatch] which means it is <u>faster</u> than [undo]
//! but less flexible.
//!
//! [dynamic dispatch]: https://doc.rust-lang.org/stable/book/trait-objects.html#dynamic-dispatch
//! [undo]: https://crates.io/crates/undo

mod stack;

pub use stack::RedoStack;

/// Every command needs to implement the `RedoCmd` trait to be able to be used with the `RedoStack`.
pub trait RedoCmd {
    /// Executes the desired command.
    fn redo(&mut self);

    /// Restores the state as it was before [`redo`] was called.
    ///
    /// [`redo`]: trait.RedoCmd.html#tymethod.redo
    fn undo(&mut self);
}
