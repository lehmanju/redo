use std::collections::hash_map;
use std::fmt;
use fnv::FnvHashMap;
use {DebugFn, Key, Result, RedoCmd, RedoStack};

/// A collection of `RedoStack`s.
///
/// A `RedoGroup` is useful when working with multiple stacks and only one of them should
/// be active at a given time, eg. a text editor with multiple documents opened. However, if only
/// a single stack is needed, it is easier to just use the stack directly.
#[derive(Default)]
pub struct RedoGroup<'a, T> {
    // The stacks in the group.
    group: FnvHashMap<Key, RedoStack<'a, T>>,
    // The active stack.
    active: Option<Key>,
    // Counter for generating new keys.
    key: u32,
    // Called when the active stack changes.
    on_stack_change: Option<Box<FnMut(Option<bool>) + 'a>>,
}

impl<'a, T> RedoGroup<'a, T> {
    /// Creates a new `RedoGroup`.
    #[inline]
    pub fn new() -> RedoGroup<'a, T> {
        RedoGroup {
            group: FnvHashMap::default(),
            active: None,
            key: 0,
            on_stack_change: None,
        }
    }

    /// Creates a new `RedoGroup` with the specified [capacity].
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn with_capacity(capacity: usize) -> RedoGroup<'a, T> {
        RedoGroup {
            group: FnvHashMap::with_capacity_and_hasher(capacity, Default::default()),
            ..RedoGroup::new()
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
    pub fn add(&mut self, stack: RedoStack<'a, T>) -> Key {
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
    pub fn remove(&mut self, key: Key) -> Option<RedoStack<'a, T>> {
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
    pub fn stacks(&'a self) -> Stacks<'a, T> {
        Stacks(self.group.iter())
    }

    /// Returns an iterator over the `(&Key, &mut RedoStack)` pairs in the group.
    #[inline]
    pub fn stacks_mut(&'a mut self) -> StacksMut<'a, T> {
        StacksMut(self.group.iter_mut())
    }
}

impl<'a, T: RedoCmd> RedoGroup<'a, T> {
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
    pub fn push(&mut self, cmd: T) -> Option<Result<T::Err>> {
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
    pub fn redo(&mut self) -> Option<Result<T::Err>> {
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
    pub fn undo(&mut self) -> Option<Result<T::Err>> {
        self.active
            .and_then(|active| self.group.get_mut(&active))
            .map(|stack| stack.undo())
    }
}

#[derive(Debug)]
pub struct IntoStacks<'a, T>(hash_map::IntoIter<Key, RedoStack<'a, T>>);

impl<'a, T> Iterator for IntoStacks<'a, T> {
    type Item = (Key, RedoStack<'a, T>);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a, T> IntoIterator for RedoGroup<'a, T> {
    type Item = (Key, RedoStack<'a, T>);
    type IntoIter = IntoStacks<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoStacks(self.group.into_iter())
    }
}

#[derive(Debug)]
pub struct Stacks<'a, T: 'a>(hash_map::Iter<'a, Key, RedoStack<'a, T>>);

impl<'a, T> Iterator for Stacks<'a, T> {
    type Item = (&'a Key, &'a RedoStack<'a, T>);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a, T> IntoIterator for &'a RedoGroup<'a, T> {
    type Item = (&'a Key, &'a RedoStack<'a, T>);
    type IntoIter = Stacks<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        Stacks(self.group.iter())
    }
}

#[derive(Debug)]
pub struct StacksMut<'a, T: 'a>(hash_map::IterMut<'a, Key, RedoStack<'a, T>>);

impl<'a, T> Iterator for StacksMut<'a, T> {
    type Item = (&'a Key, &'a mut RedoStack<'a, T>);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a, T> IntoIterator for &'a mut RedoGroup<'a, T> {
    type Item = (&'a Key, &'a mut RedoStack<'a, T>);
    type IntoIter = StacksMut<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        StacksMut(self.group.iter_mut())
    }
}

impl<'a, T: Default> RedoGroup<'a, T> {
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

impl<'a, T: fmt::Debug> fmt::Debug for RedoGroup<'a, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("RedoGroup")
            .field("group", &self.group)
            .field("active", &self.active)
            .field("key", &self.key)
            .finish()
    }
}

/// Builder for `RedoGroup`.
///
/// # Examples
/// ```
/// # #![allow(unused_variables)]
/// # use redo::{self, RedoCmd, RedoGroupBuilder};
/// # #[derive(Clone, Copy, Default)]
/// # struct PopCmd;
/// # impl RedoCmd for PopCmd {
/// #   type Err = ();
/// #   fn redo(&mut self) -> redo::Result<()> { Ok(()) }
/// #   fn undo(&mut self) -> redo::Result<()> { Ok(()) }
/// # }
/// let group = RedoGroupBuilder::new()
///     .capacity(10)
///     .on_stack_change(|is_clean| {
///         match is_clean {
///             Some(true) => { /* The new active stack is clean */ },
///             Some(false) => { /* The new active stack is dirty */ },
///             None => { /* No active stack */ },
///         }
///     })
///     .build::<PopCmd>();
/// ```
#[derive(Default)]
pub struct RedoGroupBuilder<'a> {
    capacity: usize,
    on_stack_change: Option<Box<FnMut(Option<bool>) + 'a>>,
}

impl<'a> RedoGroupBuilder<'a> {
    /// Creates a new builder.
    #[inline]
    pub fn new() -> RedoGroupBuilder<'a> {
        Default::default()
    }

    /// Sets the specified [capacity] for the group.
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> RedoGroupBuilder<'a> {
        self.capacity = capacity;
        self
    }

    /// Sets what should happen when the active stack changes.
    /// By default the `RedoGroup` does nothing when the active stack changes.
    ///
    /// # Examples
    /// ```
    /// # #![allow(unused_variables)]
    /// # use redo::{self, RedoCmd, RedoGroupBuilder};
    /// # #[derive(Clone, Copy, Default)]
    /// # struct PopCmd;
    /// # impl RedoCmd for PopCmd {
    /// #   type Err = ();
    /// #   fn redo(&mut self) -> redo::Result<()> { Ok(()) }
    /// #   fn undo(&mut self) -> redo::Result<()> { Ok(()) }
    /// # }
    /// let group = RedoGroupBuilder::new()
    ///     .on_stack_change(|is_clean| {
    ///         match is_clean {
    ///             Some(true) => { /* The new active stack is clean */ },
    ///             Some(false) => { /* The new active stack is dirty */ },
    ///             None => { /* No active stack */ },
    ///         }
    ///     })
    ///     .build::<PopCmd>();
    /// ```
    #[inline]
    pub fn on_stack_change<F>(mut self, f: F) -> RedoGroupBuilder<'a>
        where F: FnMut(Option<bool>) + 'a
    {
        self.on_stack_change = Some(Box::new(f));
        self
    }

    /// Builds the `RedoGroup`.
    #[inline]
    pub fn build<T>(self) -> RedoGroup<'a, T> {
        let RedoGroupBuilder {
            capacity,
            on_stack_change,
        } = self;
        RedoGroup {
            group: FnvHashMap::with_capacity_and_hasher(capacity, Default::default()),
            on_stack_change,
            ..RedoGroup::new()
        }
    }
}

impl<'a> fmt::Debug for RedoGroupBuilder<'a> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("UndoStackBuilder")
            .field("capacity", &self.capacity)
            .field("on_stack_change",
                   &self.on_stack_change.as_ref().map(|_| DebugFn))
            .finish()
    }
}

#[cfg(test)]
mod test {
    use super::*;

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
    fn active() {
        let mut vec1 = vec![1, 2, 3];
        let mut vec2 = vec![1, 2, 3];

        let mut group = RedoGroup::new();

        let a = group.add(RedoStack::new());
        let b = group.add(RedoStack::new());

        group.set_active(a);
        assert!(group
                    .push(PopCmd {
                              vec: &mut vec1,
                              e: None,
                          })
                    .unwrap()
                    .is_ok());
        assert_eq!(vec1.len(), 2);

        group.set_active(b);
        assert!(group
                    .push(PopCmd {
                              vec: &mut vec2,
                              e: None,
                          })
                    .unwrap()
                    .is_ok());
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
