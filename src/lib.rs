//! **High-level undo-redo functionality.**
//!
//! It is an implementation of the command pattern, where all modifications are done
//! by creating objects of commands that applies the modifications. All commands knows
//! how to undo the changes it applies, and by using the provided data structures
//! it is easy to apply, undo, and redo changes made to a target.
//!
//! # Features
//!
//! * [Command](trait.Command.html) provides the base functionality for all commands.
//! * [Record](struct.Record.html) provides basic linear undo-redo functionality.
//! * [History](struct.History.html) provides non-linear undo-redo functionality that allows you to jump between different branches.
//! * Queue wraps a record or history and extends them with queue functionality.
//! * Checkpoint wraps a record or history and extends them with checkpoint functionality.
//! * Commands can be merged into a single command by implementing the
//!   [merge](trait.Command.html#method.merge) method on the command.
//!   This allows smaller commands to be used to build more complex operations, or smaller incremental changes to be
//!   merged into larger changes that can be undone and redone in a single step.
//! * The target can be marked as being saved to disk and the data-structures can track the saved state and notify
//!   when it changes.
//! * The amount of changes being tracked can be configured by the user so only the `N` most recent changes are stored.
//! * Configurable display formatting using the display structure.
//! * The library can be used as `no_std` by default.
//!
//! # Cargo Feature Flags
//!
//! * `chrono`: Enables time stamps and time travel.
//! * `serde`: Enables serialization and deserialization.
//! * `colored`: Enables colored output when visualizing the display structures.

#![doc(html_root_url = "https://docs.rs/redo")]
#![deny(missing_docs)]
#![forbid(unsafe_code)]
