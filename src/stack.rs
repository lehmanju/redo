use RedoCmd;

/// `RedoStack` maintains a stack of `RedoCmd`s that can be undone and redone by using methods
/// on the `RedoStack`.
#[derive(Debug, Default)]
pub struct RedoStack<T> {
    stack: Vec<T>,
    idx: usize,
    limit: Option<usize>,
}

impl<T> RedoStack<T> {
    /// Creates a new `RedoStack`.
    #[inline]
    pub fn new() -> Self {
        RedoStack {
            stack: Vec::new(),
            idx: 0,
            limit: None,
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
    /// # use redo::{RedoCmd, RedoStack};
    /// # #[derive(Clone, Copy)]
    /// # struct PopCmd {
    /// #   vec: *mut Vec<i32>,
    /// #   e: Option<i32>,
    /// # }
    /// # impl RedoCmd for PopCmd {
    /// #   fn redo(&mut self) {
    /// #       self.e = unsafe {
    /// #           let ref mut vec = *self.vec;
    /// #           vec.pop()
    /// #       }
    /// #   }
    /// #   fn undo(&mut self) {
    /// #       unsafe {
    /// #           let ref mut vec = *self.vec;
    /// #           vec.push(self.e.unwrap());
    /// #       }
    /// #   }
    /// # }
    /// let mut vec = vec![1, 2, 3];
    /// let mut stack = RedoStack::with_limit(2);
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// stack.push(cmd);
    /// stack.push(cmd);
    /// stack.push(cmd); // Pops off the first cmd.
    ///
    /// assert!(vec.is_empty());
    ///
    /// stack.undo();
    /// stack.undo();
    /// stack.undo(); // Does nothing.
    ///
    /// assert_eq!(vec, vec![1, 2]);
    /// ```
    #[inline]
    pub fn with_limit(limit: usize) -> Self {
        RedoStack {
            stack: Vec::new(),
            idx: 0,
            limit: Some(limit),
        }
    }

    /// Creates a new `RedoStack` with the specified capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        RedoStack {
            stack: Vec::with_capacity(capacity),
            idx: 0,
            limit: None,
        }
    }

    /// Creates a new `RedoStack` with the specified capacity and limit.
    #[inline]
    pub fn with_capacity_and_limit(capacity: usize, limit: usize) -> Self {
        RedoStack {
            stack: Vec::with_capacity(capacity),
            idx: 0,
            limit: Some(limit),
        }
    }

    /// Returns the limit of the `RedoStack`, or `None` if it has no limit.
    ///
    /// # Examples
    /// ```rust
    /// # use redo::{RedoCmd, RedoStack};
    /// # struct A;
    /// # impl RedoCmd for A {
    /// #   fn redo(&mut self) {}
    /// #   fn undo(&mut self) {}
    /// # }
    /// let mut stack = RedoStack::with_limit(10);
    /// assert_eq!(stack.limit(), Some(10));
    /// # stack.push(A);
    ///
    /// let mut stack = RedoStack::new();
    /// assert_eq!(stack.limit(), None);
    /// # stack.push(A);
    /// ```
    #[inline]
    pub fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// Returns the capacity of the `RedoStack`.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.stack.capacity()
    }

    /// Reserves capacity for at least `additional` more commands to be inserted in the given stack.
    /// The stack may reserve more space to avoid frequent reallocations.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.stack.reserve(additional);
    }

    /// Shrinks the capacity of the `RedoStack` as much as possible.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.stack.shrink_to_fit();
    }
}

impl<T: RedoCmd> RedoStack<T> {
    /// Pushes a `RedoCmd` to the top of the `RedoStack` and executes its [`redo`] method.
    /// This pops off all `RedoCmd`s that is above the active command from the `RedoStack`.
    ///
    /// [`redo`]: trait.RedoCmd.html#tymethod.redo
    pub fn push(&mut self, mut cmd: T) {
        let len = self.idx;
        // Pop off all elements after len from stack.
        self.stack.truncate(len);
        cmd.redo();

        match self.stack.last_mut().and_then(|last| last.merge(&cmd)) {
            Some(_) => (),
            None => {
                match self.limit {
                    Some(limit) if len == limit => {
                        // Remove ~25% of the stack at once.
                        let x = len / 4 + 1;
                        self.stack.drain(..x);
                        self.idx -= x - 1;
                    },
                    _ => self.idx += 1,
                }
                self.stack.push(cmd);
            }
        }

        debug_assert_eq!(self.idx, self.stack.len());
    }

    /// Calls the [`redo`] method for the active `RedoCmd` and sets the next `RedoCmd` as the new
    /// active one.
    ///
    /// Calling this method when there are no more commands to redo does nothing.
    ///
    /// [`redo`]: trait.RedoCmd.html#tymethod.redo
    #[inline]
    pub fn redo(&mut self) {
        if let Some(cmd) = self.stack.get_mut(self.idx) {
            cmd.redo();
            self.idx += 1;
        }
    }

    /// Calls the [`undo`] method for the active `RedoCmd` and sets the previous `RedoCmd` as the
    /// new active one.
    ///
    /// Calling this method when there are no more commands to undo does nothing.
    ///
    /// [`undo`]: trait.RedoCmd.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) {
        if self.idx > 0 {
            self.idx -= 1;
            debug_assert!(self.idx < self.stack.len());
            unsafe {
                let cmd = self.stack.get_unchecked_mut(self.idx);
                cmd.undo();
            }
        }
    }
}

#[cfg(test)]
mod test {
    use {RedoStack, RedoCmd};

    #[derive(Clone, Copy)]
    struct PopCmd {
        vec: *mut Vec<i32>,
        e: Option<i32>,
    }

    impl RedoCmd for PopCmd {
        fn redo(&mut self) {
            self.e = unsafe {
                let ref mut vec = *self.vec;
                vec.pop()
            }
        }

        fn undo(&mut self) {
            unsafe {
                let ref mut vec = *self.vec;
                vec.push(self.e.unwrap());
            }
        }
    }

    #[test]
    fn pop() {
        let mut vec = vec![1, 2, 3];
        let mut stack = RedoStack::new();

        let cmd = PopCmd { vec: &mut vec, e: None };
        stack.push(cmd);
        stack.push(cmd);
        stack.push(cmd);
        assert!(vec.is_empty());

        stack.undo();
        stack.undo();
        stack.undo();
        assert_eq!(vec.len(), 3);

        stack.push(cmd);
        assert_eq!(vec.len(), 2);

        stack.undo();
        assert_eq!(vec.len(), 3);

        stack.redo();
        assert_eq!(vec.len(), 2);
    }

    #[test]
    fn limit() {
        let mut vec = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let mut stack = RedoStack::with_limit(9);

        let cmd = PopCmd { vec: &mut vec, e: None };

        for _ in 0..10 {
            stack.push(cmd);
        }

        assert!(vec.is_empty());
        assert_eq!(stack.stack.len(), 7);
    }
}
