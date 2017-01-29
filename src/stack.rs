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

    /// Creates a new `RedoStack` with the specified capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        RedoStack {
            stack: Vec::with_capacity(capacity),
            idx: 0,
            limit: None,
        }
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

    /// Sets the limit on how many `RedoCmd`s can be stored in the stack. If this limit is reached
    /// it will start popping of commands at the bottom of the stack when pushing new commands
    /// on to the stack. No limit is set by default which means it may grow indefinitely.
    ///
    /// # Panics
    /// Panics if the given limit is zero.
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
    /// let mut stack = RedoStack::new()
    ///     .limit(2);
    ///
    /// let cmd = PopCmd { vec: &mut vec, e: None };
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
    pub fn limit(mut self, limit: usize) -> Self {
        assert_ne!(limit, 0);

        if limit < self.idx {
            let x = self.idx - limit;
            self.stack.drain(..x);
            self.idx = limit;
            debug_assert_eq!(self.stack.len(), limit);
        }
        self.limit = Some(limit);
        self
    }
}

impl<T: RedoCmd> RedoStack<T> {
    /// Pushes a `RedoCmd` to the top of the `RedoStack` and executes its [`redo`] method.
    /// This pops off all `RedoCmd`s that is above the active command from the `RedoStack`.
    ///
    /// [`redo`]: trait.RedoCmd.html#tymethod.redo
    #[inline]
    pub fn push(&mut self, mut cmd: T) {
        cmd.redo();
        let idx = self.idx;
        self.stack.truncate(idx);
        match self.limit.map(|limit| idx == limit) {
            Some(false) | None => self.idx += 1,
            Some(true) => { self.stack.remove(0); },
        }
        self.stack.push(cmd);
    }

    /// Calls the [`redo`] method for the active `RedoCmd` and sets the next `RedoCmd` as the new
    /// active one.
    ///
    /// Calling this method when there are no more commands to redo does nothing.
    ///
    /// [`redo`]: trait.RedoCmd.html#tymethod.redo
    #[inline]
    pub fn redo(&mut self) {
        if self.idx < self.stack.len() {
            unsafe {
                let cmd = self.stack.get_unchecked_mut(self.idx);
                cmd.redo();
            }
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
        let mut redo_stack = RedoStack::new();

        let cmd = PopCmd { vec: &mut vec, e: None };
        redo_stack.push(cmd);
        redo_stack.push(cmd);
        redo_stack.push(cmd);
        assert!(vec.is_empty());

        redo_stack.undo();
        redo_stack.undo();
        redo_stack.undo();
        assert_eq!(vec.len(), 3);

        redo_stack.push(cmd);
        assert_eq!(vec.len(), 2);

        redo_stack.undo();
        assert_eq!(vec.len(), 3);

        redo_stack.redo();
        assert_eq!(vec.len(), 2);
    }
}
