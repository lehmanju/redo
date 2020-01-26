use crate::{Command, History, Result, Signal};
use alloc::vec::Vec;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// An archive of commands.
///
/// # Examples
/// ```
/// # use redo::{Command, Archive};
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
/// let mut history = Archive::default();
/// history.apply(Add('a'))?;
/// history.apply(Add('b'))?;
/// assert_eq!(history.target(), "ab");
/// history.undo()?;
/// history.apply(Add('c'))?;
/// assert_eq!(history.target(), "ac");
/// history.undo()?;
/// history.undo()?;
/// assert_eq!(history.target(), "ab");
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
pub struct Archive<C: Command, F = fn(Signal)> {
    current: usize,
    branches: Vec<usize>,
    history: History<C, F>,
}

impl<C: Command> Archive<C> {
    /// Returns a new archive.
    pub fn new(target: C::Target) -> Archive<C> {
        Archive {
            current: 0,
            branches: Vec::new(),
            history: History::new(target),
        }
    }
}

impl<C: Command, F: FnMut(Signal)> Archive<C, F> {
    /// Removes all commands from the archive without undoing them.
    pub fn clear(&mut self) {
        self.history.clear();
        self.current = 0;
        self.branches.clear();
    }

    /// Pushes the command to the top of the archive and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    pub fn apply(&mut self, command: C) -> Result<C> {
        self.history.apply(command)?;
        self.branches.push(self.history.branch());
        self.current = self.branches.len();
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
        if self.current == 0 || self.current > self.branches.len() {
            return Ok(());
        }
        self.current -= 1;
        let root = self.branches[self.current];
        self.branches.push(root);
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
        if self.current == self.branches.len() - 2 {
            return Ok(());
        }
        self.current += 1;
        let root = self.branches[self.current];
        self.branches.push(root);
        if root == self.history.branch() {
            self.history.redo()
        } else {
            self.history.undo()?;
            self.history.jump_to(root);
            Ok(())
        }
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

impl<C: Command> Default for Archive<C>
where
    C::Target: Default,
{
    fn default() -> Archive<C> {
        Archive::new(Default::default())
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
    fn jump_to() {
        let mut archive = Archive::default();
        archive.apply(Add('a')).unwrap();
        archive.apply(Add('b')).unwrap();
        archive.undo().unwrap();
        archive.apply(Add('c')).unwrap();
        archive.undo().unwrap();
        archive.undo().unwrap();
        assert_eq!(archive.target(), "ab");
        archive.redo().unwrap();
        archive.redo().unwrap();
        assert_eq!(archive.target(), "ac");
        archive.undo().unwrap();
        archive.undo().unwrap();
        assert_eq!(archive.target(), "ab");
        archive.undo().unwrap();
        archive.undo().unwrap();
        assert_eq!(archive.target(), "");
        archive.redo().unwrap();
        archive.redo().unwrap();
        assert_eq!(archive.target(), "ab");
        archive.redo().unwrap();
        archive.redo().unwrap();
        assert_eq!(archive.target(), "ac");
    }
}
