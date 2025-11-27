use core::fmt;
use std::error::Error as StdError;
use std::result;

/// Alias for results returned by `anyhow`-style APIs.
pub type Result<T> = result::Result<T, Error>;

/// Lightweight error wrapper compatible with `anyhow` macros.
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
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.inner.source()
    }
}

impl<E> From<E> for Error
where
    E: StdError + Send + Sync + 'static,
{
    fn from(err: E) -> Self {
        Self {
            inner: Box::new(err),
        }
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
