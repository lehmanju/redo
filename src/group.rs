use std::collections::HashMap;
use std::hash::Hash;
use record::Commands;
use {Command, Error, Stack, Record};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Group<K: Hash + Eq, V> {
    map: HashMap<K, V>,
    active: Option<K>,
}

impl<K: Hash + Eq, V> Group<K, V> {
    #[inline]
    pub fn new() -> Group<K, V> {
        Group {
            map: HashMap::new(),
            active: None,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    #[inline]
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.map.insert(k, v)
    }

    #[inline]
    pub fn remove(&mut self, k: &K) -> Option<V> {
        if self.active.as_ref().map_or(false, |active| active == k) {
            self.set(None);
        }
        self.map.remove(k)
    }

    #[inline]
    pub fn get(&self) -> Option<&V> {
        self.active
            .as_ref()
            .and_then(|active| self.map.get(&active))
    }

    #[inline]
    pub fn set<T: Into<Option<K>>>(&mut self, k: T) -> bool {
        let k = k.into();
        match k {
            Some(ref key) if !self.map.contains_key(key) => false,
            _ => {
                self.active = k;
                true
            }
        }
    }
}

impl<K: Hash + Eq, R, C: Command<R>> Group<K, Stack<R, C>> {
    #[inline]
    pub fn push<T: Into<C>>(&mut self, cmd: T) -> Option<Result<(), Error<R, C>>> {
        let map = &mut self.map;
        self.active
            .as_ref()
            .and_then(|active| map.get_mut(active))
            .map(move |stack| stack.push(cmd))
    }

    #[inline]
    pub fn pop(&mut self) -> Option<Result<C, Error<R, C>>> {
        let map = &mut self.map;
        self.active
            .as_ref()
            .and_then(|active| map.get_mut(active))
            .and_then(|stack| stack.pop())
    }
}

impl<'a, K: Hash + Eq, R, C: Command<R>> Group<K, Record<'a, R, C>> {
    #[inline]
    pub fn push<T: Into<C>>(&mut self, cmd: T) -> Option<Result<Commands<C>, Error<R, C>>> {
        let map = &mut self.map;
        self.active
            .as_ref()
            .and_then(|active| map.get_mut(active))
            .map(move |record| record.push(cmd))
    }

    #[inline]
    pub fn redo(&mut self) -> Option<Result<(), C::Err>> {
        let map = &mut self.map;
        self.active
            .as_ref()
            .and_then(|active| map.get_mut(active))
            .and_then(|record| record.redo())
    }

    #[inline]
    pub fn undo(&mut self) -> Option<Result<(), C::Err>> {
        let map = &mut self.map;
        self.active
            .as_ref()
            .and_then(|active| map.get_mut(active))
            .and_then(|record| record.undo())
    }
}

impl<K: Hash + Eq, V> Default for Group<K, V> {
    #[inline]
    fn default() -> Group<K, V> {
        Group::new()
    }
}
