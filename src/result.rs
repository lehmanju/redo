use crate::{Command, Meta};
use std::fmt;

/// A specialized Result type for undo-redo operations.
pub type Result<R, C> = std::result::Result<(), Error<R, C>>;

/// An error which holds the command that caused it.
pub struct Error<R, C: Command<R>> {
    pub(crate) meta: Meta<C>,
    pub(crate) error: C::Error,
}

impl<R, C: Command<R>> Error<R, C> {
    /// Returns a reference to the command that caused the error.
    #[inline]
    pub fn command(&self) -> &C {
        &self.meta.command
    }

    /// Returns the command that caused the error.
    #[inline]
    pub fn into_command(self) -> C {
        self.meta.command
    }
}

impl<R, C: Command<R> + fmt::Debug> fmt::Debug for Error<R, C>
where
    C::Error: fmt::Debug,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Error")
            .field("meta", &self.meta)
            .field("error", &self.error)
            .finish()
    }
}

impl<R, C: Command<R>> fmt::Display for Error<R, C>
where
    C::Error: fmt::Display,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.error as &dyn fmt::Display).fmt(f)
    }
}

#[cfg(feature = "std")]
impl<R, C: Command<R>> std::error::Error for Error<R, C>
where
    C: fmt::Debug,
    C::Error: std::error::Error,
{
    #[inline]
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.error.source()
    }
}
