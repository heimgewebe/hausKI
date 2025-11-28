//! Workspace-local stub mirroring a subset of `anyhow` 1.0.100 for offline builds.
//! This intentionally avoids pulling the full upstream crate to keep vendor
//! contents stable; it implements only the APIs currently exercised in the
//! workspace and omits features like backtrace capture or private internals.

use std::error::Error as StdError;
use std::fmt;
use std::result;

/// Alias for results returned by `anyhow`-style APIs.
pub type Result<T, E = Error> = result::Result<T, E>;

/// Error wrapper compatible with the upstream `anyhow::Error` type.
///
/// This intentionally does **not** implement [`std::error::Error`] to avoid
/// coherence issues observed with the original upstream crate in this
/// workspace. Add the trait implementation only if a concrete call site needs
/// it.
#[derive(Debug)]
pub struct Error {
    inner: Box<dyn StdError + Send + Sync + 'static>,
}

impl Error {
    /// Create an error from a displayable message.
    pub fn msg<M>(message: M) -> Self
    where
        M: fmt::Display + Send + Sync + 'static,
    {
        Self {
            inner: Box::new(StringError(message.to_string())),
        }
    }

    /// Wrap an existing error value.
    pub fn new<E>(error: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self {
            inner: Box::new(error),
        }
    }

    /// Downcast the error to a concrete type by value.
    pub fn downcast<T>(self) -> result::Result<T, Error>
    where
        T: StdError + Send + Sync + 'static,
    {
        match self.inner.downcast::<T>() {
            Ok(concrete) => Ok(*concrete),
            Err(inner) => Err(Error { inner }),
        }
    }

    /// Downcast the error to a reference of a concrete type.
    pub fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: StdError + Send + Sync + 'static,
    {
        self.inner.downcast_ref::<T>()
    }

    /// Downcast the error to a mutable reference of a concrete type.
    pub fn downcast_mut<T>(&mut self) -> Option<&mut T>
    where
        T: StdError + Send + Sync + 'static,
    {
        self.inner.downcast_mut::<T>()
    }

    /// Returns the underlying source error, if any.
    pub fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.inner.source()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

impl<E> From<E> for Error
where
    E: StdError + Send + Sync + 'static,
{
    fn from(error: E) -> Self {
        Error::new(error)
    }
}

/// Extension trait providing context for errors.
pub trait Context<T> {
    /// Attach context generated on demand.
    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static,
        F: FnOnce() -> C;

    /// Attach an immediate context value.
    fn context<C>(self, context: C) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static;
}

impl<T, E> Context<T> for result::Result<T, E>
where
    E: StdError + Send + Sync + 'static,
{
    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        self.map_err(|error| Error::new(ContextError::new(f(), error)))
    }

    fn context<C>(self, context: C) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static,
    {
        self.map_err(|error| Error::new(ContextError::new(context, error)))
    }
}

impl<T> Context<T> for Option<T> {
    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        self.ok_or_else(|| Error::msg(f()))
    }

    fn context<C>(self, context: C) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static,
    {
        self.ok_or_else(|| Error::msg(context))
    }
}

struct ContextError {
    context: String,
    source: Box<dyn StdError + Send + Sync + 'static>,
}

impl ContextError {
    fn new<C, E>(context: C, error: E) -> Self
    where
        C: fmt::Display,
        E: StdError + Send + Sync + 'static,
    {
        ContextError {
            context: context.to_string(),
            source: Box::new(error),
        }
    }
}

impl fmt::Display for ContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.context)
    }
}

impl fmt::Debug for ContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContextError")
            .field("context", &self.context)
            .finish()
    }
}

impl StdError for ContextError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(self.source.as_ref())
    }
}

#[derive(Debug)]
struct StringError(String);

impl fmt::Display for StringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl StdError for StringError {}

/// Construct an [`Error`] from a string or format arguments.
#[macro_export]
macro_rules! anyhow {
    ($msg:literal $(, $($arg:tt)+)?) => {
        $crate::Error::msg(format!($msg $(, $($arg)+)?))
    };
    ($err:expr) => {
        $crate::Error::from($err)
    };
}

/// Return early with an error constructed via [`anyhow!`].
#[macro_export]
macro_rules! bail {
    ($($arg:tt)+) => {
        return ::core::result::Result::Err($crate::anyhow!($($arg)+));
    };
}

/// Return an error if a condition fails.
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $($arg:tt)+) => {
        if !$cond {
            $crate::bail!($($arg)+);
        }
    };
    ($cond:expr) => {
        if !$cond {
            $crate::bail!("condition failed: {}", ::core::stringify!($cond));
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_adds_message() {
        let err = Err::<(), _>(StringError("root".into()))
            .context("outer")
            .unwrap_err();
        assert_eq!(format!("{}", err), "outer");
        assert!(err.source().is_some());
    }

    #[test]
    fn ensure_macro_triggers() {
        fn check(val: i32) -> Result<()> {
            ensure!(val > 0, "value must be positive: {}", val);
            Ok(())
        }
        assert!(check(1).is_ok());
        assert!(check(-1).is_err());
    }
}
