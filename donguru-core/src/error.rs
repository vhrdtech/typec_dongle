//! Typed error categories for the core library.
//!
//! Users like the CLI tools, mapping these variants onto stable exit codes (see `donguru-cli/src/exit.rs`),
//! so scripts can branch on the failure category.

/// Convenience alias
pub type Result<T> = std::result::Result<T, Error>;

/// Failure categories reported by the core library.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// No dongle matched the selector, or the selection was ambiguous.
    #[error("no matching device: {0}")]
    NoDevice(String),

    /// The dongle itself reported an error.
    #[error("device error: {0}")]
    Device(String),

    /// An operation did not complete in time.
    #[error("timed out: {0}")]
    Timeout(String),
}
