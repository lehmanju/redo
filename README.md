# Redo
[![Build Status](https://travis-ci.org/evenorog/redo.svg?branch=master)](https://travis-ci.org/evenorog/redo)
[![Crates.io](https://img.shields.io/crates/v/redo.svg)](https://crates.io/crates/redo)
[![Docs](https://docs.rs/redo/badge.svg)](https://docs.rs/redo)

An undo/redo library with static dispatch and manual command merging.
It uses the [Command Pattern] where the user modifies a receiver by
applying `Command`s on it.

The library has currently two data structures that can be used to modify the receiver:

* A simple `Stack` that pushes and pops commands to modify the receiver.
* A `Record` that can roll the state of the receiver forwards and backwards.

## Examples
```rust
use redo::{Command, Stack};

#[derive(Debug)]
struct Push(char);

impl Command<String> for Push {
    type Err = &'static str;

    fn redo(&mut self, s: &mut String) -> Result<(), &'static str> {
        s.push(self.0);
        Ok(())
    }

    fn undo(&mut self, s: &mut String) -> Result<(), &'static str> {
        self.0 = s.pop().ok_or("`String` is unexpectedly empty")?;
        Ok(())
    }
}

fn foo() -> Result<(), (Push, &'static str)> {
    let mut stack = Stack::default();

    stack.push(Push('a'))?;
    stack.push(Push('b'))?;
    stack.push(Push('c'))?;

    assert_eq!(stack.as_receiver(), "abc");

    let c = stack.pop().unwrap()?;
    let b = stack.pop().unwrap()?;
    let a = stack.pop().unwrap()?;

    assert_eq!(stack.into_receiver(), "");

    stack.push(a)?;
    stack.push(b)?;
    stack.push(c)?;

    assert_eq!(stack.into_receiver(), "abc");

    Ok(())
}
```

[Command Pattern]: https://en.wikipedia.org/wiki/Command_pattern
