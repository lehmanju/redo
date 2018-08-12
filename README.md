# redo
[![Travis](https://travis-ci.org/evenorog/redo.svg?branch=master)](https://travis-ci.org/evenorog/redo)
[![Appveyor](https://ci.appveyor.com/api/projects/status/af1g96b3xsoypbq0/branch/master?svg=true)](https://ci.appveyor.com/project/evenorog/redo/branch/master)
[![Crates.io](https://img.shields.io/crates/v/redo.svg)](https://crates.io/crates/redo)
[![Docs](https://docs.rs/redo/badge.svg)](https://docs.rs/redo)

An undo-redo library with static dispatch and manual command merging.

It uses the [command pattern] where the user modifies the receiver by
applying commands on it. Since each command knows how to undo and redo
the changes it applies to the receiver, the state of the receiver can
be rolled forwards or backwards by calling undo or redo in the correct order.

The [Record] and [History] provides functionality to store and keep track
of the applied commands, and makes it easy to undo and redo changes.
The Record provides a stack based undo-redo functionality, while the
History provides a tree based undo-redo functionality where you can
jump between different branches.

Commands can be merged using the [`merge!`] macro or the [`merge`] method.
When two commands are merged, undoing and redoing them are done in a single step.

## Examples

Add this to `Cargo.toml`:

```toml
[dependencies]
redo = "0.24"
```

And this to `main.rs`:

```rust
#[derive(Debug)]
struct Add(char);

impl Command<String> for Add {
    type Error = Box<dyn error::Error>;

    fn apply(&mut self, s: &mut String) -> Result<(), Self::Error> {
        s.push(self.0);
        Ok(())
    }

    fn undo(&mut self, s: &mut String) -> Result<(), Self::Error> {
        self.0 = s.pop().ok_or("`s` is empty")?;
        Ok(())
    }
}

fn main() -> Result<(), Error<String, Add>> {
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

[command pattern]: https://en.wikipedia.org/wiki/Command_pattern
[Record]: https://docs.rs/redo/latest/redo/struct.Record.html
[History]: https://docs.rs/redo/latest/redo/struct.History.html
[`merge!`]: https://docs.rs/redo/latest/redo/macro.merge.html
[`merge`]: https://docs.rs/redo/latest/redo/trait.Command.html#method.merge
