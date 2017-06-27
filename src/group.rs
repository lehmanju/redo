use std::collections::hash_map;
use fnv::FnvHashMap;
use {Command, Key, Stack};

/// A group of `Stack`s.
///
/// A `Group` can be used when working with multiple stacks and only one of them should
/// be active at the same time, for example a text editor with multiple documents opened.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Group<T, C: Command<T>> {
    // The stacks in the group.
    stacks: FnvHashMap<Key, Stack<T, C>>,
    // The active stack.
    active: Option<Key>,
    // Counter for generating new keys.
    key: u32,
}

impl<T, C: Command<T>> Group<T, C> {
    /// Creates a new `Group`.
    #[inline]
    pub fn new() -> Group<T, C> {
        Group {
            stacks: FnvHashMap::default(),
            active: None,
            key: 0,
        }
    }

    /// Creates a new `Group` with the specified [capacity].
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn with_capacity(capacity: usize) -> Group<T, C> {
        Group {
            stacks: FnvHashMap::with_capacity_and_hasher(capacity, Default::default()),
            ..Group::new()
        }
    }

    /// Returns the capacity of the `Group`.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.stacks.capacity()
    }

    /// Reserves capacity for at least `additional` more stacks to be inserted in the given group.
    /// The group may reserve more space to avoid frequent reallocations.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.stacks.reserve(additional);
    }

    /// Shrinks the capacity of the `Group` as much as possible.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.stacks.shrink_to_fit();
    }

    /// Returns the number of stacks in the group.
    #[inline]
    pub fn len(&self) -> usize {
        self.stacks.len()
    }

    /// Adds a `Stack` to the group and returns an unique id for this stack.
    #[inline]
    pub fn add(&mut self, stack: Stack<T, C>) -> Key {
        let key = Key(self.key);
        self.key += 1;
        self.stacks.insert(key, stack);
        key
    }

    /// Removes the `Stack` with the specified id from the `Group` and returns it.
    /// Returns `None` if the stack was not found.
    #[inline]
    pub fn remove(&mut self, key: Key) -> Option<Stack<T, C>> {
        // Check if it was the active stack that was removed.
        if let Some(active) = self.active {
            if active == key {
                self.set_active(None);
            }
        }
        self.stacks.remove(&key)
    }

    /// Sets the `Stack` with the specified id as the current active one.
    #[inline]
    pub fn set_active<K: Into<Option<Key>>>(&mut self, key: K) {
        self.active = key.into();
    }

    /// Returns an iterator over the `(&Key, &Stack)` pairs in the group.
    #[inline]
    pub fn stacks(&self) -> Stacks<T, C> {
        Stacks(self.stacks.iter())
    }

    /// Returns an iterator over the `(&Key, &mut Stack)` pairs in the group.
    #[inline]
    pub fn stacks_mut(&mut self) -> StacksMut<T, C> {
        StacksMut(self.stacks.iter_mut())
    }

    /// Calls [`push`] on the active `Stack`, if there is one.
    ///
    /// Returns `Some(Ok)` if everything went fine, `Some(Err)` if something went wrong, and `None`
    /// if there is no active stack.
    ///
    /// [`push`]: struct.Stack.html#method.push
    #[inline]
    pub fn push(&mut self, cmd: C) -> Option<Result<(), (C, C::Err)>> {
        self.active
            .and_then(|active| self.stacks.get_mut(&active))
            .map(|stack| stack.push(cmd))
    }

    /// Calls [`pop`] on the active `Stack`, if there is one.
    ///
    /// Returns `Some(Ok)` if everything went fine, `Some(Err)` if something went wrong, and `None`
    /// if there is no active stack or if the stack is empty.
    ///
    /// [`undo`]: struct.Stack.html#method.pop
    #[inline]
    pub fn pop(&mut self) -> Option<Result<C, (C, C::Err)>> {
        self.active
            .and_then(|active| self.stacks.get_mut(&active))
            .and_then(|stack| stack.pop())
    }
}

impl<T: Default, C: Command<T> + Default> Group<T, C> {
    /// Adds a default `Stack` to the group and returns an unique id for this stack.
    #[inline]
    pub fn add_default(&mut self) -> Key {
        self.add(Default::default())
    }
}

#[derive(Debug)]
pub struct IntoStacks<T, C: Command<T>>(hash_map::IntoIter<Key, Stack<T, C>>);

impl<T, C: Command<T>> Iterator for IntoStacks<T, C> {
    type Item = (Key, Stack<T, C>);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<T, C: Command<T>> IntoIterator for Group<T, C> {
    type Item = (Key, Stack<T, C>);
    type IntoIter = IntoStacks<T, C>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoStacks(self.stacks.into_iter())
    }
}

#[derive(Debug)]
pub struct Stacks<'a, T: 'a, C: Command<T> + 'a>(hash_map::Iter<'a, Key, Stack<T, C>>);

impl<'a, T, C: Command<T>> Iterator for Stacks<'a, T, C> {
    type Item = (&'a Key, &'a Stack<T, C>);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a, T, C: Command<T>> IntoIterator for &'a Group<T, C> {
    type Item = (&'a Key, &'a Stack<T, C>);
    type IntoIter = Stacks<'a, T, C>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        Stacks(self.stacks.iter())
    }
}

#[derive(Debug)]
pub struct StacksMut<'a, T: 'a, C: Command<T> + 'a>(hash_map::IterMut<'a, Key, Stack<T, C>>);

impl<'a, T, C: Command<T>> Iterator for StacksMut<'a, T, C> {
    type Item = (&'a Key, &'a mut Stack<T, C>);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a, T, C: Command<T>> IntoIterator for &'a mut Group<T, C> {
    type Item = (&'a Key, &'a mut Stack<T, C>);
    type IntoIter = StacksMut<'a, T, C>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        StacksMut(self.stacks.iter_mut())
    }
}
