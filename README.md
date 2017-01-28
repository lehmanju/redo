# Redo
An undo/redo library.

Redo does not use [dynamic dispatch] which means it is <u>faster</u> than [undo] 
but less flexible.

[![Build Status](https://travis-ci.org/evenorog/redo.svg?branch=master)](https://travis-ci.org/evenorog/redo)
[![Crates.io](https://img.shields.io/crates/v/redo.svg)](https://crates.io/crates/redo)
[![Docs](https://docs.rs/redo/badge.svg)](https://docs.rs/redo)

```toml
[dependencies]
redo = "0.1.0"
```

[dynamic dispatch]: https://doc.rust-lang.org/stable/book/trait-objects.html#dynamic-dispatch
[undo]: https://crates.io/crates/undo
