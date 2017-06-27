use std::collections::VecDeque;
use std::marker::PhantomData;
use std::fmt::{self, Debug, Formatter};
use std::borrow::Borrow;
use {Result, Command};

/// Maintains a stack of `Command`s.
///
/// `Stack` uses static dispatch so it can only hold one type of command at a given time.
///
/// When its state changes to either dirty or clean, it calls the user defined method
/// set when configuring the stack. This is useful if you want to trigger some
/// event when the state changes, eg. enabling and disabling undo and redo buttons.
#[derive(Default)]
pub struct Stack<'a, T, C: Command<T>> {
    // All commands on the stack.
    stack: VecDeque<C>,
    // The data being operated on.
    receiver: T,
    // Current position in the stack.
    idx: usize,
    // Max amount of commands allowed on the stack.
    limit: Option<usize>,
    // Called when the state changes.
    on_state_change: Option<Box<FnMut(bool) + 'a>>,
}

impl<'a, T, C: Command<T>> Stack<'a, T, C> {
    /// Creates a new `Stack`.
    #[inline]
    pub fn new(receiver: T) -> Stack<'a, T, C> {
        Stack {
            stack: VecDeque::new(),
            receiver,
            idx: 0,
            limit: None,
            on_state_change: None,
        }
    }

    /// Creates a configurator that can be used to configure the `Stack`.
    ///
    /// The configurator can set the `capacity`, `limit`, and what should happen when the state
    /// changes.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, Command, Stack};
    /// # #[derive(Clone, Copy)]
    /// # struct Pop(Option<u8>);
    /// # impl Command<Vec<u8>> for Pop {
    /// #   type Err = ();
    /// #   fn redo(&mut self, vec: &mut Vec<u8>) -> redo::Result<()> {
    /// #       self.0 = vec.pop();
    /// #       Ok(())
    /// #   }
    /// #   fn undo(&mut self, vec: &mut Vec<u8>) -> redo::Result<()> {
    /// #       let e = self.0.ok_or(())?;
    /// #       vec.push(e);
    /// #       Ok(())
    /// #   }
    /// # }
    /// let mut stack = Stack::config(vec![1, 2, 3])
    ///     .capacity(10)
    ///     .limit(10)
    ///     .on_state_change(|is_clean| {
    ///         if is_clean {
    ///             // ..
    ///         } else {
    ///             // ..
    ///         }
    ///     })
    ///     .finish();
    /// # stack.push(Pop(None)).unwrap();
    /// ```
    #[inline]
    pub fn config(receiver: T) -> Config<'a, T, C> {
        Config {
            receiver,
            capacity: 0,
            limit: None,
            on_state_change: None,
            phantom: PhantomData,
        }
    }

    /// Returns the limit of the `Stack`, or `None` if it has no limit.
    #[inline]
    pub fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// Returns the capacity of the `Stack`.
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

    /// Shrinks the capacity of the `Stack` as much as possible.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.stack.shrink_to_fit();
    }

    /// Returns `true` if the state of the stack is clean, `false` otherwise.
    #[inline]
    pub fn is_clean(&self) -> bool {
        self.idx == self.stack.len()
    }

    /// Returns `true` if the state of the stack is dirty, `false` otherwise.
    #[inline]
    pub fn is_dirty(&self) -> bool {
        !self.is_clean()
    }

    /// Consumes the `Stack`, returning the receiver.
    #[inline]
    pub fn into_receiver(self) -> T {
        self.receiver
    }

    /// Pushes `cmd` to the top of the stack and executes its [`redo`] method.
    /// This pops off all other commands above the active command from the stack.
    ///
    /// # Errors
    /// If an error occur when executing `redo` or merging commands, the error is returned.
    ///
    /// [`redo`]: trait.Command.html#tymethod.redo
    #[inline]
    pub fn push(&mut self, mut cmd: C) -> Result<C::Err> {
        let is_dirty = self.is_dirty();
        let len = self.idx;
        cmd.redo(&mut self.receiver)?;
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

    /// Calls the [`redo`] method for the active `Command` and sets the next `Command` as the new
    /// active one.
    ///
    /// # Errors
    /// If an error occur when executing `redo` the error is returned and the state of the stack is
    /// left unchanged.
    ///
    /// [`redo`]: trait.Command.html#tymethod.redo
    #[inline]
    pub fn redo(&mut self) -> Result<C::Err> {
        if self.idx < self.stack.len() {
            let is_dirty = self.is_dirty();
            self.stack[self.idx].redo(&mut self.receiver)?;
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

    /// Calls the [`undo`] method for the active `Command` and sets the previous `Command` as the
    /// new active one.
    ///
    /// # Errors
    /// If an error occur when executing `undo` the error is returned and the state of the stack is left unchanged.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Result<C::Err> {
        if self.idx > 0 {
            let is_clean = self.is_clean();
            self.stack[self.idx - 1].undo(&mut self.receiver)?;
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

impl<'a, T, C: Command<T>> Borrow<T> for Stack<'a, T, C> {
    #[inline]
    fn borrow(&self) -> &T {
        &self.receiver
    }
}

impl<'a, T: Debug, C: Command<T> + Debug> Debug for Stack<'a, T, C> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Stack")
            .field("stack", &self.stack)
            .field("receiver", &self.receiver)
            .field("idx", &self.idx)
            .field("limit", &self.limit)
            .field(
                "on_state_change",
                &self.on_state_change.as_ref().map(|_| "|_| { .. }"),
            )
            .finish()
    }
}

/// Configurator for the `Stack`.
#[derive(Default)]
pub struct Config<'a, T, C: Command<T>> {
    receiver: T,
    capacity: usize,
    limit: Option<usize>,
    on_state_change: Option<Box<FnMut(bool) + 'a>>,
    phantom: PhantomData<C>,
}

impl<'a, T, C: Command<T>> Config<'a, T, C> {
    /// Sets the specified [capacity] for the stack.
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> Config<'a, T, C> {
        self.capacity = capacity;
        self
    }

    /// Sets a limit on how many `Command`s can be stored in the stack.
    /// If this limit is reached it will start popping of commands at the bottom of the stack when
    /// pushing new commands on to the stack. No limit is set by default which means it may grow
    /// indefinitely.
    #[inline]
    pub fn limit(mut self, limit: usize) -> Config<'a, T, C> {
        self.limit = Some(limit);
        self
    }

    /// Sets what should happen when the state changes.
    #[inline]
    pub fn on_state_change<F>(mut self, f: F) -> Config<'a, T, C>
    where
        F: FnMut(bool) + 'a,
    {
        self.on_state_change = Some(Box::new(f));
        self
    }

    /// Returns the `Stack`.
    #[inline]
    pub fn finish(self) -> Stack<'a, T, C> {
        Stack {
            receiver: self.receiver,
            stack: VecDeque::with_capacity(self.capacity),
            idx: 0,
            limit: self.limit,
            on_state_change: self.on_state_change,
        }
    }
}

impl<'a, T: Debug, C: Command<T>> Debug for Config<'a, T, C> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Config")
            .field("receiver", &self.receiver)
            .field("capacity", &self.capacity)
            .field("limit", &self.limit)
            .field(
                "on_state_change",
                &self.on_state_change.as_ref().map(|_| "|_| { .. }"),
            )
            .finish()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Clone, Copy)]
    struct Pop(Option<u8>);

    impl Command<Vec<u8>> for Pop {
        type Err = ();

        fn redo(&mut self, vec: &mut Vec<u8>) -> Result<()> {
            self.0 = vec.pop();
            Ok(())
        }

        fn undo(&mut self, vec: &mut Vec<u8>) -> Result<()> {
            let e = self.0.ok_or(())?;
            vec.push(e);
            Ok(())
        }
    }

    #[test]
    fn state() {
        use std::cell::Cell;

        let x = Cell::new(0);
        let mut stack = Stack::config(vec![1, 2, 3])
            .on_state_change(|is_clean| if is_clean {
                x.set(0);
            } else {
                x.set(1);
            })
            .finish();

        let cmd = Pop(None);
        for _ in 0..3 {
            stack.push(cmd).unwrap();
        }
        assert_eq!(x.get(), 0);
        assert!({
            let stack: &Vec<_> = stack.borrow();
            stack.is_empty()
        });

        for _ in 0..3 {
            stack.undo().unwrap();
        }
        assert_eq!(x.get(), 1);
        assert_eq!(
            {
                let stack: &Vec<_> = stack.borrow();
                stack
            },
            &vec![1, 2, 3]
        );

        stack.push(cmd).unwrap();
        assert_eq!(x.get(), 0);
        assert_eq!(
            {
                let stack: &Vec<_> = stack.borrow();
                stack
            },
            &vec![1, 2]
        );

        stack.undo().unwrap();
        assert_eq!(x.get(), 1);
        assert_eq!(
            {
                let stack: &Vec<_> = stack.borrow();
                stack
            },
            &vec![1, 2, 3]
        );

        stack.redo().unwrap();
        assert_eq!(x.get(), 0);
        assert_eq!(stack.into_receiver(), vec![1, 2]);
    }
}
