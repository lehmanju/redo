//! **High-level undo-redo functionality.**
//!
//! It is an implementation of the command pattern, where all modifications are done
//! by creating objects of commands that applies the modifications. All commands knows
//! how to undo the changes it applies, and by using the provided data structures
//! it is easy to apply, undo, and redo changes made to a target.

#![doc(html_root_url = "https://docs.rs/redo")]
#![deny(missing_docs)]
#![forbid(unsafe_code)]

#[cfg(feature = "serde")]
use serde_crate::{Deserialize, Serialize};
use undo::{Command, History as Inner, Signal};

/// A history of commands.
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(
        crate = "serde_crate",
        bound(
            serialize = "C: Command + Serialize, C::Target: Serialize",
            deserialize = "C: Command + Deserialize<'de>, C::Target: Deserialize<'de>"
        )
    )
)]
#[derive(Clone)]
pub struct History<C: Command, F = Box<dyn FnMut(Signal)>> {
    inner: Inner<C, F>,
    target: C::Target,
}

impl<C: Command> History<C> {
    /// Returns a new history.
    pub fn new(target: C::Target) -> History<C> {
        History {
            inner: Inner::new(),
            target,
        }
    }
}
