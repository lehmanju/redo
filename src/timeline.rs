use crate::{Command, Result};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A timeline of commands.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct Timeline<R, C: Command<R>> {
    commands: [Option<C>; 32],
    receiver: R,
    cursor: usize,
}

impl<R, C: Command<R>> Timeline<R, C> {
    /// Returns a new timeline.
    #[inline]
    pub fn new(receiver: impl Into<R>) -> Timeline<R, C> {
        Timeline {
            commands: Default::default(),
            receiver: receiver.into(),
            cursor: 0,
        }
    }

    /// Pushes the command on top of the timeline and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned together with the command.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    #[inline]
    pub fn apply(&mut self, _: C) -> Result<R, C> {
        unimplemented!()
    }

    /// Calls the [`undo`] method for the active command and sets the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned together with the command.
    ///
    /// [`undo`]: ../trait.Command.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result<R, C>> {
        unimplemented!()
    }

    /// Calls the [`redo`] method for the active command and sets the next one as the new
    /// active one.
    ///
    /// # Errors
    /// If an error occur when applying [`redo`] the error is returned together with the command.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result<R, C>> {
        unimplemented!()
    }
}
