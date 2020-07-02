use std::{result, fmt, error};
use std::fmt::Display;

pub type Result<T> = result::Result<T,Error>;

/// Return an `Error` from a function.
///
/// `bail!("something went wrong")` is equivalent to:
///
/// ```rust.ignore
/// return Err(Errror::message(format!("something went wrong", )));
/// ```
///
#[macro_export]
macro_rules! bail {
    ($e:expr) => {
        return Err($crate::error::Error::message($e));
    };
    ($fmt:expr, $($arg:tt)*) => {
        return Err($crate::error::Error::message(format!($fmt, $($arg)*)));
    };
}

/// Create an `Error::Message` instance by formatting a string
#[macro_export]
macro_rules! format_err {
    ($($arg:tt)*) => {
        $crate::error::Error::message(format!($($arg)*))
    }
}

/// for use in map_err()
///
/// ```
///   map_err(context!("something went wrong with {:?}", path))
/// ```
///
/// is the same as
///
/// ```
///   map_err(|e| format_err!("something went wrong with {:?}: {}", path, e))
/// ```
///
#[macro_export]
macro_rules! context {
    ($($arg:tt)*) => { |e|
        $crate::Error::with_error(format!($($arg)*), e)
    }
}

#[derive(Debug)]
pub enum Error {
    Message(String),
}

impl Error {
    pub fn message<S: Into<String>>(msg: S) -> Self {
        Error::Message(msg.into())
    }

    pub fn with_error<S,D>(msg: S, err: D) -> Self
    where
        S: Into<String>,
        D: Display,
    {
        let msg = msg.into();
        Self::message(format!("{}: {}", msg, err))
    }

}

impl error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Message(msg) => msg.fmt(f),
        }
    }
}
