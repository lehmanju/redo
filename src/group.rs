use std::collections::hash_map::{HashMap, RandomState};
use std::fmt::{self, Debug, Display, Formatter};
use std::hash::{BuildHasher, Hash};
use std::marker::PhantomData;
use {Command, Error, Record};

/// A group of records.
pub struct Group<'a, K: Hash + Eq, R, C: Command<R>, S = RandomState> where S: BuildHasher {
    records: HashMap<K, Record<'a, R, C>, S>,
    active: Option<K>,
    signals: Option<Box<FnMut(Option<&K>) + Send + Sync + 'a>>,
}

impl<'a, K: Hash + Eq, R, C: Command<R>> Group<'a, K, R, C, RandomState> {
    /// Returns a new group.
    #[inline]
    pub fn new() -> Group<'a, K, R, C, RandomState> {
        Default::default()
    }
}

impl<'a, K: Hash + Eq, R, C: Command<R>, S: BuildHasher> Group<'a, K, R, C, S> {
    /// Returns a builder for a group.
    #[inline]
    pub fn builder() -> GroupBuilder<'a, K, R, C, S> {
        GroupBuilder {
            records: PhantomData,
            capacity: 0,
            signals: None,
        }
    }

    /// Returns the capacity of the group.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.records.capacity()
    }

    /// Returns the number of items in the group.
    #[inline]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns `true` if the group is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Inserts an item into the group.
    #[inline]
    pub fn insert(&mut self, k: K, record: Record<'a, R, C>) -> Option<Record<'a, R, C>> {
        self.records.insert(k, record)
    }

    /// Removes an item from the group.
    #[inline]
    pub fn remove(&mut self, k: &K) -> Option<Record<'a, R, C>> {
        self.records.remove(k)
    }

    /// Gets a reference to the current active item in the group.
    #[inline]
    pub fn get(&self) -> Option<&Record<'a, R, C>> {
        self.active.as_ref().and_then(|active| self.records.get(active))
    }

    /// Gets a mutable reference to the current active item in the group.
    #[inline]
    pub fn get_mut(&mut self) -> Option<&mut Record<'a, R, C>> {
        let map = &mut self.records;
        self.active.as_mut().and_then(move |active| map.get_mut(active))
    }

    /// Sets the current active item in the group.
    ///
    /// Returns `None` if the item was successfully set, otherwise `k` is returned.
    #[inline]
    pub fn set(&mut self, k: K) -> Option<K> {
        if self.records.contains_key(&k) {
            if self.active.as_ref().map_or(false, |a| a != &k) {
                if let Some(ref mut f) = self.signals {
                    f(Some(&k))
                }
            }
            self.active = Some(k);
            None
        } else {
            Some(k)
        }
    }

    /// Unsets the current active item in the group.
    #[inline]
    pub fn unset(&mut self) {
        if self.active.is_some() {
            self.active = None;
            if let Some(ref mut f) = self.signals {
                f(None);
            }
        }
    }

    /// Returns an iterator over the records.
    #[inline]
    pub fn records(&self) -> impl Iterator<Item = &Record<'a, R, C>> {
        self.records.values()
    }
}

impl<'a, K: Hash + Eq, R, C: Command<R>, S: BuildHasher> Group<'a, K, R, C, S> {
    /// Calls the [`set_saved`] method on the active record.
    ///
    /// [`set_saved`]: record/struct.Record.html#method.set_saved
    #[inline]
    pub fn set_saved(&mut self) -> Option<()> {
        self.get_mut().map(|stack| stack.set_saved())
    }

    /// Calls the [`set_unsaved`] method on the active record.
    ///
    /// [`set_unsaved`]: record/struct.Record.html#method.set_unsaved
    #[inline]
    pub fn set_unsaved(&mut self) -> Option<()> {
        self.get_mut().map(|stack| stack.set_unsaved())
    }

    /// Calls the [`is_saved`] method on the active record.
    ///
    /// [`is_saved`]: record/struct.Record.html#method.is_saved
    #[inline]
    pub fn is_saved(&self) -> Option<bool> {
        self.get().map(|stack| stack.is_saved())
    }

    /// Calls the [`apply`] method on the active record.
    ///
    /// [`apply`]: record/struct.Record.html#method.apply
    #[inline]
    pub fn apply(&mut self, cmd: C) -> Option<Result<impl Iterator<Item=C>, Error<R, C>>> {
        self.get_mut().map(move |record| record.apply(cmd))
    }

    /// Calls the [`undo`] method on the active record.
    ///
    /// [`undo`]: record/struct.Record.html#method.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result<(), C::Error>> {
        self.get_mut().and_then(|record| record.undo())
    }

    /// Calls the [`redo`] method on the active record.
    ///
    /// [`redo`]: record/struct.Record.html#method.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result<(), C::Error>> {
        self.get_mut().and_then(|record| record.redo())
    }
}

impl<'a, K: Hash + Eq, R, C: Command<R> + ToString, S: BuildHasher> Group<'a, K, R, C, S> {
    /// Calls the [`to_undo_string`] method on the active record.
    ///
    /// [`to_undo_string`]: record/struct.Record.html#method.to_undo_string
    #[inline]
    pub fn to_undo_string(&self) -> Option<String> {
        self.get().and_then(|record| record.to_undo_string())
    }

    /// Calls the [`to_redo_string`] method on the active record.
    ///
    /// [`to_redo_string`]: record/struct.Record.html#method.to_redo_string
    #[inline]
    pub fn to_redo_string(&self) -> Option<String> {
        self.get().and_then(|record| record.to_redo_string())
    }
}

impl<'a, K: Hash + Eq + Debug, R: Debug, C: Command<R> + Debug, S: BuildHasher> Debug for Group<'a, K, R, C, S> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Group")
            .field("map", &self.records)
            .field("active", &self.active)
            .finish()
    }
}

impl<'a, K: Hash + Eq + Display, R, C: Command<R> + Display, S: BuildHasher> Display for Group<'a, K, R, C, S> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        for (key, item) in &self.records {
            if self.active.as_ref().map_or(false, |a| a == key) {
                writeln!(f, "* {}:\n{}", key, item)?;
            } else {
                writeln!(f, "  {}:\n{}", key, item)?;
            }
        }
        Ok(())
    }
}

impl<'a, K: Hash + Eq, R, C: Command<R>> Default for Group<'a, K, R, C, RandomState> {
    #[inline]
    fn default() -> Group<'a, K, R, C, RandomState> {
        Group {
            records: HashMap::new(),
            active: None,
            signals: None,
        }
    }
}

/// Builder for a group.
pub struct GroupBuilder<'a, K: Hash + Eq, R, C: Command<R>, S: BuildHasher> {
    records: PhantomData<(K, R, C, S)>,
    capacity: usize,
    signals: Option<Box<FnMut(Option<&K>) + Send + Sync + 'a>>,
}

impl<'a, K: Hash + Eq, R, C: Command<R>> GroupBuilder<'a, K, R, C, RandomState> {
    /// Creates the group.
    #[inline]
    pub fn build(self) -> Group<'a, K, R, C, RandomState> {
        self.build_with_hasher(Default::default())
    }
}

impl<'a, K: Hash + Eq, R, C: Command<R>, S: BuildHasher> GroupBuilder<'a, K, R, C, S> {
    /// Sets the specified [capacity] for the group.
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> GroupBuilder<'a, K, R, C, S> {
        self.capacity = capacity;
        self
    }

    /// Decides what should happen when the active stack changes.
    #[inline]
    pub fn signals<F>(mut self, f: F) -> GroupBuilder<'a, K, R, C, S>
        where
            F: FnMut(Option<&K>) + Send + Sync + 'a
    {
        self.signals = Some(Box::new(f));
        self
    }

    /// Creates the group with the given hasher.
    #[inline]
    pub fn build_with_hasher(self, hasher: S) -> Group<'a, K, R, C, S> {
        Group {
            records: HashMap::with_capacity_and_hasher(self.capacity, hasher),
            active: None,
            signals: self.signals,
        }
    }
}

impl<'a, K: Hash + Eq, R, C: Command<R>, S: BuildHasher> Debug for GroupBuilder<'a, K, R, C, S> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("GroupBuilder")
            .field("map", &self.records)
            .field("capacity", &self.capacity)
            .finish()
    }
}
