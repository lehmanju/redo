use std::collections::vec_deque::{VecDeque, IntoIter};
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use Command;

/// A record of commands.
///
/// The `Record` works mostly like a `Stack`, but it stores the commands
/// instead of returning them when undoing. This means it can roll the
/// receivers state backwards and forwards by using the undo and redo methods.
/// In addition, the `Record` has an internal state that is either clean or dirty.
/// A clean state means that the `Record` does not have any `Command`s to redo,
/// while a dirty state means that it does. The user can give the `Record` a function
/// that is called each time the state changes by using the `config` constructor.
///
/// # Examples
/// ```
/// use redo::{Command, Record};
///
/// #[derive(Debug)]
/// struct Add(char);
///
/// impl Command<String> for Add {
///     type Err = &'static str;
///
///     fn redo(&mut self, s: &mut String) -> Result<(), &'static str> {
///         s.push(self.0);
///         Ok(())
///     }
///
///     fn undo(&mut self, s: &mut String) -> Result<(), &'static str> {
///         self.0 = s.pop().ok_or("`String` is unexpectedly empty")?;
///         Ok(())
///     }
/// }
///
/// fn foo() -> Result<(), &'static str> {
///     let mut record = Record::default();
///
///     record.push(Add('a')).map_err(|(_, e)| e)?;
///     record.push(Add('b')).map_err(|(_, e)| e)?;
///     record.push(Add('c')).map_err(|(_, e)| e)?;
///
///     assert_eq!(record.as_receiver(), "abc");
///
///     record.undo()?;
///     record.undo()?;
///     record.undo()?;
///
///     assert_eq!(record.as_receiver(), "");
///
///     record.redo()?;
///     record.redo()?;
///     record.redo()?;
///
///     assert_eq!(record.into_receiver(), "abc");
///
///     Ok(())
/// }
/// # foo().unwrap();
/// ```
pub struct Record<'a, T, C: Command<T>> {
    commands: VecDeque<C>,
    receiver: T,
    idx: usize,
    limit: Option<usize>,
    state_change: Option<Box<FnMut(bool) + 'a>>,
}

impl<'a, T, C: Command<T>> Record<'a, T, C> {
    /// Returns a new `Record`.
    #[inline]
    pub fn new<U: Into<T>>(receiver: U) -> Record<'a, T, C> {
        Record {
            commands: VecDeque::new(),
            receiver: receiver.into(),
            idx: 0,
            limit: None,
            state_change: None,
        }
    }

    /// Returns a configurator for a `Record`.
    ///
    /// # Examples
    /// ```
    /// # use redo::{Command, Record};
    /// # #[derive(Debug)]
    /// # struct Add(char);
    /// # impl Command<String> for Add {
    /// #     type Err = &'static str;
    /// #     fn redo(&mut self, s: &mut String) -> Result<(), &'static str> {
    /// #         s.push(self.0);
    /// #         Ok(())
    /// #     }
    /// #     fn undo(&mut self, s: &mut String) -> Result<(), &'static str> {
    /// #         self.0 = s.pop().ok_or("`String` is unexpectedly empty")?;
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> Result<(), &'static str> {
    /// let mut record = Record::config("")
    ///     .capacity(2)
    ///     .limit(2)
    ///     .finish();
    ///
    /// record.push(Add('a')).map_err(|(_, e)| e)?;
    /// record.push(Add('b')).map_err(|(_, e)| e)?;
    /// record.push(Add('c')).map_err(|(_, e)| e)?; // 'a' is removed from the record since limit is 2.
    ///
    /// assert_eq!(record.as_receiver(), "abc");
    ///
    /// record.undo()?;
    /// record.undo()?;
    /// record.undo()?;
    ///
    /// assert_eq!(record.as_receiver(), "a");
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn config<U: Into<T>>(receiver: U) -> Config<'a, T, C> {
        Config {
            commands: PhantomData,
            receiver: receiver.into(),
            capacity: 0,
            limit: None,
            state_change: None,
        }
    }

    /// Returns the limit of the `Record`, or `None` if it has no limit.
    #[inline]
    pub fn limit(&self) -> Option<usize> {
        self.limit
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

    /// Returns a reference to the `receiver`.
    #[inline]
    pub fn as_receiver(&self) -> &T {
        &self.receiver
    }

    /// Consumes the `Record`, returning the `receiver`.
    #[inline]
    pub fn into_receiver(self) -> T {
        self.receiver
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
    /// # Examples
    /// ```
    /// # use redo::{Command, Record};
    /// # #[derive(Debug, Eq, PartialEq)]
    /// # struct Add(char);
    /// # impl Command<String> for Add {
    /// #     type Err = &'static str;
    /// #     fn redo(&mut self, s: &mut String) -> Result<(), &'static str> {
    /// #         s.push(self.0);
    /// #         Ok(())
    /// #     }
    /// #     fn undo(&mut self, s: &mut String) -> Result<(), &'static str> {
    /// #         self.0 = s.pop().ok_or("`String` is unexpectedly empty")?;
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> Result<(), &'static str> {
    /// let mut record = Record::default();
    ///
    /// record.push(Add('a')).map_err(|(_, e)| e)?;
    /// record.push(Add('b')).map_err(|(_, e)| e)?;
    /// record.push(Add('c')).map_err(|(_, e)| e)?;
    ///
    /// assert_eq!(record.as_receiver(), "abc");
    ///
    /// record.undo()?;
    /// record.undo()?;
    /// let mut bc = record.push(Add('e')).map_err(|(_, e)| e)?;
    ///
    /// assert_eq!(record.as_receiver(), "ae");
    /// assert_eq!(bc.next(), Some(Add('b')));
    /// assert_eq!(bc.next(), Some(Add('c')));
    /// assert_eq!(bc.next(), None);
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
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
        debug_assert_eq!(len, self.len());

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

impl<'a, T: Default, C: Command<T>> Default for Record<'a, T, C> {
    #[inline]
    fn default() -> Record<'a, T, C> {
        Record {
            commands: VecDeque::new(),
            receiver: Default::default(),
            idx: 0,
            limit: None,
            state_change: None,
        }
    }
}

impl<'a, T, C: Command<T>> AsRef<T> for Record<'a, T, C> {
    #[inline]
    fn as_ref(&self) -> &T {
        self.as_receiver()
    }
}

impl<'a, T: Debug, C: Command<T> + Debug> Debug for Record<'a, T, C> {
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

/// Iterator over `Command`s.
#[derive(Debug)]
pub struct Commands<C>(IntoIter<C>);

impl<C> Iterator for Commands<C> {
    type Item = C;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// Configurator for `Record`.
pub struct Config<'a, T, C: Command<T>> {
    commands: PhantomData<C>,
    receiver: T,
    capacity: usize,
    limit: Option<usize>,
    state_change: Option<Box<FnMut(bool) + 'a>>,
}

impl<'a, T, C: Command<T>> Config<'a, T, C> {
    /// Sets the `capacity` for the `Record`.
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> Config<'a, T, C> {
        self.capacity = capacity;
        self
    }

    /// Sets the `limit` for the `Record`.
    #[inline]
    pub fn limit(mut self, limit: usize) -> Config<'a, T, C> {
        self.limit = if limit == 0 { None } else { Some(limit) };
        self
    }

    /// Sets what should happen when the state changes.
    ///
    /// # Examples
    /// ```
    /// # use std::cell::Cell;
    /// # use redo::{Command, Record};
    /// # #[derive(Debug, Eq, PartialEq)]
    /// # struct Add(char);
    /// # impl Command<String> for Add {
    /// #     type Err = &'static str;
    /// #     fn redo(&mut self, s: &mut String) -> Result<(), &'static str> {
    /// #         s.push(self.0);
    /// #         Ok(())
    /// #     }
    /// #     fn undo(&mut self, s: &mut String) -> Result<(), &'static str> {
    /// #         self.0 = s.pop().ok_or("`String` is unexpectedly empty")?;
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> Result<(), &'static str> {
    /// let x = Cell::new(0);
    /// let mut record = Record::config("")
    ///     .state_change(|is_clean| {
    ///         if is_clean {
    ///             x.set(1);
    ///         } else {
    ///             x.set(2);
    ///         }
    ///     })
    ///     .finish();
    ///
    /// assert_eq!(x.get(), 0);
    /// record.push(Add('a')).map_err(|(_, e)| e)?;
    /// assert_eq!(x.get(), 0);
    /// record.undo()?;
    /// assert_eq!(x.get(), 2);
    /// record.redo()?;
    /// assert_eq!(x.get(), 1);
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn state_change<F>(mut self, f: F) -> Config<'a, T, C>
    where
        F: FnMut(bool) + 'a,
    {
        self.state_change = Some(Box::new(f));
        self
    }

    /// Creates the `Record`.
    #[inline]
    pub fn finish(self) -> Record<'a, T, C> {
        Record {
            commands: VecDeque::with_capacity(self.capacity),
            receiver: self.receiver,
            idx: 0,
            limit: self.limit,
            state_change: self.state_change,
        }
    }
}

impl<'a, T: Debug, C: Command<T> + Debug> Debug for Config<'a, T, C> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Config")
            .field("receiver", &self.receiver)
            .field("capacity", &self.capacity)
            .field("limit", &self.limit)
            .finish()
    }
}
