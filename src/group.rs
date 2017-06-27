use std::collections::hash_map;
use std::marker::PhantomData;
use std::fmt::{self, Debug, Formatter};
use fnv::FnvHashMap;
use {Command, Key, Result, Stack};

/// A collection of `Stack`s.
///
/// A `Group` is useful when working with multiple stacks and only one of them should
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
    /// Creates a new `Group`.
    #[inline]
    pub fn new() -> Group<'a, T, C> {
        Group {
            group: FnvHashMap::default(),
            active: None,
            key: 0,
            on_stack_change: None,
        }
    }

    /// Creates a configurator that can be used to configure the `Group`.
    ///
    /// The configurator can set the `capacity` and what should happen when the active stack
    /// changes.
    ///
    /// # Examples
    /// ```
    /// # use redo::{self, Command, Group, Stack};
    /// # struct Pop;
    /// # impl Command<u8> for Pop {
    /// #   type Err = ();
    /// #   fn redo(&mut self, _: &mut u8) -> redo::Result<()> { Ok(()) }
    /// #   fn undo(&mut self, _: &mut u8) -> redo::Result<()> { Ok(()) }
    /// # }
    /// let mut group = Group::config()
    ///     .capacity(10)
    ///     .on_stack_change(|is_clean| {
    ///         match is_clean {
    ///             Some(true) => { /* The new active stack is clean */ },
    ///             Some(false) => { /* The new active stack is dirty */ },
    ///             None => { /* No active stack */ },
    ///         }
    ///     })
    ///     .finish();
    /// # group.add(Stack::<u8, Pop>::new(0_u8));
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

    /// Creates a new `Group` with the specified [capacity].
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn with_capacity(capacity: usize) -> Group<'a, T, C> {
        Group {
            group: FnvHashMap::with_capacity_and_hasher(capacity, Default::default()),
            ..Group::new()
        }
    }

    /// Returns the capacity of the `Group`.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.group.capacity()
    }

    /// Reserves capacity for at least `additional` more stacks to be inserted in the given group.
    /// The group may reserve more space to avoid frequent reallocations.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.group.reserve(additional);
    }

    /// Shrinks the capacity of the `Group` as much as possible.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.group.shrink_to_fit();
    }

    /// Returns the number of stacks in the group.
    #[inline]
    pub fn len(&self) -> usize {
        self.group.len()
    }

    /// Adds an `Stack` to the group and returns an unique id for this stack.
    #[inline]
    pub fn add(&mut self, stack: Stack<'a, T, C>) -> Key {
        let key = Key(self.key);
        self.key += 1;
        self.group.insert(key, stack);
        key
    }

    /// Removes the `Stack` with the specified id and returns the stack.
    /// Returns `None` if the stack was not found.
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

    /// Sets the `Stack` with the specified id as the current active one.
    #[inline]
    pub fn set_active(&mut self, key: Key) {
        if let Some(is_clean) = self.group.get(&key).map(|stack| stack.is_clean()) {
            self.active = Some(key);
            if let Some(ref mut f) = self.on_stack_change {
                f(Some(is_clean));
            }
        }
    }

    /// Clears the current active `Stack`.
    #[inline]
    pub fn clear_active(&mut self) {
        self.active = None;
        if let Some(ref mut f) = self.on_stack_change {
            f(None);
        }
    }

    /// Calls [`is_clean`] on the active `Stack`, if there is one.
    /// Returns `None` if there is no active stack.
    ///
    /// [`is_clean`]: struct.Stack.html#method.is_clean
    #[inline]
    pub fn is_clean(&self) -> Option<bool> {
        self.active.map(|i| self.group[&i].is_clean())
    }

    /// Calls [`is_dirty`] on the active `Stack`, if there is one.
    /// Returns `None` if there is no active stack.
    ///
    /// [`is_dirty`]: struct.Stack.html#method.is_dirty
    #[inline]
    pub fn is_dirty(&self) -> Option<bool> {
        self.is_clean().map(|t| !t)
    }

    /// Returns an iterator over the `(&Key, &Stack)` pairs in the group.
    #[inline]
    pub fn stacks(&'a self) -> Stacks<'a, T, C> {
        Stacks(self.group.iter())
    }

    /// Returns an iterator over the `(&Key, &mut Stack)` pairs in the group.
    #[inline]
    pub fn stacks_mut(&'a mut self) -> StacksMut<'a, T, C> {
        StacksMut(self.group.iter_mut())
    }

    /// Calls [`push`] on the active `Stack`, if there is one.
    ///
    /// Returns `Some(Ok)` if everything went fine, `Some(Err)` if something went wrong, and `None`
    /// if there is no active stack.
    ///
    /// [`push`]: struct.Stack.html#method.push
    #[inline]
    pub fn push(&mut self, cmd: C) -> Option<Result<C::Err>> {
        self.active
            .and_then(|active| self.group.get_mut(&active))
            .map(|stack| stack.push(cmd))
    }

    /// Calls [`redo`] on the active `Stack`, if there is one.
    ///
    /// Returns `Some(Ok)` if everything went fine, `Some(Err)` if something went wrong, and `None`
    /// if there is no active stack.
    ///
    /// [`redo`]: struct.Stack.html#method.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result<C::Err>> {
        self.active
            .and_then(|active| self.group.get_mut(&active))
            .map(|stack| stack.redo())
    }

    /// Calls [`undo`] on the active `Stack`, if there is one.
    ///
    /// Returns `Some(Ok)` if everything went fine, `Some(Err)` if something went wrong, and `None`
    /// if there is no active stack.
    ///
    /// [`undo`]: struct.Stack.html#method.undo
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
    /// Adds a default `Stack` to the group and returns an unique id for this stack.
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

/// Configurator for `Group`.
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
    /// By default the `Group` does nothing when the active stack changes.
    #[inline]
    pub fn on_stack_change<F>(mut self, f: F) -> Config<'a, T, C>
    where
        F: FnMut(Option<bool>) + 'a,
    {
        self.on_stack_change = Some(Box::new(f));
        self
    }

    /// Returns the `Group`.
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
