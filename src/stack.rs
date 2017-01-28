use RedoCmd;

/// `RedoStack` maintains a stack of `RedoCmd`s that can be undone and redone by using methods
/// on the `RedoStack`.
#[derive(Debug, Default)]
pub struct RedoStack<T> {
    stack: Vec<T>,
    idx: usize,
}

impl<T: RedoCmd> RedoStack<T> {
    /// Creates a new `RedoStack`.
    #[inline]
    pub fn new() -> Self {
        RedoStack {
            stack: Vec::new(),
            idx: 0,
        }
    }

    /// Creates a new `RedoStack` with the specified capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        RedoStack {
            stack: Vec::with_capacity(capacity),
            idx: 0,
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

    /// Pushes a `RedoCmd` to the top of the `RedoStack` and executes its [`redo`] method.
    /// This pops off all `RedoCmd`s that is above the active command from the `RedoStack`.
    ///
    /// [`redo`]: trait.RedoCmd.html#tymethod.redo
    #[inline]
    pub fn push(&mut self, mut cmd: T) {
        cmd.redo();
        self.stack.truncate(self.idx);
        self.idx += 1;
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
            unsafe {
                let cmd = self.stack.get_unchecked_mut(self.idx);
                cmd.undo();
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::rc::Rc;
    use std::cell::RefCell;
    use {RedoStack, RedoCmd};

    /// Pops an element from a vector.
    #[derive(Clone, Default)]
    struct PopCmd {
        vec: Rc<RefCell<Vec<i32>>>,
        e: Option<i32>,
    }

    impl RedoCmd for PopCmd {
        fn redo(&mut self) {
            self.e = self.vec.borrow_mut().pop();
        }

        fn undo(&mut self) {
            self.vec.borrow_mut().push(self.e.unwrap());
            self.e = None;
        }
    }

    #[test]
    fn pop() {
        let vec = Rc::new(RefCell::new(vec![1, 2, 3]));
        let mut redo_stack = RedoStack::new();

        let cmd = PopCmd { vec: vec.clone(), e: None };
        redo_stack.push(cmd.clone());
        redo_stack.push(cmd.clone());
        redo_stack.push(cmd.clone());
        assert!(vec.borrow().is_empty());

        redo_stack.undo();
        redo_stack.undo();
        redo_stack.undo();
        assert_eq!(vec.borrow().len(), 3);

        redo_stack.push(cmd.clone());
        assert_eq!(vec.borrow().len(), 2);

        redo_stack.undo();
        assert_eq!(vec.borrow().len(), 3);

        redo_stack.redo();
        assert_eq!(vec.borrow().len(), 2);
    }
}
