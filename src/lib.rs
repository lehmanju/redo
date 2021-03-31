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

/// A history of commands.
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
#[derive(Clone)]
pub struct History<C: Command, F = Box<dyn FnMut(Signal)>> {
    inner: Inner<C, F>,
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
}

impl<C: Command, F> History<C, F> {
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
    pub fn connect(&mut self, slot: F) -> Option<F> {
        self.inner.connect(slot)
    }

    /// Removes and returns the slot if it exists.
    pub fn disconnect(&mut self) -> Option<F> {
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
}

impl<C: Command, F: FnMut(Signal)> History<C, F> {
    /// Pushes the command to the top of the history and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    pub fn apply(&mut self, target: &mut C::Target, command: C) -> Result<C> {
        self.inner.apply(target, command)
    }

    /// Calls the [`undo`] method for the active command
    /// and sets the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    pub fn undo(&mut self, target: &mut C::Target) -> Result<C> {
        self.inner.undo(target)
    }

    /// Calls the [`redo`] method for the active command
    /// and sets the next one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the error is returned.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    pub fn redo(&mut self, target: &mut C::Target) -> Result<C> {
        self.inner.redo(target)
    }

    /// Repeatedly calls [`undo`] or [`redo`] until the command in `branch` at `current` is reached.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    /// [`redo`]: trait.Command.html#method.redo
    pub fn go_to(
        &mut self,
        target: &mut C::Target,
        branch: usize,
        current: usize,
    ) -> Option<Result<C>> {
        self.inner.go_to(target, branch, current)
    }

    /// Go back or forward in the history to the command that was made closest to the datetime provided.
    ///
    /// This method does not jump across branches.
    #[cfg(feature = "chrono")]
    pub fn time_travel(
        &mut self,
        target: &mut C::Target,
        to: &DateTime<impl TimeZone>,
    ) -> Option<Result<C>> {
        self.inner.time_travel(target, to)
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
