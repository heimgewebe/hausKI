use core::fmt;
use std::error::Error as StdError;
use std::result;

/// Alias for results returned by `anyhow`-style APIs.
pub type Result<T> = result::Result<T, Error>;

/// Lightweight error wrapper compatible with `anyhow` macros.
///
/// Note: Unlike some error types, `Error` intentionally does not implement
/// `std::error::Error`. This matches the behavior of the real `anyhow` crate
/// and avoids conflicting with `impl<T> From<T> for T` from core.
#[derive(Debug)]
pub struct Error {
    inner: Box<dyn StdError + Send + Sync + 'static>,
}

impl Error {
    pub fn msg<M: fmt::Display + Send + Sync + 'static>(message: M) -> Self {
        Self {
            inner: Box::new(StringError(message.to_string())),
        }
    }

    /// Wrap an existing error.
    pub fn new<E: StdError + Send + Sync + 'static>(error: E) -> Self {
        Self {
            inner: Box::new(error),
        }
    }

    /// Returns the root cause of this error, if any.
    pub fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.inner.source()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl<E> From<E> for Error
where
    E: StdError + Send + Sync + 'static,
{
    fn from(err: E) -> Self {
        Self::new(err)
    }
}

/// Extend `Result` with context helpers similar to the real crate.
pub trait Context<T> {
    fn context<C>(self, context: C) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static;

    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static,
        F: FnOnce() -> C;
}

impl<T, E> Context<T> for result::Result<T, E>
where
    E: StdError + Send + Sync + 'static,
{
    fn context<C>(self, context: C) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static,
    {
        self.map_err(|err| Error::msg(format!("{}: {}", context, err)))
    }

    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: fmt::Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        self.map_err(|err| Error::msg(format!("{}: {}", f(), err)))
    }
}

#[macro_export]
macro_rules! anyhow {
    ($($arg:tt)*) => {
        $crate::Error::msg(format!($($arg)*))
    };
}

#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err($crate::anyhow!($($arg)*));
    };
}

#[macro_export]
macro_rules! ensure {
    ($cond:expr, $($arg:tt)*) => {
        if !($cond) {
            $crate::bail!($($arg)*);
        }
    };
}

#[derive(Debug)]
struct StringError(String);

impl fmt::Display for StringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl StdError for StringError {}
