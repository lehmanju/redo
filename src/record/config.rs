use std::collections::VecDeque;
use std::marker::PhantomData;
use std::fmt::{self, Debug, Formatter};
use Command;
use super::Record;

/// Configurator for `Record`.
pub struct Config<T, C: Command<T>> {
    commands: PhantomData<C>,
    receiver: T,
    capacity: usize,
    limit: usize,
    state_change: Option<Box<FnMut(bool)>>,
}

impl<T, C: Command<T>> Config<T, C> {
    /// Creates a `Config`.
    #[inline]
    pub fn new(receiver: T) -> Config<T, C> {
        Config {
            commands: PhantomData,
            receiver,
            capacity: 0,
            limit: 0,
            state_change: None,
        }
    }

    /// Sets the `capacity` for the `Record`.
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> Config<T, C> {
        self.capacity = capacity;
        self
    }

    /// Sets the `limit` for the `Record`.
    #[inline]
    pub fn limit(mut self, limit: usize) -> Config<T, C> {
        self.limit = limit;
        self
    }

    /// Sets what should happen when the state changes.
    #[inline]
    pub fn state_change<F>(mut self, f: F) -> Config<T, C>
    where
        F: FnMut(bool) + 'static,
    {
        self.state_change = Some(Box::new(f));
        self
    }

    /// Creates the `Record`.
    #[inline]
    pub fn finish(self) -> Record<T, C> {
        Record {
            commands: VecDeque::with_capacity(self.capacity),
            receiver: self.receiver,
            idx: 0,
            limit: if self.limit == 0 {
                None
            } else {
                Some(self.limit)
            },
            state_change: self.state_change,
        }
    }
}

impl<T: Debug, C: Command<T> + Debug> Debug for Config<T, C> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Config")
            .field("receiver", &self.receiver)
            .field("capacity", &self.capacity)
            .field("limit", &self.limit)
            .finish()
    }
}
