//! Errors as strings.
//!
//! The CLI's job is to print one readable line and exit non-zero. `anyhow` does
//! that well, but it is a dependency for a `String` with a wrapper around it, so
//! the wrapper is three functions here instead.

pub type Result<T> = std::result::Result<T, String>;

/// Attach "what was happening" to any error, keeping the original message.
pub trait Context<T> {
    fn context(self, what: impl Into<String>) -> Result<T>;
    fn with_context<F: FnOnce() -> String>(self, what: F) -> Result<T>;
}

impl<T, E: std::fmt::Display> Context<T> for std::result::Result<T, E> {
    fn context(self, what: impl Into<String>) -> Result<T> {
        self.map_err(|error| format!("{}: {error}", what.into()))
    }

    fn with_context<F: FnOnce() -> String>(self, what: F) -> Result<T> {
        self.map_err(|error| format!("{}: {error}", what()))
    }
}

#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return ::std::result::Result::Err(format!($($arg)*))
    };
}
