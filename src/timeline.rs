use crate::Command;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A timeline of commands.
///
/// Can be used with `no_std`.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default)]
struct Timeline<R, C: Command<R>> {
    commands: [C; 32],
    receiver: R,
    current: usize,
}
