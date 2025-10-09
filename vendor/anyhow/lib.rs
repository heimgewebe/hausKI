#![forbid(unsafe_code)]

use core::fmt;
use std::error::Error as StdError;
use std::result::Result as StdResult;

/// A simplified error type compatible with `anyhow::Error`.
pub struct Error {
    inner: Box<dyn StdError + Send + Sync + 'static>,
}

impl Error {
    fn new(inner: Box<dyn StdError + Send + Sync + 'static>) -> Self {
        Self { inner }
    }

    /// Create an error from a displayable message.
    pub fn msg<M>(message: M) -> Self
    where
        M: fmt::Display + Send + Sync + 'static,
    {
        Self::new(Box::new(MessageError(message.to_string())))
    }

    /// Attach additional context to this error.
    pub fn context<C>(self, context: C) -> Self
    where
        C: fmt::Display + Send + Sync + 'static,
    {
        Self::new(Box::new(ContextError {
            context: context.to_string(),
            source: self,
        }))
    }

    fn as_dyn_error(&self) -> &(dyn StdError + Send + Sync + 'static) {
        &*self.inner
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.inner.as_ref(), f)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.inner.as_ref(), f)
    }
}

// NOTE: This implementation is commented out to resolve a conflict with the
// generic `impl<E: StdError> From<E> for Error`. The conflict arises because
// this impl makes `Error` itself a `StdError`, which causes the generic `From`
// to conflict with the standard library's `impl<T> From<T> for T`.
/*
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.inner.source()
    }
}
*/

impl<E> From<E> for Error
where
    E: StdError + Send + Sync + 'static,
{
    fn from(error: E) -> Self {
        Self::new(Box::new(error))
    }
}

#[derive(Debug)]
struct MessageError(String);

impl fmt::Display for MessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl StdError for MessageError {}

#[derive(Debug)]
struct ContextError {
    context: String,
    source: Error,
}

impl fmt::Display for ContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.context, self.source)
    }
}

impl StdError for ContextError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(self.source.as_dyn_error())
    }
}

/// Convenient result type alias.
pub type Result<T> = StdResult<T, Error>;

/// Additional context on [`Result`] and [`Option`].
pub trait Context<T> {
    fn context<C>(self, context: C) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static;

    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static,
        F: FnOnce() -> C;
}

impl<T, E> Context<T> for core::result::Result<T, E>
where
    Error: From<E>,
{
    fn context<C>(self, context: C) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static,
    {
        self.map_err(|error| Error::from(error).context(context))
    }

    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        self.map_err(|error| Error::from(error).context(f()))
    }
}

impl<T> Context<T> for Option<T> {
    fn context<C>(self, context: C) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static,
    {
        self.ok_or_else(|| Error::msg(context))
    }

    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        self.ok_or_else(|| Error::msg(f()))
    }
}

/// Creates an [`Error`] from the supplied message or error value.
#[macro_export]
macro_rules! anyhow {
    ($($arg:tt)+) => {
        $crate::Error::msg(format!($($arg)+))
    };
}

/// Early-return an error constructed via [`anyhow!`].
#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err($crate::anyhow!($($arg)*));
    };
}

/// Mirror of `anyhow::ensure!` for common guard checks.
#[macro_export]
macro_rules! ensure {
    ($cond:expr $(,)?) => {
        if !$cond {
            return Err($crate::anyhow!("condition failed: {}", stringify!($cond)));
        }
    };
    ($cond:expr, $fmt:expr $(, $arg:tt)*) => {
        if !$cond {
            return Err($crate::anyhow!($fmt $(, $arg)*));
        }
    };
}

// Public re-exports to match anyhow's surface a bit closer.
pub use crate::{Context as _};