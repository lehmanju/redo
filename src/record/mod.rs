mod config;

pub use self::config::Config;

use std::collections::VecDeque;
use std::collections::vec_deque;
use std::fmt::{self, Debug, Formatter};
use Command;

/// A record of commands.
pub struct Record<T, C: Command<T>> {
    commands: VecDeque<C>,
    receiver: T,
    idx: usize,
    limit: Option<usize>,
    state_change: Option<Box<FnMut(bool)>>,
}

impl<T, C: Command<T>> Record<T, C> {
    /// Returns a new `Record`.
    #[inline]
    pub fn new(receiver: T) -> Record<T, C> {
        Record {
            commands: VecDeque::new(),
            receiver,
            idx: 0,
            limit: None,
            state_change: None,
        }
    }

    /// Returns a configurator for a `Record`.
    #[inline]
    pub fn config(receiver: T) -> Config<T, C> {
        Config::new(receiver)
    }

    /// Returns the capacity of the `Record`.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.commands.capacity()
    }

    /// Returns the number of `Command`s in the `Record`.
    #[inline]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns `true` if the `Record` is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Returns a reference to the `receiver`.
    #[inline]
    pub fn as_receiver(&self) -> &T {
        &self.receiver
    }

    /// Consumes the `Stack`, returning the `receiver`.
    #[inline]
    pub fn into_receiver(self) -> T {
        self.receiver
    }

    /// Returns `true` if the state of the `Record` is clean, `false` otherwise.
    #[inline]
    pub fn is_clean(&self) -> bool {
        self.idx == self.len()
    }

    /// Returns `true` if the state of the `Record` is dirty, `false` otherwise.
    #[inline]
    pub fn is_dirty(&self) -> bool {
        !self.is_clean()
    }

    /// Pushes `cmd` on top of the `Record` and executes its [`redo`] method.
    /// The command is merged with the previous top `Command` if [`merge`] does not return `None`.
    ///
    /// All `Command`s above the active one are removed from the stack and returned as an iterator.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] or [merging commands][`merge`],
    /// the error is returned together with the `Command`.
    ///
    /// [`redo`]: ../trait.Command.html#tymethod.redo
    /// [`merge`]: ../trait.Command.html#method.merge
    pub fn push(&mut self, mut cmd: C) -> Result<Commands<C>, (C, C::Err)> {
        let is_dirty = self.is_dirty();
        let len = self.idx;
        if let Err(e) = cmd.redo(&mut self.receiver) {
            return Err((cmd, e));
        }
        let iter = self.commands.split_off(len).into_iter();

        match self.commands.back_mut().and_then(|last| last.merge(&cmd)) {
            Some(x) => {
                if let Err(e) = x {
                    return Err((cmd, e));
                }
            }
            None => {
                match self.limit {
                    Some(limit) if len == limit => {
                        self.commands.pop_front();
                    }
                    _ => self.idx += 1,
                }
                self.commands.push_back(cmd);
            }
        }

        debug_assert_eq!(self.idx, self.len());
        // State is always clean after a push, check if it was dirty before.
        if is_dirty {
            if let Some(ref mut f) = self.state_change {
                f(true);
            }
        }
        Ok(Commands(iter))
    }

    /// Calls the [`redo`] method for the active `Command` and sets the next one as the new
    /// active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the error is returned.
    ///
    /// [`redo`]: ../trait.Command.html#tymethod.redo
    #[inline]
    pub fn redo(&mut self) -> Result<(), C::Err> {
        if self.idx < self.len() {
            let is_dirty = self.is_dirty();
            self.commands[self.idx].redo(&mut self.receiver)?;
            self.idx += 1;
            // Check if the state went from dirty to clean.
            if is_dirty && self.is_clean() {
                if let Some(ref mut f) = self.state_change {
                    f(true);
                }
            }
        }
        Ok(())
    }

    /// Calls the [`undo`] method for the active `Command` and sets the previous one as the new
    /// active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned.
    ///
    /// [`undo`]: ../trait.Command.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Result<(), C::Err> {
        if self.idx > 0 {
            let is_clean = self.is_clean();
            self.idx -= 1;
            self.commands[self.idx].undo(&mut self.receiver)?;
            // Check if stack went from clean to dirty.
            if is_clean && self.is_dirty() {
                if let Some(ref mut f) = self.state_change {
                    f(false);
                }
            }
        }
        Ok(())
    }
}

impl<T: Debug, C: Command<T> + Debug> Debug for Record<T, C> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Record")
            .field("commands", &self.commands)
            .field("receiver", &self.receiver)
            .field("idx", &self.idx)
            .field("limit", &self.limit)
            .finish()
    }
}

/// Iterator over `Command`s of type `C`.
#[derive(Debug)]
pub struct Commands<C>(vec_deque::IntoIter<C>);

impl<C> Iterator for Commands<C> {
    type Item = C;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
