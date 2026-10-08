//! Application error type and exit-code mapping (FR-6.1).
//!
//! Messages are built from static text plus an OS error description only; the
//! payload is never included (FR-6.2), because it may hold secrets such as
//! tokens or Wi-Fi credentials.

use std::fmt;

/// A fatal error, mapped to a process exit code by [`AppError::exit_code`].
#[derive(Debug)]
pub enum AppError {
    /// Invalid usage or a bad input source; exit code `2`.
    Usage(String),
    /// Empty or unencodable input; exit code `2`.
    Input(String),
    /// A runtime/IO failure; exit code `1`.
    Io {
        /// Static description of the failed operation (never the payload).
        context: &'static str,
        /// The underlying OS error.
        source: std::io::Error,
    },
}

impl AppError {
    /// Maps the error to the process exit code (aligned with qrpipe: 0/2/1).
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            AppError::Io { .. } => 1,
            AppError::Usage(_) | AppError::Input(_) => 2,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Usage(message) | AppError::Input(message) => f.write_str(message),
            AppError::Io { context, source } => write!(f, "{context}: {source}"),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
