# Redo
[![Travis](https://travis-ci.org/evenorog/redo.svg?branch=master)](https://travis-ci.org/evenorog/redo)
[![Appveyor](https://ci.appveyor.com/api/projects/status/af1g96b3xsoypbq0/branch/master?svg=true)](https://ci.appveyor.com/project/evenorog/redo/branch/master)
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
struct Add(char);

impl Command<String> for Add {
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

fn foo() -> Result<(), (Add, &'static str)> {
    let mut stack = Stack::default();

    stack.push(Add('a'))?;
    stack.push(Add('b'))?;
    stack.push(Add('c'))?;

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
