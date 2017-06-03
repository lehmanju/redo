# Redo
An undo/redo library with static dispatch, state handling and manual command merging.

[![Build Status](https://travis-ci.org/evenorog/redo.svg?branch=master)](https://travis-ci.org/evenorog/redo)
[![Crates.io](https://img.shields.io/crates/v/redo.svg)](https://crates.io/crates/redo)
[![Docs](https://docs.rs/redo/badge.svg)](https://docs.rs/redo)

## About
It uses the [Command Pattern] where the user implements the `RedoCmd` trait for a command.

The `RedoStack` has two states, clean and dirty. The stack is clean when no more commands can
be redone, otherwise it is dirty. The stack will notice when it's state changes to either dirty
or clean, and call the user defined methods set in [`on_clean`] and [`on_dirty`].
This is useful if you want to trigger some event when the state changes, eg. enabling and
disabling buttons in an ui.

It also supports merging of commands by implementing the [`merge`] method for a command.

## Redo vs Undo
|                 | Redo         | Undo            |
|-----------------|--------------|-----------------|
| Dispatch        | Static       | Dynamic         |
| State Handling  | Yes          | Yes             |
| Command Merging | Manual       | Auto            |

Both supports command merging but `undo` will automatically merge commands with the same id
while in `redo` you need to implement the merge method yourself.

## Examples
```toml
[dependencies]
redo = "0.4.0"
```

```rust
use redo::{self, RedoCmd, RedoStack};

#[derive(Clone, Copy)]
struct PopCmd {
    vec: *mut Vec<i32>,
    e: Option<i32>,
}

impl RedoCmd for PopCmd {
    type Err = &'static str;

    fn redo(&mut self) -> redo::Result<&'static str> {
        self.e = unsafe {
            let ref mut vec = *self.vec;
            vec.pop()
        };
        Ok(())
    }

    fn undo(&mut self) -> redo::Result<&'static str> {
        unsafe {
            let ref mut vec = *self.vec;
            let e = self.e.ok_or("`e` is invalid")?;
            vec.push(e);
        }
        Ok(())
    }
}

fn foo() -> redo::Result<&'static str> {
    let mut vec = vec![1, 2, 3];
    let mut stack = RedoStack::new();
    let cmd = PopCmd { vec: &mut vec, e: None };

    stack.push(cmd)?;
    stack.push(cmd)?;
    stack.push(cmd)?;

    assert!(vec.is_empty());

    stack.undo()?;
    stack.undo()?;
    stack.undo()?;

    assert_eq!(vec.len(), 3);
    Ok(())
}
```

[Command Pattern]: https://en.wikipedia.org/wiki/Command_pattern
[`on_clean`]: struct.RedoStack.html#method.on_clean
[`on_dirty`]: struct.RedoStack.html#method.on_dirty
[static dispatch]: https://doc.rust-lang.org/stable/book/trait-objects.html#static-dispatch
[dynamic dispatch]: https://doc.rust-lang.org/stable/book/trait-objects.html#dynamic-dispatch
[`undo`]: https://crates.io/crates/undo
[`merge`]: trait.RedoCmd.html#method.merge
