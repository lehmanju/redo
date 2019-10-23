![redo](https://raw.githubusercontent.com/evenorog/redo/master/redo.svg?sanitize=true)

[![Travis](https://travis-ci.com/evenorog/redo.svg?branch=master)](https://travis-ci.com/evenorog/redo)
[![Crates.io](https://img.shields.io/crates/v/redo.svg)](https://crates.io/crates/redo)
[![Docs](https://docs.rs/redo/badge.svg)](https://docs.rs/redo)

Provides undo-redo functionality with static dispatch and manual command merging.

It is an implementation of the command pattern, where all modifications are done
by creating objects of commands that applies the modifications. All commands knows
how to undo the changes it applies, and by using the provided data structures
it is easy to apply, undo, and redo changes made to a receiver.
Both linear and non-linear undo-redo functionality is provided through
the [Record] and [History] data structures.
This library provides more or less the same functionality as the [undo] library 
but is more focused on performance instead of ease of use.

# Contents

* [Command] provides the base functionality for all commands.
* [Record] provides linear undo-redo functionality.
* [History] provides non-linear undo-redo functionality that allows you to jump between different branches.
* [Queue] wraps a [Record] or [History] and extends them with queue functionality.
* [Checkpoint] wraps a [Record] or [History] and extends them with checkpoint functionality.
* Configurable display formatting is provided when the `display` feature is enabled.
* Time stamps and time travel is provided when the `chrono` feature is enabled.
* Serialization and deserialization is provided when the `serde` feature is enabled.

# Concepts

* Commands can be merged into a single command by implementing the [merge] method on the command.
  This allows smaller commands to be used to build more complex operations, or smaller incremental changes to be
  merged into larger changes that can be undone and redone in a single step.
* The receiver can be marked as being saved to disk and the data-structures can track the saved state and tell the user
  when it changes.
* The amount of changes being tracked can be configured by the user so only the `n` most recent changes are stored.

# Examples

Add this to `Cargo.toml`:

```toml
[dependencies]
redo = "0.37"
```

And this to `main.rs`:

```rust
use redo::{Command, Record};

struct Add(char);

impl Command<String> for Add {
    type Error = &'static str;

    fn apply(&mut self, s: &mut String) -> Result<(), Self::Error> {
        s.push(self.0);
        Ok(())
    }

    fn undo(&mut self, s: &mut String) -> Result<(), Self::Error> {
        self.0 = s.pop().ok_or("`s` is empty")?;
        Ok(())
    }
}

fn main() -> Result<(), &'static str> {
    let mut record = Record::default();
    record.apply(Add('a'))?;
    record.apply(Add('b'))?;
    record.apply(Add('c'))?;
    assert_eq!(record.as_receiver(), "abc");
    record.undo().unwrap()?;
    record.undo().unwrap()?;
    record.undo().unwrap()?;
    assert_eq!(record.as_receiver(), "");
    record.redo().unwrap()?;
    record.redo().unwrap()?;
    record.redo().unwrap()?;
    assert_eq!(record.as_receiver(), "abc");
    Ok(())
}
```

### License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.

[Command]: https://docs.rs/redo/latest/redo/trait.Command.html
[Record]: https://docs.rs/redo/latest/redo/struct.Record.html
[Timeline]: https://docs.rs/redo/latest/redo/struct.Timeline.html
[History]: https://docs.rs/redo/latest/redo/struct.History.html
[Queue]: https://docs.rs/undo/latest/undo/struct.Queue.html
[Checkpoint]: https://docs.rs/undo/latest/undo/struct.Checkpoint.html
[merge]: https://docs.rs/redo/latest/redo/trait.Command.html#method.merge
[undo]: https://github.com/evenorog/undo
