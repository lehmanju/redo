//! **High-level undo-redo functionality.**
//!
//! It is an implementation of the command pattern, where all modifications are done
//! by creating objects of commands that applies the modifications. All commands knows
//! how to undo the changes it applies, and by using the provided data structures
//! it is easy to apply, undo, and redo changes made to a target.

#![doc(html_root_url = "https://docs.rs/redo")]
#![deny(missing_docs)]
#![forbid(unsafe_code)]

#[cfg(feature = "chrono")]
use chrono_crate::{DateTime, TimeZone};
#[cfg(feature = "serde")]
use serde_crate::{Deserialize, Serialize};
use undo::History as Inner;
pub use undo::{Command, Merge, Result, Signal};

/// The history structs maintains the target and the commands that has benn applied to the target.
///
/// # Examples
/// ```
/// # use redo::{Command, History};
/// # struct Add(char);
/// # impl From<char> for Add {
/// #     fn from(c: char) -> Self { Add(c) }
/// # }
/// # impl Command for Add {
/// #     type Target = String;
/// #     type Error = &'static str;
/// #     fn apply(&mut self, s: &mut String) -> redo::Result<Add> {
/// #         s.push(self.0);
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self, s: &mut String) -> redo::Result<Add> {
/// #         self.0 = s.pop().unwrap();
/// #         Ok(())
/// #     }
/// # }
/// # fn main() -> redo::Result<Add> {
/// let mut history = History::<Add>::default();
/// history.apply('a')?;
/// history.apply('b')?;
/// history.apply('c')?;
/// assert_eq!(history.target(), "abc");
/// history.undo()?;
/// history.undo()?;
/// history.undo()?;
/// assert_eq!(history.target(), "");
/// history.redo()?;
/// history.redo()?;
/// history.redo()?;
/// assert_eq!(history.target(), "abc");
/// # Ok(())
/// # }
/// ```
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(
        crate = "serde_crate",
        bound(
            serialize = "C: Command + Serialize, C::Target: Serialize",
            deserialize = "C: Command + Deserialize<'de>, C::Target: Deserialize<'de>"
        )
    )
)]
pub struct History<C: Command> {
    inner: Inner<C>,
    target: C::Target,
}

impl<C: Command> History<C> {
    /// Returns a new history.
    pub fn new(target: C::Target) -> History<C> {
        History {
            inner: Inner::new(),
            target,
        }
    }

    /// Reserves capacity for at least `additional` more commands.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    pub fn reserve(&mut self, additional: usize) {
        self.inner.reserve(additional);
    }

    /// Returns the capacity of the history.
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// Shrinks the capacity of the history as much as possible.
    pub fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit();
    }

    /// Returns the number of commands in the current branch of the history.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the current branch of the history is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the limit of the history.
    pub fn limit(&self) -> usize {
        self.inner.limit()
    }

    /// Sets how the signal should be handled when the state changes.
    ///
    /// The previous slot is returned if it exists.
    pub fn connect(&mut self, slot: impl FnMut(Signal) + 'static) -> Option<impl FnMut(Signal)> {
        self.inner.connect(Box::new(slot))
    }

    /// Removes and returns the slot if it exists.
    pub fn disconnect(&mut self) -> Option<impl FnMut(Signal)> {
        self.inner.disconnect()
    }

    /// Returns `true` if the target is in a saved state, `false` otherwise.
    pub fn is_saved(&self) -> bool {
        self.inner.is_saved()
    }

    /// Returns `true` if the history can undo.
    pub fn can_undo(&self) -> bool {
        self.inner.can_undo()
    }

    /// Returns `true` if the history can redo.
    pub fn can_redo(&self) -> bool {
        self.inner.can_redo()
    }

    /// Returns the current branch.
    pub fn branch(&self) -> usize {
        self.inner.branch()
    }

    /// Returns the position of the current command.
    pub fn current(&self) -> usize {
        self.inner.current()
    }

    /// Returns a reference to the target.
    pub fn target(&self) -> &C::Target {
        &self.target
    }

    /// Pushes the command to the top of the history and executes its `apply`method.
    ///
    /// # Errors
    /// If an error occur when executing `apply` the error is returned.
    pub fn apply(&mut self, command: impl Into<C>) -> Result<C> {
        self.inner.apply(&mut self.target, command.into())
    }

    /// Calls the `undo` method for the active command
    /// and sets the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing `undo` the error is returned.
    pub fn undo(&mut self) -> Result<C> {
        self.inner.undo(&mut self.target)
    }

    /// Calls the [`redo`] method for the active command
    /// and sets the next one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the error is returned.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    pub fn redo(&mut self) -> Result<C> {
        self.inner.redo(&mut self.target)
    }

    /// Repeatedly calls `undo` or`redo` until the command in `branch` at `current` is reached.
    ///
    /// # Errors
    /// If an error occur when executing `undo` or `redo` the error is returned.
    pub fn go_to(&mut self, branch: usize, current: usize) -> Option<Result<C>> {
        self.inner.go_to(&mut self.target, branch, current)
    }

    /// Go back or forward in the history to the command that was made closest to the datetime provided.
    ///
    /// This method does not jump across branches.
    #[cfg(feature = "chrono")]
    pub fn time_travel(&mut self, to: &DateTime<impl TimeZone>) -> Option<Result<C>> {
        self.inner.time_travel(&mut self.target, to)
    }

    /// Marks the target as currently being in a saved or unsaved state.
    pub fn set_saved(&mut self, saved: bool) {
        self.inner.set_saved(saved);
    }

    /// Removes all commands from the history without undoing them.
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl<C: Command> Default for History<C>
where
    C::Target: Default,
{
    fn default() -> Self {
        History::new(C::Target::default())
    }
}
