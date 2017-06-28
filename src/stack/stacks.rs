use std::collections::hash_map;
use fnv::FnvHashMap;
use super::Stack;
use {Command, Key};

/// A collection of `Stack`s.
///
/// `Stacks` can be used when working with multiple stacks and only one of them should
/// be active at the same time, for example a text editor with multiple documents opened.
///
/// # Examples
/// ```
/// # #![allow(unused_variables)]
/// use redo::{Command, Stacks};
///
/// #[derive(Debug)]
/// struct Push(char);
///
/// impl Command<String> for Push {
///     type Err = &'static str;
///
///     fn redo(&mut self, s: &mut String) -> Result<(), &'static str> {
///         s.push(self.0);
///         Ok(())
///     }
///
///     fn undo(&mut self, s: &mut String) -> Result<(), &'static str> {
///         self.0 = s.pop().ok_or("`String` is unexpectedly empty")?;
///         Ok(())
///     }
/// }
///
/// fn foo() -> Result<(), (Push, &'static str)> {
///     let mut stacks = Stacks::default();
///     stacks.add_default();
///
///     stacks.push(Push('a')).unwrap()?;
///     stacks.push(Push('b')).unwrap()?;
///     stacks.push(Push('c')).unwrap()?;
///
///     assert_eq!(stacks.as_receiver().unwrap(), "abc");
///
///     let c = stacks.pop().unwrap()?;
///     let b = stacks.pop().unwrap()?;
///     let a = stacks.pop().unwrap()?;
///
///     assert_eq!(stacks.as_receiver().unwrap(), "");
///     Ok(())
/// }
/// # foo().unwrap();
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stacks<T, C: Command<T>> {
    stacks: FnvHashMap<Key, Stack<T, C>>,
    active: Option<Key>,
    key: u32,
}

impl<T, C: Command<T>> Stacks<T, C> {
    /// Creates a new `Group`.
    #[inline]
    pub fn new() -> Stacks<T, C> {
        Stacks {
            stacks: FnvHashMap::default(),
            active: None,
            key: 0,
        }
    }

    /// Creates a new `Group` with the specified [capacity].
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn with_capacity(capacity: usize) -> Stacks<T, C> {
        Stacks {
            stacks: FnvHashMap::with_capacity_and_hasher(capacity, Default::default()),
            ..Stacks::new()
        }
    }

    /// Returns the capacity of the `Group`.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.stacks.capacity()
    }

    /// Returns the number of `Stack`s in the `Group`.
    #[inline]
    pub fn len(&self) -> usize {
        self.stacks.len()
    }

    /// Returns `true` if the `Group` is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.stacks.is_empty()
    }

    /// Returns a reference to the `receiver` of the active `Stack`.
    #[inline]
    pub fn as_receiver(&self) -> Option<&T> {
        self.active
            .and_then(|active| self.stacks.get(&active))
            .map(|stack| stack.as_receiver())
    }

    /// Adds a `Stack` to the group and returns an unique id for this stack.
    ///
    /// The stack is set as the active one if there is *not* already an active stack.
    #[inline]
    pub fn add(&mut self, stack: Stack<T, C>) -> Key {
        let key = Key(self.key);
        self.key += 1;
        self.stacks.insert(key, stack);
        if let None = self.active {
            self.set_active(key);
        }
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
    pub fn iter(&self) -> Iter<T, C> {
        Iter(self.stacks.iter())
    }

    /// Returns an iterator over the `(&Key, &mut Stack)` pairs in the group.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<T, C> {
        IterMut(self.stacks.iter_mut())
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
    /// [`pop`]: struct.Stack.html#method.pop
    #[inline]
    pub fn pop(&mut self) -> Option<Result<C, (C, C::Err)>> {
        self.active
            .and_then(|active| self.stacks.get_mut(&active))
            .and_then(|stack| stack.pop())
    }
}

impl<T, C: Command<T>> Default for Stacks<T, C> {
    #[inline]
    fn default() -> Stacks<T, C> {
        Stacks::new()
    }
}

impl<T: Default, C: Command<T>> Stacks<T, C> {
    /// Adds a default `Stack` to the group and returns an unique id for this stack.
    #[inline]
    pub fn add_default(&mut self) -> Key {
        self.add(Default::default())
    }
}

#[derive(Debug)]
pub struct Iter<'a, T: 'a, C: Command<T> + 'a>(hash_map::Iter<'a, Key, Stack<T, C>>);

impl<'a, T, C: Command<T>> Iterator for Iter<'a, T, C> {
    type Item = (&'a Key, &'a Stack<T, C>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a, T, C: Command<T>> IntoIterator for &'a Stacks<T, C> {
    type Item = (&'a Key, &'a Stack<T, C>);
    type IntoIter = Iter<'a, T, C>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        Iter(self.stacks.iter())
    }
}

#[derive(Debug)]
pub struct IterMut<'a, T: 'a, C: Command<T> + 'a>(hash_map::IterMut<'a, Key, Stack<T, C>>);

impl<'a, T, C: Command<T>> Iterator for IterMut<'a, T, C> {
    type Item = (&'a Key, &'a mut Stack<T, C>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a, T, C: Command<T>> IntoIterator for &'a mut Stacks<T, C> {
    type Item = (&'a Key, &'a mut Stack<T, C>);
    type IntoIter = IterMut<'a, T, C>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IterMut(self.stacks.iter_mut())
    }
}

#[derive(Debug)]
pub struct IntoIter<T, C: Command<T>>(hash_map::IntoIter<Key, Stack<T, C>>);

impl<T, C: Command<T>> Iterator for IntoIter<T, C> {
    type Item = (Key, Stack<T, C>);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<T, C: Command<T>> IntoIterator for Stacks<T, C> {
    type Item = (Key, Stack<T, C>);
    type IntoIter = IntoIter<T, C>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.stacks.into_iter())
    }
}
