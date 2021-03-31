# redo

**High-level undo-redo functionality.**

[![Travis](https://travis-ci.com/evenorog/redo.svg?branch=master)](https://travis-ci.com/evenorog/redo)
[![Crates.io](https://img.shields.io/crates/v/redo.svg)](https://crates.io/crates/redo)
[![Docs](https://docs.rs/redo/badge.svg)](https://docs.rs/redo)

It is an implementation of the command pattern, where all modifications are done
by creating objects of commands that applies the modifications. All commands knows
how to undo the changes it applies, and by using the provided data structures
it is easy to apply, undo, and redo changes made to a target.

### License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
