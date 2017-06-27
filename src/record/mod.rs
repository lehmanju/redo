//! A `Record` of `Command`s.

use std::collections::VecDeque;
use std::marker::PhantomData;
use std::fmt::{self, Debug, Formatter};
use Command;

/// A record of `Command`s.
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

    /// Returns a `Config` for `Record`.
    #[inline]
    pub fn config(receiver: T) -> Config<T, C> {
        Config {
            commands: PhantomData,
            receiver,
            capacity: 0,
            limit: 0,
            state_change: None,
        }
    }

    /// Returns the capacity of the `Record`.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.commands.capacity()
    }

    /// Reserves capacity for at least `additional` more commands to be inserted in the `Record`.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.commands.reserve(additional);
    }

    /// Shrinks the capacity of the `Record` as much as possible.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.commands.shrink_to_fit();
    }

    /// Returns the number of `Command`s in the `Record`.
    #[inline]
    pub fn len(&self) -> usize {
        self.commands.len()
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

    /// Pushes `cmd` on top of the `Record` and executes its [`redo`] method. The command is merged with
    /// the previous top `Command` if [`merge`] does not return `None`.
    ///
    /// All `Command`s above the active one are removed from the stack and returned.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] or [merging commands][`merge`], the error is returned together
    /// with the `Command`.
    ///
    /// [`redo`]: trait.Command.html#tymethod.redo
    /// [`merge`]: trait.Command.html#method.merge
    pub fn push(&mut self, mut cmd: C) -> Result<Vec<C>, (C, C::Err)> {
        let is_dirty = self.is_dirty();
        let len = self.idx;
        if let Err(e) = cmd.redo(&mut self.receiver) {
            return Err((cmd, e));
        }
        let drained: Vec<_> = self.commands.drain(len..).collect();

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
        Ok(drained)
    }

    /// Calls the [`redo`] method for the active `Command` and sets the next one as the new
    /// active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the error is returned.
    ///
    /// [`redo`]: trait.Command.html#tymethod.redo
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
    /// [`undo`]: trait.Command.html#tymethod.undo
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
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Record")
            .field("commands", &self.commands)
            .field("receiver", &self.receiver)
            .field("idx", &self.idx)
            .field("limit", &self.limit)
            .finish()
    }
}

/// Configurator for `Record`.
pub struct Config<T, C: Command<T>> {
    commands: PhantomData<C>,
    receiver: T,
    capacity: usize,
    limit: usize,
    state_change: Option<Box<FnMut(bool)>>,
}

impl<T, C: Command<T>> Config<T, C> {
    /// Sets the `capacity` for the `Record`.
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> Config<T, C> {
        self.capacity = capacity;
        self
    }

    /// Sets the `limit` for the `Record`.
    #[inline]
    pub fn limit(mut self, limit: usize) -> Config<T, C> {
        self.limit = limit;
        self
    }

    /// Sets what should happen when the state changes.
    #[inline]
    pub fn state_change<F>(mut self, f: F) -> Config<T, C>
        where
            F: FnMut(bool) + 'static,
    {
        self.state_change = Some(Box::new(f));
        self
    }

    /// Creates the `Record`.
    #[inline]
    pub fn finish(self) -> Record<T, C> {
        Record {
            commands: VecDeque::with_capacity(self.capacity),
            receiver: self.receiver,
            idx: 0,
            limit: if self.limit == 0 {
                None
            } else {
                Some(self.limit)
            },
            state_change: self.state_change,
        }
    }
}

impl<T: Debug, C: Command<T> + Debug> Debug for Config<T, C> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Config")
            .field("receiver", &self.receiver)
            .field("capacity", &self.capacity)
            .field("limit", &self.limit)
            .finish()
    }
}
