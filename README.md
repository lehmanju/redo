# redo
[![Travis](https://travis-ci.org/evenorog/redo.svg?branch=master)](https://travis-ci.org/evenorog/redo)
[![Crates.io](https://img.shields.io/crates/v/redo.svg)](https://crates.io/crates/redo)
[![Docs](https://docs.rs/redo/badge.svg)](https://docs.rs/redo)

Provides undo-redo functionality with static dispatch and manual command merging.

* [Record] provides a stack based undo-redo functionality.
* [History] provides a tree based undo-redo functionality that allows you to jump between different branches.
* [Queue] wraps a [Record] or [History] and provides batch queue functionality.
* [Checkpoint] wraps a [Record] or [History] and provides checkpoint functionality.
* Commands can be merged using the [`merge`] method.
  When two commands are merged, undoing and redoing them are done in a single step.
* Configurable display formatting is provided through the [Display] structure.
* Time stamps and time travel is provided when the `chrono` feature is enabled.
* Serialization and deserialization is provided when the `serde` feature is enabled.

## Examples

Add this to `Cargo.toml`:

```toml
[dependencies]
redo = "0.28"
```

And this to `main.rs`:

```rust
extern crate redo;

use redo::{self, Command, Record};

#[derive(Debug)]
struct Add(char);

impl Command<String> for Add {
    type Error = Box<dyn Error>;

    fn apply(&mut self, s: &mut String) -> Result<(), Self::Error> {
        s.push(self.0);
        Ok(())
    }

    fn undo(&mut self, s: &mut String) -> Result<(), Self::Error> {
        self.0 = s.pop().ok_or("`s` is empty")?;
        Ok(())
    }
}

fn main() -> Result<(), redo::Error<String, Add>> {
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

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.

[Record]: https://docs.rs/redo/latest/redo/struct.Record.html
[History]: https://docs.rs/redo/latest/redo/struct.History.html
[Queue]: https://docs.rs/undo/latest/undo/struct.Queue.html
[Checkpoint]: https://docs.rs/undo/latest/undo/struct.Checkpoint.html
[Display]: https://docs.rs/undo/latest/undo/struct.Display.html
[`merge`]: https://docs.rs/redo/latest/redo/trait.Command.html#method.merge
