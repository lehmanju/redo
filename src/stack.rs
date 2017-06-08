use std::collections::VecDeque;
use std::fmt;
use {DebugFn, Result, RedoCmd};

/// Maintains a stack of `RedoCmd`s.
///
/// `RedoStack` uses static dispatch so it can only hold one type of command at a given time.
///
/// When its state changes to either dirty or clean, it calls the user defined method
/// set in [`on_state_change`]. This is useful if you want to trigger some
/// event when the state changes, eg. enabling and disabling undo and redo buttons.
///
/// [`on_state_change`]: struct.RedoStackBuilder.html#method.on_state_change
#[derive(Default)]
pub struct RedoStack<'a, T> {
    // All commands on the stack.
    stack: VecDeque<T>,
    // Current position in the stack.
    idx: usize,
    // Max amount of commands allowed on the stack.
    limit: Option<usize>,
    // Called when the state changes.
    on_state_change: Option<Box<FnMut(bool) + 'a>>,
}

impl<'a, T> RedoStack<'a, T> {
    /// Creates a new `RedoStack`.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoStack};
    /// # struct A(u8);
    /// # impl RedoCmd for A {
    /// #   type Err = ();
    /// #   fn redo(&mut self) -> redo::Result<()> { Ok(()) }
    /// #   fn undo(&mut self) -> redo::Result<()> { Ok(()) }
    /// # }
    /// let mut stack = RedoStack::new();
    /// # stack.push(A(1)).unwrap();
    /// ```
    #[inline]
    pub fn new() -> RedoStack<'a, T> {
        RedoStack {
            stack: VecDeque::new(),
            idx: 0,
            limit: None,
            on_state_change: None,
        }
    }

    /// Creates a new `RedoStack` with a limit on how many `RedoCmd`s can be stored in the stack.
    /// If this limit is reached it will start popping of commands at the bottom of the stack when
    /// pushing new commands on to the stack. No limit is set by default which means it may grow
    /// indefinitely.
    ///
    /// The stack may remove multiple commands at a time to increase performance.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoStack};
    /// # #[derive(Clone, Copy)]
    /// # struct PopCmd {
    /// #   vec: *mut Vec<i32>,
    /// #   e: Option<i32>,
    /// # }
    /// # impl RedoCmd for PopCmd {
    /// #   type Err = ();
    /// #   fn redo(&mut self) -> redo::Result<()> {
    /// #       self.e = unsafe {
    /// #           let ref mut vec = *self.vec;
    /// #           vec.pop()
    /// #       };
    /// #       Ok(())
    /// #   }
    /// #   fn undo(&mut self) -> redo::Result<()> {
    /// #       unsafe {
    /// #           let ref mut vec = *self.vec;
    /// #           let e = self.e.ok_or(())?;
    /// #           vec.push(e);
    /// #       }
    /// #       Ok(())
    /// #   }
    /// # }
    /// # fn foo() -> redo::Result<()> {
    /// let mut vec = vec![1, 2, 3];
    /// let mut stack = RedoStack::with_limit(2);
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    /// stack.push(cmd)?; // Pops off the first cmd.
    ///
    /// assert!(vec.is_empty());
    ///
    /// stack.undo()?;
    /// stack.undo()?;
    /// stack.undo()?; // Does nothing.
    ///
    /// assert_eq!(vec, vec![1, 2]);
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn with_limit(limit: usize) -> RedoStack<'a, T> {
        RedoStack {
            stack: VecDeque::new(),
            idx: 0,
            limit: if limit == 0 { None } else { Some(limit) },
            on_state_change: None,
        }
    }

    /// Creates a new `RedoStack` with the specified capacity.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoStack};
    /// # struct A(u8);
    /// # impl RedoCmd for A {
    /// #   type Err = ();
    /// #   fn redo(&mut self) -> redo::Result<()> { Ok(()) }
    /// #   fn undo(&mut self) -> redo::Result<()> { Ok(()) }
    /// # }
    /// let mut stack = RedoStack::with_capacity(10);
    /// assert!(stack.capacity() >= 10);
    /// # stack.push(A(0)).unwrap();
    /// ```
    #[inline]
    pub fn with_capacity(capacity: usize) -> RedoStack<'a, T> {
        RedoStack {
            stack: VecDeque::with_capacity(capacity),
            idx: 0,
            limit: None,
            on_state_change: None,
        }
    }

    /// Creates a new `RedoStack` with the specified capacity and limit.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoStack};
    /// # struct A(u8);
    /// # impl RedoCmd for A {
    /// #   type Err = ();
    /// #   fn redo(&mut self) -> redo::Result<()> { Ok(()) }
    /// #   fn undo(&mut self) -> redo::Result<()> { Ok(()) }
    /// # }
    /// let mut stack = RedoStack::with_capacity_and_limit(10, 10);
    /// assert!(stack.capacity() >= 10);
    /// assert_eq!(stack.limit(), Some(10));
    /// # stack.push(A(0)).unwrap();
    /// ```
    #[inline]
    pub fn with_capacity_and_limit(capacity: usize, limit: usize) -> RedoStack<'a, T> {
        RedoStack {
            stack: VecDeque::with_capacity(capacity),
            idx: 0,
            limit: if limit == 0 { None } else { Some(limit) },
            on_state_change: None,
        }
    }

    /// Returns the limit of the `RedoStack`, or `None` if it has no limit.
    ///
    /// # Examples
    /// ```rust
    /// # use redo::{self, RedoCmd, RedoStack};
    /// # struct A(u8);
    /// # impl RedoCmd for A {
    /// #   type Err = ();
    /// #   fn redo(&mut self) -> redo::Result<()> { Ok(()) }
    /// #   fn undo(&mut self) -> redo::Result<()> { Ok(()) }
    /// # }
    /// # fn foo() -> redo::Result<()> {
    /// let mut stack = RedoStack::with_limit(10);
    /// assert_eq!(stack.limit(), Some(10));
    /// # stack.push(A(0))?;
    ///
    /// let mut stack = RedoStack::new();
    /// assert_eq!(stack.limit(), None);
    /// # stack.push(A(0))?;
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// Returns the capacity of the `RedoStack`.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoStack};
    /// # struct A(u8);
    /// # impl RedoCmd for A {
    /// #   type Err = ();
    /// #   fn redo(&mut self) -> redo::Result<()> { Ok(()) }
    /// #   fn undo(&mut self) -> redo::Result<()> { Ok(()) }
    /// # }
    /// let mut stack = RedoStack::with_capacity(10);
    /// assert!(stack.capacity() >= 10);
    /// # stack.push(A(0)).unwrap();
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        self.stack.capacity()
    }

    /// Reserves capacity for at least `additional` more commands to be inserted in the given stack.
    /// The stack may reserve more space to avoid frequent reallocations.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoStack};
    /// # #[derive(Clone, Copy)]
    /// # struct PopCmd {
    /// #     vec: *mut Vec<i32>,
    /// #     e: Option<i32>,
    /// # }
    /// # impl RedoCmd for PopCmd {
    /// #     type Err = ();
    /// #     fn redo(&mut self) -> redo::Result<()> {
    /// #         self.e = unsafe {
    /// #             let ref mut vec = *self.vec;
    /// #             vec.pop()
    /// #         };
    /// #         Ok(())
    /// #     }
    /// #     fn undo(&mut self) -> redo::Result<()> {
    /// #         unsafe {
    /// #             let ref mut vec = *self.vec;
    /// #             let e = self.e.ok_or(())?;
    /// #             vec.push(e);
    /// #         }
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> redo::Result<()> {
    /// let mut vec = vec![1, 2, 3];
    /// let mut stack = RedoStack::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// stack.push(cmd)?;
    /// stack.reserve(10);
    /// assert!(stack.capacity() >= 11);
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.stack.reserve(additional);
    }

    /// Shrinks the capacity of the `RedoStack` as much as possible.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoStack};
    /// # #[derive(Clone, Copy)]
    /// # struct PopCmd {
    /// #     vec: *mut Vec<i32>,
    /// #     e: Option<i32>,
    /// # }
    /// # impl RedoCmd for PopCmd {
    /// #     type Err = ();
    /// #     fn redo(&mut self) -> redo::Result<()> {
    /// #         self.e = unsafe {
    /// #             let ref mut vec = *self.vec;
    /// #             vec.pop()
    /// #         };
    /// #         Ok(())
    /// #     }
    /// #     fn undo(&mut self) -> redo::Result<()> {
    /// #         unsafe {
    /// #             let ref mut vec = *self.vec;
    /// #             let e = self.e.ok_or(())?;
    /// #             vec.push(e);
    /// #         }
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> redo::Result<()> {
    /// let mut vec = vec![1, 2, 3];
    /// let mut stack = RedoStack::with_capacity(10);
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    ///
    /// assert!(stack.capacity() >= 10);
    /// stack.shrink_to_fit();
    /// assert!(stack.capacity() >= 3);
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.stack.shrink_to_fit();
    }

    /// Returns `true` if the state of the stack is clean, `false` otherwise.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoStack};
    /// # #[derive(Clone, Copy)]
    /// # struct PopCmd {
    /// #     vec: *mut Vec<i32>,
    /// #     e: Option<i32>,
    /// # }
    /// # impl RedoCmd for PopCmd {
    /// #     type Err = ();
    /// #     fn redo(&mut self) -> redo::Result<()> {
    /// #         self.e = unsafe {
    /// #             let ref mut vec = *self.vec;
    /// #             vec.pop()
    /// #         };
    /// #         Ok(())
    /// #     }
    /// #     fn undo(&mut self) -> redo::Result<()> {
    /// #         unsafe {
    /// #             let ref mut vec = *self.vec;
    /// #             let e = self.e.ok_or(())?;
    /// #             vec.push(e);
    /// #         }
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> redo::Result<()> {
    /// let mut vec = vec![1, 2, 3];
    /// let mut stack = RedoStack::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// assert!(stack.is_clean()); // An empty stack is always clean.
    /// stack.push(cmd)?;
    /// assert!(stack.is_clean());
    /// stack.undo()?;
    /// assert!(!stack.is_clean());
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn is_clean(&self) -> bool {
        self.idx == self.stack.len()
    }

    /// Returns `true` if the state of the stack is dirty, `false` otherwise.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoStack};
    /// # #[derive(Clone, Copy)]
    /// # struct PopCmd {
    /// #     vec: *mut Vec<i32>,
    /// #     e: Option<i32>,
    /// # }
    /// # impl RedoCmd for PopCmd {
    /// #     type Err = ();
    /// #     fn redo(&mut self) -> redo::Result<()> {
    /// #         self.e = unsafe {
    /// #             let ref mut vec = *self.vec;
    /// #             vec.pop()
    /// #         };
    /// #         Ok(())
    /// #     }
    /// #     fn undo(&mut self) -> redo::Result<()> {
    /// #         unsafe {
    /// #             let ref mut vec = *self.vec;
    /// #             let e = self.e.ok_or(())?;
    /// #             vec.push(e);
    /// #         }
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> redo::Result<()> {
    /// let mut vec = vec![1, 2, 3];
    /// let mut stack = RedoStack::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// assert!(!stack.is_dirty()); // An empty stack is always clean.
    /// stack.push(cmd)?;
    /// assert!(!stack.is_dirty());
    /// stack.undo()?;
    /// assert!(stack.is_dirty());
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn is_dirty(&self) -> bool {
        !self.is_clean()
    }
}

impl<'a, T: RedoCmd> RedoStack<'a, T> {
    /// Pushes `cmd` to the top of the stack and executes its [`redo`] method.
    /// This pops off all other commands above the active command from the stack.
    ///
    /// # Errors
    /// If an error occur when executing `redo` the error is returned as `Some(Err(_))`
    /// and the state of the stack is left unchanged.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoStack};
    /// # #[derive(Clone, Copy)]
    /// # struct PopCmd {
    /// #     vec: *mut Vec<i32>,
    /// #     e: Option<i32>,
    /// # }
    /// # impl RedoCmd for PopCmd {
    /// #     type Err = ();
    /// #     fn redo(&mut self) -> redo::Result<()> {
    /// #         self.e = unsafe {
    /// #             let ref mut vec = *self.vec;
    /// #             vec.pop()
    /// #         };
    /// #         Ok(())
    /// #     }
    /// #     fn undo(&mut self) -> redo::Result<()> {
    /// #         unsafe {
    /// #             let ref mut vec = *self.vec;
    /// #             let e = self.e.ok_or(())?;
    /// #             vec.push(e);
    /// #         }
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> redo::Result<()> {
    /// let mut vec = vec![1, 2, 3];
    /// let mut stack = RedoStack::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    ///
    /// assert!(vec.is_empty());
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    ///
    /// [`redo`]: trait.RedoCmd.html#tymethod.redo
    pub fn push(&mut self, mut cmd: T) -> Result<T::Err> {
        let is_dirty = self.is_dirty();
        let len = self.idx;
        cmd.redo()?;
        // Pop off all elements after len from stack.
        self.stack.truncate(len);

        match self.stack.back_mut().and_then(|last| last.merge(&cmd)) {
            Some(x) => x?,
            None => {
                match self.limit {
                    Some(limit) if len == limit => {
                        let _ = self.stack.pop_front();
                    }
                    _ => self.idx += 1,
                }
                self.stack.push_back(cmd);
            }
        }

        debug_assert_eq!(self.idx, self.stack.len());
        // State is always clean after a push, check if it was dirty before.
        if is_dirty {
            if let Some(ref mut f) = self.on_state_change {
                f(true);
            }
        }
        Ok(())
    }

    /// Calls the [`redo`] method for the active `RedoCmd` and sets the next `RedoCmd` as the new
    /// active one.
    ///
    /// # Errors
    /// If an error occur when executing `redo` the error is returned as `Some(Err(_))`
    /// and the state of the stack is left unchanged.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoStack};
    /// # #[derive(Clone, Copy)]
    /// # struct PopCmd {
    /// #     vec: *mut Vec<i32>,
    /// #     e: Option<i32>,
    /// # }
    /// # impl RedoCmd for PopCmd {
    /// #     type Err = ();
    /// #     fn redo(&mut self) -> redo::Result<()> {
    /// #         self.e = unsafe {
    /// #             let ref mut vec = *self.vec;
    /// #             vec.pop()
    /// #         };
    /// #         Ok(())
    /// #     }
    /// #     fn undo(&mut self) -> redo::Result<()> {
    /// #         unsafe {
    /// #             let ref mut vec = *self.vec;
    /// #             let e = self.e.ok_or(())?;
    /// #             vec.push(e);
    /// #         }
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> redo::Result<()> {
    /// let mut vec = vec![1, 2, 3];
    /// let mut stack = RedoStack::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    ///
    /// assert!(vec.is_empty());
    ///
    /// stack.undo()?;
    /// stack.undo()?;
    /// stack.undo()?;
    ///
    /// assert_eq!(vec, vec![1, 2, 3]);
    ///
    /// stack.redo()?;
    /// stack.redo()?;
    /// stack.redo()?;
    ///
    /// assert!(vec.is_empty());
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    ///
    /// [`redo`]: trait.RedoCmd.html#tymethod.redo
    #[inline]
    pub fn redo(&mut self) -> Result<T::Err> {
        if self.idx < self.stack.len() {
            let is_dirty = self.is_dirty();
            self.stack[self.idx].redo()?;
            self.idx += 1;
            // Check if stack went from dirty to clean.
            if is_dirty && self.is_clean() {
                if let Some(ref mut f) = self.on_state_change {
                    f(true);
                }
            }
        }
        Ok(())
    }

    /// Calls the [`undo`] method for the active `RedoCmd` and sets the previous `RedoCmd` as the
    /// new active one.
    ///
    /// # Errors
    /// If an error occur when executing `undo` the error is returned as `Some(Err(_))`
    /// and the state of the stack is left unchanged.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoStack};
    /// # #[derive(Clone, Copy)]
    /// # struct PopCmd {
    /// #     vec: *mut Vec<i32>,
    /// #     e: Option<i32>,
    /// # }
    /// # impl RedoCmd for PopCmd {
    /// #     type Err = ();
    /// #     fn redo(&mut self) -> redo::Result<()> {
    /// #         self.e = unsafe {
    /// #             let ref mut vec = *self.vec;
    /// #             vec.pop()
    /// #         };
    /// #         Ok(())
    /// #     }
    /// #     fn undo(&mut self) -> redo::Result<()> {
    /// #         unsafe {
    /// #             let ref mut vec = *self.vec;
    /// #             let e = self.e.ok_or(())?;
    /// #             vec.push(e);
    /// #         }
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> redo::Result<()> {
    /// let mut vec = vec![1, 2, 3];
    /// let mut stack = RedoStack::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    ///
    /// assert!(vec.is_empty());
    ///
    /// stack.undo()?;
    /// stack.undo()?;
    /// stack.undo()?;
    ///
    /// assert_eq!(vec, vec![1, 2, 3]);
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    ///
    /// [`undo`]: trait.RedoCmd.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Result<T::Err> {
        if self.idx > 0 {
            let is_clean = self.is_clean();
            self.stack[self.idx - 1].undo()?;
            self.idx -= 1;
            // Check if stack went from clean to dirty.
            if is_clean && self.is_dirty() {
                if let Some(ref mut f) = self.on_state_change {
                    f(false);
                }
            }
        }
        Ok(())
    }
}

impl<'a, T: fmt::Debug> fmt::Debug for RedoStack<'a, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("RedoStack")
            .field("stack", &self.stack)
            .field("idx", &self.idx)
            .field("limit", &self.limit)
            .field("on_state_change",
                   &self.on_state_change.as_ref().map(|_| DebugFn))
            .finish()
    }
}

/// Builder for `RedoStack`.
///
/// # Examples
/// ```
/// # #![allow(unused_variables)]
/// # use redo::{self, RedoCmd, RedoStackBuilder};
/// # #[derive(Clone, Copy)]
/// # struct PopCmd {
/// #     vec: *mut Vec<i32>,
/// #     e: Option<i32>,
/// # }
/// # impl RedoCmd for PopCmd {
/// #     type Err = ();
/// #     fn redo(&mut self) -> redo::Result<()> {
/// #         self.e = unsafe {
/// #             let ref mut vec = *self.vec;
/// #             vec.pop()
/// #         };
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self) -> redo::Result<()> {
/// #         unsafe {
/// #             let ref mut vec = *self.vec;
/// #             let e = self.e.ok_or(())?;
/// #             vec.push(e);
/// #         }
/// #         Ok(())
/// #     }
/// # }
/// let stack = RedoStackBuilder::new()
///     .capacity(10)
///     .limit(10)
///     .on_state_change(|is_clean| {
///         println!("{}", is_clean);
///     })
///     .build::<PopCmd>();
/// ```
#[derive(Default)]
pub struct RedoStackBuilder<'a> {
    capacity: usize,
    limit: Option<usize>,
    on_state_change: Option<Box<FnMut(bool) + 'a>>,
}

impl<'a> RedoStackBuilder<'a> {
    /// Creates the new builder.
    #[inline]
    pub fn new() -> RedoStackBuilder<'a> {
        Default::default()
    }

    /// Sets the specified [capacity] for the stack.
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> RedoStackBuilder<'a> {
        self.capacity = capacity;
        self
    }

    /// Sets a limit on how many `RedoCmd`s can be stored in the stack.
    /// If this limit is reached it will start popping of commands at the bottom of the stack when
    /// pushing new commands on to the stack. No limit is set by default which means it may grow
    /// indefinitely.
    #[inline]
    pub fn limit(mut self, limit: usize) -> RedoStackBuilder<'a> {
        self.limit = Some(limit);
        self
    }

    /// Sets what should happen if the state changes.
    /// By default the `RedoStack` does nothing when the state changes.
    ///
    /// # Examples
    /// ```
    /// # use std::cell::Cell;
    /// # use redo::{self, RedoCmd, RedoStackBuilder};
    /// # #[derive(Clone, Copy)]
    /// # struct PopCmd {
    /// #     vec: *mut Vec<i32>,
    /// #     e: Option<i32>,
    /// # }
    /// # impl RedoCmd for PopCmd {
    /// #     type Err = ();
    /// #     fn redo(&mut self) -> redo::Result<()> {
    /// #         self.e = unsafe {
    /// #             let ref mut vec = *self.vec;
    /// #             vec.pop()
    /// #         };
    /// #         Ok(())
    /// #     }
    /// #     fn undo(&mut self) -> redo::Result<()> {
    /// #         unsafe {
    /// #             let ref mut vec = *self.vec;
    /// #             let e = self.e.ok_or(())?;
    /// #             vec.push(e);
    /// #         }
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> redo::Result<()> {
    /// let mut vec = vec![1, 2, 3];
    /// let x = Cell::new(0);
    /// let mut stack = RedoStackBuilder::new()
    ///     .on_state_change(|is_clean| {
    ///         if is_clean {
    ///             x.set(0);
    ///         } else {
    ///             x.set(1);
    ///         }
    ///     })
    ///     .build();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    /// stack.push(cmd)?;
    /// stack.undo()?;
    /// assert_eq!(x.get(), 1);
    /// stack.redo()?;
    /// assert_eq!(x.get(), 0);
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn on_state_change<F>(mut self, f: F) -> RedoStackBuilder<'a>
        where F: FnMut(bool) + 'a
    {
        self.on_state_change = Some(Box::new(f));
        self
    }

    /// Builds the `RedoStack`.
    #[inline]
    pub fn build<T>(self) -> RedoStack<'a, T> {
        let RedoStackBuilder {
            capacity,
            limit,
            on_state_change,
        } = self;
        RedoStack {
            stack: VecDeque::with_capacity(capacity),
            idx: 0,
            limit,
            on_state_change,
        }
    }
}

impl<'a> fmt::Debug for RedoStackBuilder<'a> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("UndoStackBuilder")
            .field("capacity", &self.capacity)
            .field("limit", &self.limit)
            .field("on_state_change",
                   &self.on_state_change.as_ref().map(|_| DebugFn))
            .finish()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Clone, Copy)]
    struct PopCmd {
        vec: *mut Vec<i32>,
        e: Option<i32>,
    }

    impl RedoCmd for PopCmd {
        type Err = ();

        fn redo(&mut self) -> Result<()> {
            self.e = unsafe {
                let ref mut vec = *self.vec;
                vec.pop()
            };
            Ok(())
        }

        fn undo(&mut self) -> Result<()> {
            unsafe {
                let ref mut vec = *self.vec;
                let e = self.e.ok_or(())?;
                vec.push(e);
            }
            Ok(())
        }
    }

    #[test]
    fn state() {
        use std::cell::Cell;

        let x = Cell::new(0);
        let mut vec = vec![1, 2, 3];
        let mut stack = RedoStackBuilder::new()
            .on_state_change(|is_clean| if is_clean {
                                  x.set(0);
                              } else {
                                  x.set(1);
                              }).build();

        let cmd = PopCmd {
            vec: &mut vec,
            e: None,
        };
        for _ in 0..3 {
            stack.push(cmd).unwrap();
        }
        assert_eq!(x.get(), 0);
        assert!(vec.is_empty());

        for _ in 0..3 {
            stack.undo().unwrap();
        }
        assert_eq!(x.get(), 1);
        assert_eq!(vec, vec![1, 2, 3]);

        stack.push(cmd).unwrap();
        assert_eq!(x.get(), 0);
        assert_eq!(vec, vec![1, 2]);

        stack.undo().unwrap();
        assert_eq!(x.get(), 1);
        assert_eq!(vec, vec![1, 2, 3]);

        stack.redo().unwrap();
        assert_eq!(x.get(), 0);
        assert_eq!(vec, vec![1, 2]);
    }
}
