//! A timeline of commands.

use crate::{history::Display, Command, History, Result, Signal};
use alloc::vec::Vec;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A timeline of commands.
///
/// # Examples
/// ```
/// # use redo::{Command, Timeline};
/// # struct Add(char);
/// # impl Command for Add {
/// #     type Target = String;
/// #     type Error = &'static str;
/// #     fn apply(&mut self, s: &mut String) -> redo::Result<Add> {
/// #         s.push(self.0);
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self, s: &mut String) -> redo::Result<Add> {
/// #         self.0 = s.pop().ok_or("s is empty")?;
/// #         Ok(())
/// #     }
/// # }
/// # fn main() -> redo::Result<Add> {
/// let mut timeline = Timeline::default();
/// timeline.apply(Add('a'))?;
/// timeline.apply(Add('b'))?;
/// assert_eq!(timeline.target(), "ab");
/// timeline.undo()?;
/// timeline.apply(Add('c'))?;
/// assert_eq!(timeline.target(), "ac");
/// timeline.undo()?;
/// timeline.undo()?;
/// assert_eq!(timeline.target(), "ab");
/// # Ok(())
/// # }
/// ```
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(bound(
        serialize = "C: Command + Serialize, C::Target: Serialize",
        deserialize = "C: Command + Deserialize<'de>, C::Target: Deserialize<'de>"
    ))
)]
pub struct Timeline<C: Command, F = fn(Signal)> {
    index: usize,
    timeline: Vec<usize>,
    history: History<C, F>,
}

impl<C: Command> Timeline<C> {
    /// Returns a new timeline.
    pub fn new(target: C::Target) -> Timeline<C> {
        Timeline {
            index: 0,
            timeline: Vec::new(),
            history: History::new(target),
        }
    }
}

impl<C: Command, F> Timeline<C, F> {
    /// Returns the current branch.
    pub fn branch(&self) -> usize {
        self.history.branch()
    }

    /// Returns the position of the current command.
    pub fn current(&self) -> usize {
        self.history.current()
    }

    /// Returns a structure for configurable formatting of the timeline.
    pub fn display(&self) -> Display<C, F> {
        Display::from(&self.history)
    }

    /// Returns a reference to the `target`.
    pub fn target(&self) -> &C::Target {
        self.history.target()
    }

    /// Returns a mutable reference to the `target`.
    ///
    /// This method should **only** be used when doing changes that should not be able to be undone.
    pub fn target_mut(&mut self) -> &mut C::Target {
        self.history.target_mut()
    }

    /// Consumes the history, returning the `target`.
    pub fn into_target(self) -> C::Target {
        self.history.into_target()
    }
}

impl<C: Command, F: FnMut(Signal)> Timeline<C, F> {
    /// Marks the target as currently being in a saved or unsaved state.
    pub fn set_saved(&mut self, saved: bool) {
        self.history.set_saved(saved);
    }

    /// Removes all commands from the archive without undoing them.
    pub fn clear(&mut self) {
        self.index = 0;
        self.timeline.clear();
        self.history.clear();
    }

    /// Pushes the command to the top of the archive and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    pub fn apply(&mut self, command: C) -> Result<C> {
        self.history.apply(command)?;
        let root = self.history.branch();
        self.timeline.push(root);
        self.index = self.timeline.len();
        Ok(())
    }

    /// Calls the [`undo`] method for the active command
    /// and sets the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    pub fn undo(&mut self) -> Result<C> {
        if self.index == 0 || self.index > self.timeline.len() {
            return Ok(());
        }
        self.index -= 1;
        let root = self.timeline[self.index];
        self.timeline.push(root);
        if root == self.history.branch() {
            self.history.undo()
        } else {
            self.history.jump_to(root);
            self.history.redo()
        }
    }

    /// Calls the [`redo`] method for the active command
    /// and sets the next one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the error is returned.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    pub fn redo(&mut self) -> Result<C> {
        if self.index == self.timeline.len() - 1 {
            return Ok(());
        }
        self.index += 1;
        let root = self.timeline[self.index];
        self.timeline.push(root);
        if root == self.history.branch() {
            self.history.redo()
        } else {
            self.history.undo()?;
            self.history.jump_to(root);
            Ok(())
        }
    }
}

impl<C: Command> Default for Timeline<C>
where
    C::Target: Default,
{
    fn default() -> Timeline<C> {
        Timeline::new(Default::default())
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use alloc::string::String;

    struct Add(char);

    impl Command for Add {
        type Target = String;
        type Error = &'static str;

        fn apply(&mut self, s: &mut String) -> Result<Add> {
            s.push(self.0);
            Ok(())
        }

        fn undo(&mut self, s: &mut String) -> Result<Add> {
            self.0 = s.pop().ok_or("s is empty")?;
            Ok(())
        }
    }

    #[test]
    fn simple() {
        let mut timeline = Timeline::default();
        timeline.apply(Add('a')).unwrap();
        timeline.apply(Add('b')).unwrap();
        timeline.undo().unwrap();
        timeline.apply(Add('c')).unwrap();
        timeline.undo().unwrap();
        timeline.undo().unwrap();
        assert_eq!(timeline.target(), "ab");
        timeline.redo().unwrap();
        timeline.redo().unwrap();
        assert_eq!(timeline.target(), "ac");
        timeline.undo().unwrap();
        timeline.undo().unwrap();
        assert_eq!(timeline.target(), "ab");
        timeline.undo().unwrap();
        timeline.undo().unwrap();
        assert_eq!(timeline.target(), "");
        timeline.redo().unwrap();
        timeline.redo().unwrap();
        assert_eq!(timeline.target(), "ab");
        timeline.redo().unwrap();
        timeline.redo().unwrap();
        assert_eq!(timeline.target(), "ac");
    }
}
