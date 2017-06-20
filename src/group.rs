use std::collections::hash_map;
use std::marker::PhantomData;
use std::fmt::{self, Debug, Formatter};
use fnv::FnvHashMap;
use {Command, Key, Result, Stack};

/// A collection of `RedoStack`s.
///
/// A `RedoGroup` is useful when working with multiple stacks and only one of them should
/// be active at a given time, eg. a text editor with multiple documents opened. However, if only
/// a single stack is needed, it is easier to just use the stack directly.
#[derive(Default)]
pub struct Group<'a, T, C: Command<T>> {
    // The stacks in the group.
    group: FnvHashMap<Key, Stack<'a, T, C>>,
    // The active stack.
    active: Option<Key>,
    // Counter for generating new keys.
    key: u32,
    // Called when the active stack changes.
    on_stack_change: Option<Box<FnMut(Option<bool>) + 'a>>,
}

impl<'a, T, C: Command<T>> Group<'a, T, C> {
    /// Creates a new `RedoGroup`.
    #[inline]
    pub fn new() -> Group<'a, T, C> {
        Group {
            group: FnvHashMap::default(),
            active: None,
            key: 0,
            on_stack_change: None,
        }
    }

    /// Creates a configurator that can be used to configure the `RedoGroup`.
    ///
    /// The configurator can set the `capacity` and what should happen when the active stack
    /// changes.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoGroup};
    /// # #[derive(Clone, Copy, Default)]
    /// # struct PopCmd;
    /// # impl RedoCmd for PopCmd {
    /// #   type Err = ();
    /// #   fn redo(&mut self) -> redo::Result<()> { Ok(()) }
    /// #   fn undo(&mut self) -> redo::Result<()> { Ok(()) }
    /// # }
    /// let _ = RedoGroup::<PopCmd>::config()
    ///     .capacity(10)
    ///     .on_stack_change(|is_clean| {
    ///         match is_clean {
    ///             Some(true) => { /* The new active stack is clean */ },
    ///             Some(false) => { /* The new active stack is dirty */ },
    ///             None => { /* No active stack */ },
    ///         }
    ///     })
    ///     .finish();
    /// ```
    #[inline]
    pub fn config() -> Config<'a, T, C> {
        Config {
            stack: PhantomData,
            receiver: PhantomData,
            capacity: 0,
            on_stack_change: None,
        }
    }

    /// Creates a new `RedoGroup` with the specified [capacity].
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn with_capacity(capacity: usize) -> Group<'a, T, C> {
        Group {
            group: FnvHashMap::with_capacity_and_hasher(capacity, Default::default()),
            ..Group::new()
        }
    }

    /// Returns the capacity of the `RedoGroup`.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.group.capacity()
    }

    /// Reserves capacity for at least `additional` more stacks to be inserted in the given group.
    /// The group may reserve more space to avoid frequent reallocations.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoGroup};
    /// # #[derive(Clone, Copy, Default)]
    /// # struct PopCmd;
    /// # impl RedoCmd for PopCmd {
    /// #   type Err = ();
    /// #   fn redo(&mut self) -> redo::Result<()> { Ok(()) }
    /// #   fn undo(&mut self) -> redo::Result<()> { Ok(()) }
    /// # }
    /// let mut group = RedoGroup::<PopCmd>::new();
    /// group.add_default();
    /// group.reserve(10);
    /// assert!(group.capacity() >= 11);
    /// ```
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.group.reserve(additional);
    }

    /// Shrinks the capacity of the `RedoGroup` as much as possible.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoGroup};
    /// # #[derive(Clone, Copy, Default)]
    /// # struct PopCmd;
    /// # impl RedoCmd for PopCmd {
    /// #   type Err = ();
    /// #   fn redo(&mut self) -> redo::Result<()> { Ok(()) }
    /// #   fn undo(&mut self) -> redo::Result<()> { Ok(()) }
    /// # }
    /// let mut group = RedoGroup::<PopCmd>::with_capacity(10);
    /// group.add_default();
    /// group.add_default();
    /// group.add_default();
    ///
    /// assert!(group.capacity() >= 10);
    /// group.shrink_to_fit();
    /// assert!(group.capacity() >= 3);
    /// ```
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.group.shrink_to_fit();
    }

    /// Returns the number of stacks in the group.
    #[inline]
    pub fn len(&self) -> usize {
        self.group.len()
    }

    /// Adds an `RedoStack` to the group and returns an unique id for this stack.
    ///
    /// # Examples
    /// ```
    /// # #![allow(unused_variables)]
    /// # use redo::{self, RedoCmd, RedoGroup};
    /// # #[derive(Clone, Copy, Default)]
    /// # struct PopCmd;
    /// # impl RedoCmd for PopCmd {
    /// #   type Err = ();
    /// #   fn redo(&mut self) -> redo::Result<()> { Ok(()) }
    /// #   fn undo(&mut self) -> redo::Result<()> { Ok(()) }
    /// # }
    /// let mut group = RedoGroup::<PopCmd>::new();
    /// let a = group.add_default();
    /// let b = group.add_default();
    /// let c = group.add_default();
    /// ```
    #[inline]
    pub fn add(&mut self, stack: Stack<'a, T, C>) -> Key {
        let key = Key(self.key);
        self.key += 1;
        self.group.insert(key, stack);
        key
    }

    /// Removes the `RedoStack` with the specified id and returns the stack.
    /// Returns `None` if the stack was not found.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoGroup};
    /// # #[derive(Clone, Copy, Default)]
    /// # struct PopCmd;
    /// # impl RedoCmd for PopCmd {
    /// #   type Err = ();
    /// #   fn redo(&mut self) -> redo::Result<()> { Ok(()) }
    /// #   fn undo(&mut self) -> redo::Result<()> { Ok(()) }
    /// # }
    /// let mut group = RedoGroup::<PopCmd>::new();
    /// let a = group.add_default();
    /// let stack = group.remove(a);
    /// assert!(stack.is_some());
    /// ```
    #[inline]
    pub fn remove(&mut self, key: Key) -> Option<Stack<'a, T, C>> {
        // Check if it was the active stack that was removed.
        if let Some(active) = self.active {
            if active == key {
                self.clear_active();
            }
        }
        self.group.remove(&key)
    }

    /// Sets the `RedoStack` with the specified id as the current active one.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoGroup};
    /// # #[derive(Clone, Copy, Default)]
    /// # struct PopCmd;
    /// # impl RedoCmd for PopCmd {
    /// #   type Err = ();
    /// #   fn redo(&mut self) -> redo::Result<()> { Ok(()) }
    /// #   fn undo(&mut self) -> redo::Result<()> { Ok(()) }
    /// # }
    /// let mut group = RedoGroup::<PopCmd>::new();
    /// let a = group.add_default();
    /// group.set_active(a);
    /// ```
    #[inline]
    pub fn set_active(&mut self, key: Key) {
        if let Some(is_clean) = self.group.get(&key).map(|stack| stack.is_clean()) {
            self.active = Some(key);
            if let Some(ref mut f) = self.on_stack_change {
                f(Some(is_clean));
            }
        }
    }

    /// Clears the current active `RedoStack`.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoGroup};
    /// # #[derive(Clone, Copy, Default)]
    /// # struct PopCmd;
    /// # impl RedoCmd for PopCmd {
    /// #   type Err = ();
    /// #   fn redo(&mut self) -> redo::Result<()> { Ok(()) }
    /// #   fn undo(&mut self) -> redo::Result<()> { Ok(()) }
    /// # }
    /// let mut group = RedoGroup::<PopCmd>::new();
    /// let a = group.add_default();
    /// group.set_active(a);
    /// group.clear_active();
    /// ```
    #[inline]
    pub fn clear_active(&mut self) {
        self.active = None;
        if let Some(ref mut f) = self.on_stack_change {
            f(None);
        }
    }

    /// Calls [`is_clean`] on the active `RedoStack`, if there is one.
    /// Returns `None` if there is no active stack.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoStack, RedoGroup};
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
    /// let mut vec = vec![1, 2, 3];
    /// let mut group = RedoGroup::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// let a = group.add(RedoStack::new());
    /// assert_eq!(group.is_clean(), None);
    /// group.set_active(a);
    ///
    /// assert_eq!(group.is_clean(), Some(true)); // An empty stack is always clean.
    /// group.push(cmd);
    /// assert_eq!(group.is_clean(), Some(true));
    /// group.undo();
    /// assert_eq!(group.is_clean(), Some(false));
    /// ```
    ///
    /// [`is_clean`]: struct.RedoStack.html#method.is_clean
    #[inline]
    pub fn is_clean(&self) -> Option<bool> {
        self.active.map(|i| self.group[&i].is_clean())
    }

    /// Calls [`is_dirty`] on the active `RedoStack`, if there is one.
    /// Returns `None` if there is no active stack.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoStack, RedoGroup};
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
    /// let mut vec = vec![1, 2, 3];
    /// let mut group = RedoGroup::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// let a = group.add(RedoStack::new());
    /// assert_eq!(group.is_dirty(), None);
    /// group.set_active(a);
    ///
    /// assert_eq!(group.is_dirty(), Some(false)); // An empty stack is always clean.
    /// group.push(cmd);
    /// assert_eq!(group.is_dirty(), Some(false));
    /// group.undo();
    /// assert_eq!(group.is_dirty(), Some(true));
    /// ```
    ///
    /// [`is_dirty`]: struct.RedoStack.html#method.is_dirty
    #[inline]
    pub fn is_dirty(&self) -> Option<bool> {
        self.is_clean().map(|t| !t)
    }

    /// Returns an iterator over the `(&Key, &RedoStack)` pairs in the group.
    #[inline]
    pub fn stacks(&'a self) -> Stacks<'a, T, C> {
        Stacks(self.group.iter())
    }

    /// Returns an iterator over the `(&Key, &mut RedoStack)` pairs in the group.
    #[inline]
    pub fn stacks_mut(&'a mut self) -> StacksMut<'a, T, C> {
        StacksMut(self.group.iter_mut())
    }

    /// Calls [`push`] on the active `RedoStack`, if there is one.
    ///
    /// Returns `Some(Ok)` if everything went fine, `Some(Err)` if something went wrong, and `None`
    /// if there is no active stack.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoStack, RedoGroup};
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
    /// let mut vec = vec![1, 2, 3];
    /// let mut group = RedoGroup::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// let a = group.add(RedoStack::new());
    /// group.set_active(a);
    ///
    /// group.push(cmd);
    /// group.push(cmd);
    /// group.push(cmd);
    ///
    /// assert!(vec.is_empty());
    /// ```
    ///
    /// [`push`]: struct.RedoStack.html#method.push
    #[inline]
    pub fn push(&mut self, cmd: C) -> Option<Result<C::Err>> {
        self.active
            .and_then(|active| self.group.get_mut(&active))
            .map(|stack| stack.push(cmd))
    }

    /// Calls [`redo`] on the active `RedoStack`, if there is one.
    ///
    /// Returns `Some(Ok)` if everything went fine, `Some(Err)` if something went wrong, and `None`
    /// if there is no active stack.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoStack, RedoGroup};
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
    /// let mut vec = vec![1, 2, 3];
    /// let mut group = RedoGroup::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// let a = group.add(RedoStack::new());
    /// group.set_active(a);
    ///
    /// group.push(cmd);
    /// group.push(cmd);
    /// group.push(cmd);
    ///
    /// assert!(vec.is_empty());
    ///
    /// group.undo();
    /// group.undo();
    /// group.undo();
    ///
    /// assert_eq!(vec, vec![1, 2, 3]);
    ///
    /// group.redo();
    /// group.redo();
    /// group.redo();
    ///
    /// assert!(vec.is_empty());
    /// ```
    ///
    /// [`redo`]: struct.RedoStack.html#method.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result<C::Err>> {
        self.active
            .and_then(|active| self.group.get_mut(&active))
            .map(|stack| stack.redo())
    }

    /// Calls [`undo`] on the active `RedoStack`, if there is one.
    ///
    /// Returns `Some(Ok)` if everything went fine, `Some(Err)` if something went wrong, and `None`
    /// if there is no active stack.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoStack, RedoGroup};
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
    /// let mut vec = vec![1, 2, 3];
    /// let mut group = RedoGroup::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// let a = group.add(RedoStack::new());
    /// group.set_active(a);
    ///
    /// group.push(cmd);
    /// group.push(cmd);
    /// group.push(cmd);
    ///
    /// assert!(vec.is_empty());
    ///
    /// group.undo();
    /// group.undo();
    /// group.undo();
    ///
    /// assert_eq!(vec, vec![1, 2, 3]);
    /// ```
    ///
    /// [`undo`]: struct.RedoStack.html#method.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result<C::Err>> {
        self.active
            .and_then(|active| self.group.get_mut(&active))
            .map(|stack| stack.undo())
    }
}

#[derive(Debug)]
pub struct IntoStacks<'a, T, C: Command<T>>(hash_map::IntoIter<Key, Stack<'a, T, C>>);

impl<'a, T, C: Command<T>> Iterator for IntoStacks<'a, T, C> {
    type Item = (Key, Stack<'a, T, C>);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a, T, C: Command<T>> IntoIterator for Group<'a, T, C> {
    type Item = (Key, Stack<'a, T, C>);
    type IntoIter = IntoStacks<'a, T, C>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoStacks(self.group.into_iter())
    }
}

#[derive(Debug)]
pub struct Stacks<'a, T: 'a, C: Command<T> + 'a>(hash_map::Iter<'a, Key, Stack<'a, T, C>>);

impl<'a, T, C: Command<T>> Iterator for Stacks<'a, T, C> {
    type Item = (&'a Key, &'a Stack<'a, T, C>);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a, T, C: Command<T>> IntoIterator for &'a Group<'a, T, C> {
    type Item = (&'a Key, &'a Stack<'a, T, C>);
    type IntoIter = Stacks<'a, T, C>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        Stacks(self.group.iter())
    }
}

#[derive(Debug)]
pub struct StacksMut<'a, T: 'a, C: Command<T> + 'a>(hash_map::IterMut<'a, Key, Stack<'a, T, C>>);

impl<'a, T, C: Command<T>> Iterator for StacksMut<'a, T, C> {
    type Item = (&'a Key, &'a mut Stack<'a, T, C>);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a, T, C: Command<T>> IntoIterator for &'a mut Group<'a, T, C> {
    type Item = (&'a Key, &'a mut Stack<'a, T, C>);
    type IntoIter = StacksMut<'a, T, C>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        StacksMut(self.group.iter_mut())
    }
}

impl<'a, T: Default, C: Command<T> + Default> Group<'a, T, C> {
    /// Adds a default `RedoStack` to the group and returns an unique id for this stack.
    ///
    /// # Examples
    /// ```
    /// # #![allow(unused_variables)]
    /// # use redo::{self, RedoCmd, RedoGroup};
    /// # #[derive(Clone, Copy, Default)]
    /// # struct PopCmd;
    /// # impl RedoCmd for PopCmd {
    /// #   type Err = ();
    /// #   fn redo(&mut self) -> redo::Result<()> { Ok(()) }
    /// #   fn undo(&mut self) -> redo::Result<()> { Ok(()) }
    /// # }
    /// let mut group = RedoGroup::<PopCmd>::new();
    /// let a = group.add_default();
    /// let b = group.add_default();
    /// let c = group.add_default();
    /// ```
    #[inline]
    pub fn add_default(&mut self) -> Key {
        self.add(Default::default())
    }
}

impl<'a, T: Debug, C: Command<T> + Debug> Debug for Group<'a, T, C> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Group")
            .field("group", &self.group)
            .field("active", &self.active)
            .field("key", &self.key)
            .finish()
    }
}

/// Configurator for `RedoGroup`.
#[derive(Default)]
pub struct Config<'a, T, C: Command<T>> {
    stack: PhantomData<T>,
    receiver: PhantomData<C>,
    capacity: usize,
    on_stack_change: Option<Box<FnMut(Option<bool>) + 'a>>,
}

impl<'a, T, C: Command<T>> Config<'a, T, C> {
    /// Sets the specified [capacity] for the group.
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> Config<'a, T, C> {
        self.capacity = capacity;
        self
    }

    /// Sets what should happen when the active stack changes.
    /// By default the `RedoGroup` does nothing when the active stack changes.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, RedoCmd, RedoGroup};
    /// # #[derive(Clone, Copy, Default)]
    /// # struct PopCmd;
    /// # impl RedoCmd for PopCmd {
    /// #   type Err = ();
    /// #   fn redo(&mut self) -> redo::Result<()> { Ok(()) }
    /// #   fn undo(&mut self) -> redo::Result<()> { Ok(()) }
    /// # }
    /// let _ = RedoGroup::<PopCmd>::config()
    ///     .on_stack_change(|is_clean| {
    ///         match is_clean {
    ///             Some(true) => { /* The new active stack is clean */ },
    ///             Some(false) => { /* The new active stack is dirty */ },
    ///             None => { /* No active stack */ },
    ///         }
    ///     })
    ///     .finish();
    /// ```
    #[inline]
    pub fn on_stack_change<F>(mut self, f: F) -> Config<'a, T, C>
    where
        F: FnMut(Option<bool>) + 'a,
    {
        self.on_stack_change = Some(Box::new(f));
        self
    }

    /// Builds the `RedoGroup`.
    #[inline]
    pub fn finish(self) -> Group<'a, T, C> {
        Group {
            group: FnvHashMap::with_capacity_and_hasher(self.capacity, Default::default()),
            on_stack_change: self.on_stack_change,
            ..Group::new()
        }
    }
}

impl<'a, T, C: Command<T>> Debug for Config<'a, T, C> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Config")
            .field("capacity", &self.capacity)
            .field(
                "on_stack_change",
                &self.on_stack_change.as_ref().map(|_| "|_| { .. }"),
            )
            .finish()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Clone, Copy)]
    struct PopCmd(Option<i32>);

    impl Command for PopCmd {
        type Err = ();

        fn redo(&mut self, vec: &mut Vec<i32>) -> Result<()> {
            self.e = vec.pop();
            Ok(())
        }

        fn undo(&self, vec: &mut Vec<i32>) -> Result<()> {
            let e = self.0.ok_or(())?;
            vec.push(e);
            Ok(())
        }
    }

    #[test]
    fn active() {
        let mut vec1 = vec![1, 2, 3];
        let mut vec2 = vec![1, 2, 3];

        let mut group = Group::new();

        let a = group.add(Stack::new(vec1));
        let b = group.add(Stack::new(vec2));

        group.set_active(a);
        assert!(group.push(PopCmd(None).unwrap().is_ok()));
        assert_eq!(vec1.len(), 2);

        group.set_active(b);
        assert!(group.push(PopCmd(None).unwrap().is_ok()));
        assert_eq!(vec2.len(), 2);

        group.set_active(a);
        assert!(group.undo().unwrap().is_ok());
        assert_eq!(vec1.len(), 3);

        group.set_active(b);
        assert!(group.undo().unwrap().is_ok());
        assert_eq!(vec2.len(), 3);

        assert!(group.remove(b).is_some());
        assert_eq!(group.group.len(), 1);

        assert!(group.redo().is_none());
        assert_eq!(vec2.len(), 3);
    }
}
