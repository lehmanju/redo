use std::collections::vec_deque::{IntoIter, VecDeque};
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use {Command, Error};

/// Used to represent the state the record or the receiver can be in.
#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum Signal {
    /// The record can redo.
    Redo,
    /// The record can not redo.
    NoRedo,
    /// The record can undo.
    Undo,
    /// The record can not undo.
    NoUndo,
    /// The receiver is in a saved state.
    Saved,
    /// The receiver is not in a saved state.
    Unsaved,
}

/// A record of commands.
///
/// The record works mostly like a stack, but it stores the commands
/// instead of returning them when undoing. This means it can roll the
/// receivers state backwards and forwards by using the undo and redo methods.
/// In addition, the record has an internal state that is either clean or dirty.
/// A clean state means that the record does not have any commands to redo,
/// while a dirty state means that it does. The user can give the record a function
/// that is called each time the state changes by using the [`builder`].
///
/// # Examples
/// ```
/// use std::error::Error;
/// use std::fmt::{self, Display, Formatter};
/// use redo::{Command, Record};
///
/// #[derive(Debug)]
/// struct StrErr(&'static str);
///
/// impl Display for StrErr {
///     fn fmt(&self, f: &mut Formatter) -> fmt::Result { f.write_str(self.0) }
/// }
///
/// impl Error for StrErr {
///     fn description(&self) -> &str { self.0 }
/// }
///
/// #[derive(Debug)]
/// struct Add(char);
///
/// impl Command<String> for Add {
///     type Err = StrErr;
///
///     fn redo(&mut self, s: &mut String) -> Result<(), Self::Err> {
///         s.push(self.0);
///         Ok(())
///     }
///
///     fn undo(&mut self, s: &mut String) -> Result<(), Self::Err> {
///         self.0 = s.pop().ok_or(StrErr("`s` is empty"))?;
///         Ok(())
///     }
/// }
///
/// fn foo() -> Result<(), Box<Error>> {
///     let mut record = Record::default();
///
///     record.push(Add('a'))?;
///     record.push(Add('b'))?;
///     record.push(Add('c'))?;
///
///     assert_eq!(record.as_receiver(), "abc");
///
///     record.undo().unwrap()?;
///     record.undo().unwrap()?;
///     record.undo().unwrap()?;
///
///     assert_eq!(record.as_receiver(), "");
///
///     record.redo().unwrap()?;
///     record.redo().unwrap()?;
///     record.redo().unwrap()?;
///
///     assert_eq!(record.into_receiver(), "abc");
///
///     Ok(())
/// }
/// # foo().unwrap();
/// ```
///
/// [`builder`]: struct.RecordBuilder.html
pub struct Record<'a, R, C: Command<R>> {
    commands: VecDeque<C>,
    receiver: R,
    cursor: usize,
    limit: usize,
    saved: Option<usize>,
    signals: Option<Box<FnMut(Signal) + Send + Sync + 'a>>,
}

impl<'a, R, C: Command<R>> Record<'a, R, C> {
    /// Returns a new record.
    #[inline]
    pub fn new<T: Into<R>>(receiver: T) -> Record<'a, R, C> {
        Record {
            commands: VecDeque::new(),
            receiver: receiver.into(),
            cursor: 0,
            limit: 0,
            saved: None,
            signals: None,
        }
    }

    /// Returns a builder for a record.
    #[inline]
    pub fn builder() -> RecordBuilder<'a, R, C> {
        RecordBuilder {
            commands: PhantomData,
            receiver: PhantomData,
            capacity: 0,
            limit: 0,
            signals: None,
        }
    }

    /// Returns the capacity of the record.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.commands.capacity()
    }

    /// Returns the limit of the record, or `None` if it has no limit.
    #[inline]
    pub fn limit(&self) -> Option<usize> {
        if self.limit == 0 { None } else { Some(self.limit) }
    }

    /// Returns the number of commands in the record.
    #[inline]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns `true` if the record is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Returns a reference to the `receiver`.
    #[inline]
    pub fn as_receiver(&self) -> &R {
        &self.receiver
    }

    /// Consumes the record, returning the `receiver`.
    #[inline]
    pub fn into_receiver(self) -> R {
        self.receiver
    }

    /// Marks the receiver as being in a saved state.
    #[inline]
    pub fn set_saved(&mut self, is_saved: bool) {
        self.saved = match self.saved {
            Some(saved) if is_saved && saved != self.cursor => {
                if let Some(ref mut f) = self.signals {
                    f(Signal::Saved);
                }
                Some(self.cursor)
            },
            Some(saved) if is_saved => Some(saved),
            Some(_) => {
                if let Some(ref mut f) = self.signals {
                    f(Signal::Unsaved);
                }
                None
            },
            None => None,
        };
    }

    /// Returns `true` if the receiver is in a saved state, `false` otherwise.
    #[inline]
    pub fn is_saved(&self) -> bool {
        self.saved.map_or(false, |saved| saved == self.cursor)
    }

    /// Pushes the command on top of the record and executes its [`redo`] method.
    /// The command is merged with the previous top command if [`merge`] does not return `None`.
    ///
    /// All commands above the active one are removed from the stack and returned as an iterator.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] or [merging commands][`merge`],
    /// the error is returned together with the command.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # use std::fmt::{self, Display, Formatter};
    /// # use redo::{Command, Record};
    /// # #[derive(Debug)]
    /// # struct StrErr(&'static str);
    /// # impl Display for StrErr {
    /// #     fn fmt(&self, f: &mut Formatter) -> fmt::Result { write!(f, "{}", self.0) }
    /// # }
    /// # impl Error for StrErr {
    /// #     fn description(&self) -> &str { self.0 }
    /// # }
    /// # #[derive(Debug, Eq, PartialEq)]
    /// # struct Add(char);
    /// # impl From<char> for Add {
    /// #   fn from(c: char) -> Add { Add(c) }
    /// # }
    /// # impl Command<String> for Add {
    /// #     type Err = StrErr;
    /// #     fn redo(&mut self, s: &mut String) -> Result<(), StrErr> {
    /// #         s.push(self.0);
    /// #         Ok(())
    /// #     }
    /// #     fn undo(&mut self, s: &mut String) -> Result<(), StrErr> {
    /// #         self.0 = s.pop().ok_or(StrErr("`String` is unexpectedly empty"))?;
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> Result<(), Box<Error>> {
    /// let mut record = Record::default();
    ///
    /// record.push(Add('a'))?;
    /// record.push(Add('b'))?;
    /// record.push(Add('c'))?;
    ///
    /// assert_eq!(record.as_receiver(), "abc");
    ///
    /// record.undo().unwrap()?;
    /// record.undo().unwrap()?;
    /// let mut bc = record.push(Add('e'))?;
    ///
    /// assert_eq!(record.into_receiver(), "ae");
    /// assert_eq!(bc.next(), Some(Add('b')));
    /// assert_eq!(bc.next(), Some(Add('c')));
    /// assert_eq!(bc.next(), None);
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    ///
    /// [`redo`]: trait.Command.html#tymethod.redo
    /// [`merge`]: trait.Command.html#method.merge
    #[inline]
    pub fn push(&mut self, mut cmd: C) -> Result<Commands<C>, Error<R, C>> {
        match cmd.redo(&mut self.receiver) {
            Ok(_) => {
                let cursor = self.cursor;
                let was_dirty = cursor != self.len();
                let was_saved = self.is_saved();

                // Pop off all elements after len from record.
                let iter = self.commands.split_off(cursor).into_iter();
                debug_assert_eq!(cursor, self.len());

                // Check if the saved state was popped off.
                if let Some(saved) = self.saved {
                    if saved > cursor {
                        self.saved = None;
                    }
                }

                let cmd = match self.commands.back_mut() {
                    Some(ref mut last) if !was_saved => match last.merge(cmd) {
                        Ok(_) => None,
                        Err(cmd) => Some(cmd),
                    },
                    _ => Some(cmd),
                };

                if let Some(cmd) = cmd {
                    if self.limit != 0 && self.limit == cursor {
                        let _ = self.commands.pop_front();
                        self.saved = match self.saved {
                            Some(0) => None,
                            Some(saved) => Some(saved - 1),
                            None => None,
                        };
                    } else {
                        self.cursor += 1;
                    }
                    self.commands.push_back(cmd);
                }

                debug_assert_eq!(self.cursor, self.len());
                if let Some(ref mut f) = self.signals {
                    // Record is always clean after a push, check if it was dirty before.
                    if was_dirty {
                        f(Signal::NoRedo);
                    }
                    // Check if the stack was empty before pushing the command.
                    if cursor == 0 {
                        f(Signal::Undo);
                    }
                    // Check if record went from saved to unsaved.
                    if was_saved {
                        f(Signal::Unsaved);
                    }
                }
                Ok(Commands(iter))
            }
            Err(e) => Err(Error(cmd, e)),
        }
    }

    /// Calls the [`redo`] method for the active command and sets the next one as the new
    /// active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the
    /// error is returned and the state is left unchanged.
    ///
    /// [`redo`]: trait.Command.html#tymethod.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result<(), C::Err>> {
        if self.cursor < self.len() {
            match self.commands[self.cursor].redo(&mut self.receiver) {
                Ok(_) => {
                    let was_dirty = self.cursor != self.len();
                    let was_saved = self.is_saved();
                    self.cursor += 1;
                    let is_clean = self.cursor == self.len();
                    let is_saved = self.is_saved();
                    if let Some(ref mut f) = self.signals {
                        // Check if record went from dirty to clean.
                        if was_dirty && is_clean {
                            f(Signal::NoRedo);
                        }
                        // Check if the stack was empty before pushing the command.
                        if self.cursor == 1 {
                            f(Signal::Undo);
                        }
                        // Check if record went from saved to unsaved.
                        if was_saved {
                            f(Signal::Unsaved);
                        }
                        // Check if record went from unsaved to saved.
                        if is_saved {
                            f(Signal::Saved);
                        }
                    }
                    Some(Ok(()))
                }
                Err(e) => Some(Err(e)),
            }
        } else {
            None
        }
    }

    /// Calls the [`undo`] method for the active command and sets the previous one as the new
    /// active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the
    /// error is returned and the state is left unchanged.
    ///
    /// [`undo`]: ../trait.Command.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result<(), C::Err>> {
        if self.cursor > 0 {
            match self.commands[self.cursor - 1].undo(&mut self.receiver) {
                Ok(_) => {
                    let was_clean = self.cursor == self.len();
                    let was_saved = self.is_saved();
                    self.cursor -= 1;
                    let is_dirty = self.cursor != self.len();
                    let is_saved = self.is_saved();
                    if let Some(ref mut f) = self.signals {
                        if was_clean && is_dirty {
                            // Check if record went from clean to dirty.
                            f(Signal::Redo);
                        } else if self.cursor == 0 {
                            // Check if the stack was not empty before pushing the command.
                            f(Signal::NoUndo);
                        } else if was_saved {
                            // Check if record went from saved to unsaved.
                            f(Signal::Unsaved);
                        } else if is_saved {
                            // Check if record went from unsaved to saved.
                            f(Signal::Saved);
                        }
                    }
                    Some(Ok(()))
                }
                Err(e) => Some(Err(e)),
            }
        } else {
            None
        }
    }
}

impl<'a, R: Default, C: Command<R>> Default for Record<'a, R, C> {
    #[inline]
    fn default() -> Record<'a, R, C> {
        Record {
            commands: Default::default(),
            receiver: Default::default(),
            cursor: 0,
            limit: 0,
            saved: None,
            signals: None,
        }
    }
}

impl<'a, R, C: Command<R>> AsRef<R> for Record<'a, R, C> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.as_receiver()
    }
}

impl<'a, R, C: Command<R>> From<R> for Record<'a, R, C> {
    #[inline]
    fn from(receiver: R) -> Self {
        Record::new(receiver)
    }
}

impl<'a, R: Debug, C: Command<R> + Debug> Debug for Record<'a, R, C> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Record")
            .field("commands", &self.commands)
            .field("receiver", &self.receiver)
            .field("cursor", &self.cursor)
            .field("limit", &self.limit)
            .field("saved", &self.saved)
            .finish()
    }
}

/// Iterator over commands.
#[derive(Clone, Debug)]
pub struct Commands<C>(IntoIter<C>);

impl<C> Iterator for Commands<C> {
    type Item = C;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// Builder for a record.
pub struct RecordBuilder<'a, R, C: Command<R>> {
    commands: PhantomData<C>,
    receiver: PhantomData<R>,
    capacity: usize,
    limit: usize,
    signals: Option<Box<FnMut(Signal) + Send + Sync + 'a>>,
}

impl<'a, R, C: Command<R>> RecordBuilder<'a, R, C> {
    /// Sets the `capacity` for the record.
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> RecordBuilder<'a, R, C> {
        self.capacity = capacity;
        self
    }

    /// Sets the `limit` for the record.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # use std::fmt::{self, Display, Formatter};
    /// # use redo::{Command, Record};
    /// # #[derive(Debug)]
    /// # struct StrErr(&'static str);
    /// # impl Display for StrErr {
    /// #     fn fmt(&self, f: &mut Formatter) -> fmt::Result { write!(f, "{}", self.0) }
    /// # }
    /// # impl Error for StrErr {
    /// #     fn description(&self) -> &str { self.0 }
    /// # }
    /// # #[derive(Debug)]
    /// # struct Add(char);
    /// # impl Command<String> for Add {
    /// #     type Err = StrErr;
    /// #     fn redo(&mut self, s: &mut String) -> Result<(), StrErr> {
    /// #         s.push(self.0);
    /// #         Ok(())
    /// #     }
    /// #     fn undo(&mut self, s: &mut String) -> Result<(), StrErr> {
    /// #         self.0 = s.pop().ok_or(StrErr("`String` is unexpectedly empty"))?;
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> Result<(), Box<Error>> {
    /// let mut record = Record::builder()
    ///     .capacity(2)
    ///     .limit(2)
    ///     .default();
    ///
    /// record.push(Add('a'))?;
    /// record.push(Add('b'))?;
    /// record.push(Add('c'))?; // 'a' is removed from the record since limit is 2.
    ///
    /// assert_eq!(record.as_receiver(), "abc");
    ///
    /// record.undo().unwrap()?;
    /// record.undo().unwrap()?;
    /// assert!(record.undo().is_none());
    ///
    /// assert_eq!(record.into_receiver(), "a");
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn limit(mut self, limit: usize) -> RecordBuilder<'a, R, C> {
        self.limit = limit;
        self
    }

    /// Decides how different signals should be handled when the state changes.
    /// By default the record does nothing.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # use std::fmt::{self, Display, Formatter};
    /// # use redo::{Command, Record, Signal};
    /// # #[derive(Debug)]
    /// # struct StrErr(&'static str);
    /// # impl Display for StrErr {
    /// #     fn fmt(&self, f: &mut Formatter) -> fmt::Result { write!(f, "{}", self.0) }
    /// # }
    /// # impl Error for StrErr {
    /// #     fn description(&self) -> &str { self.0 }
    /// # }
    /// # #[derive(Debug)]
    /// # struct Add(char);
    /// # impl Command<String> for Add {
    /// #     type Err = StrErr;
    /// #     fn redo(&mut self, s: &mut String) -> Result<(), StrErr> {
    /// #         s.push(self.0);
    /// #         Ok(())
    /// #     }
    /// #     fn undo(&mut self, s: &mut String) -> Result<(), StrErr> {
    /// #         self.0 = s.pop().ok_or(StrErr("`String` is unexpectedly empty"))?;
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> Result<(), Box<Error>> {
    /// # let mut record =
    /// Record::builder()
    ///     .signals(|signal| {
    ///         match signal {
    ///             Signal::Redo => println!("The record can redo."),
    ///             Signal::NoRedo => println!("The record can not redo."),
    ///             Signal::Undo => println!("The record can undo."),
    ///             Signal::NoUndo => println!("The record can not undo."),
    ///             Signal::Saved => println!("The receiver is in a saved state."),
    ///             Signal::Unsaved => println!("The receiver is not in a saved state."),
    ///         }
    ///     })
    ///     .default();
    /// # record.push(Add('a'))?;
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn signals<F>(mut self, f: F) -> RecordBuilder<'a, R, C>
        where
            F: FnMut(Signal) + Send + Sync + 'a,
    {
        self.signals = Some(Box::new(f));
        self
    }

    /// Creates the record.
    #[inline]
    pub fn build<T: Into<R>>(self, receiver: T) -> Record<'a, R, C> {
        Record {
            commands: VecDeque::with_capacity(self.capacity),
            receiver: receiver.into(),
            cursor: 0,
            limit: self.limit,
            saved: None,
            signals: self.signals,
        }
    }
}

impl<'a, R: Default, C: Command<R>> RecordBuilder<'a, R, C> {
    /// Creates the record with a default `receiver`.
    #[inline]
    pub fn default(self) -> Record<'a, R, C> {
        self.build(R::default())
    }
}

impl<'a, R: Debug, C: Command<R> + Debug> Debug for RecordBuilder<'a, R, C> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Config")
            .field("receiver", &self.receiver)
            .field("capacity", &self.capacity)
            .field("limit", &self.limit)
            .finish()
    }
}
