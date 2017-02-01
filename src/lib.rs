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

    /// Used for manual merging of two `RedoCmd`s.
    ///
    /// Should return `Some(())` if a merge happened and `None` if not. Default implementation
    /// returns `None`.
    ///
    /// # Examples
    /// ```
    /// # #![allow(dead_code)]
    /// # use redo::{RedoCmd, RedoStack};
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
    ///     fn redo(&mut self) {}
    ///
    ///     fn undo(&mut self) {}
    ///
    ///     fn merge(&mut self, cmd: &Self) -> Option<()> {
    ///         // Merge cmd if not a space.
    ///         if cmd.c != ' ' {
    ///             self.txt.push(cmd.c);
    ///             Some(())
    ///         } else {
    ///             None
    ///         }
    ///     }
    /// }
    ///
    /// let mut stack = RedoStack::new();
    /// stack.push(TxtCmd::new('a'));
    /// stack.push(TxtCmd::new('b'));
    /// stack.push(TxtCmd::new('c')); // 'a', 'b' and 'c' is merged.
    /// stack.push(TxtCmd::new(' '));
    /// stack.push(TxtCmd::new('d')); // ' ' and 'd' is merged.
    ///
    /// println!("{:#?}", stack);
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
    #[allow(unused_variables)]
    #[inline]
    fn merge(&mut self, cmd: &Self) -> Option<()> {
        None
    }
}
