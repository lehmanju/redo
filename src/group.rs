use std::collections::HashMap;
use record::Commands;
use {Command, Error, Stack, Record};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Group<T> {
    map: HashMap<u32, T>,
    idx: Option<u32>,
}

impl<T> Group<T> {
    #[inline]
    pub fn new() -> Group<T> {
        Group {
            map: HashMap::new(),
            idx: None,
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
}

impl<R, C: Command<R>> Group<Stack<R, C>> {
    #[inline]
    pub fn push<T: Into<C>>(&mut self, cmd: T) -> Option<Result<(), Error<R, C>>> {
        self.idx
            .and_then(|idx| self.map.get_mut(&idx))
            .map(move |stack| stack.push(cmd))
    }

    #[inline]
    pub fn pop(&mut self) -> Option<Result<C, Error<R, C>>> {
        self.idx
            .and_then(|idx| self.map.get_mut(&idx))
            .and_then(|stack| stack.pop())
    }
}

impl<'a, R, C: Command<R>> Group<Record<'a, R, C>> {
    #[inline]
    pub fn push<T: Into<C>>(&mut self, cmd: T) -> Option<Result<Commands<C>, Error<R, C>>> {
        self.idx
            .and_then(|idx| self.map.get_mut(&idx))
            .map(move |record| record.push(cmd))
    }

    #[inline]
    pub fn redo(&mut self) -> Option<Result<(), C::Err>> {
        self.idx
            .and_then(|idx| self.map.get_mut(&idx))
            .and_then(|record| record.redo())
    }

    #[inline]
    pub fn undo(&mut self) -> Option<Result<(), C::Err>> {
        self.idx
            .and_then(|idx| self.map.get_mut(&idx))
            .and_then(|record| record.undo())
    }
}

impl<T> Default for Group<T> {
    #[inline]
    fn default() -> Group<T> {
        Group::new()
    }
}
