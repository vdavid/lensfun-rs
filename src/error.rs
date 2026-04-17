//! Crate-level error type.

use std::path::PathBuf;

/// Anything that can go wrong loading or querying a `lensfun` database.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// I/O error reading a database file.
    #[error("io error reading {path}: {source}", path = path.display())]
    Io {
        /// Path that triggered the error.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// XML parse error in a database file.
    #[error("xml parse error in {path}: {message}", path = path.display())]
    Xml {
        /// Path that triggered the error.
        path: PathBuf,
        /// Human-readable error from the XML parser.
        message: String,
    },

    /// A required attribute or element was missing or had an unexpected value.
    #[error("invalid database entry in {path}: {message}", path = path.display())]
    InvalidEntry {
        /// Path that triggered the error.
        path: PathBuf,
        /// What was wrong.
        message: String,
    },

    /// Lookup found no matching entry.
    #[error("no match: {0}")]
    NoMatch(String),
}

/// Convenient `Result` alias.
pub type Result<T> = std::result::Result<T, Error>;
