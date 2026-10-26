//! Safe terminal capability queries over the Windows console (FR-3.12, FR-3.13).
//!
//! [`probe`] writes one or more queries to standard output and collects the
//! terminal's replies from `CONIN$`. The Win32 calls live in the private
//! [`win32`] module, where each function opts into `unsafe`; this crate root
//! denies unsafe everywhere else. The polling loop, UTF-16 decoding, and reply
//! handling are safe and covered by unit tests on every platform.
//!
//! The caller must check that standard output is a terminal before calling
//! [`probe`]; otherwise the escape sequences would be written into a redirected
//! stream. A terminal that does not answer simply times out with an empty reply.

#![deny(unsafe_code)]
#![warn(missing_docs)]

use std::io;
use std::time::Duration;

#[cfg(any(windows, test))]
mod collect;
#[cfg(any(windows, test))]
mod event;
#[cfg(any(windows, test))]
mod utf16;
#[cfg(windows)]
mod win32;

/// Send `queries` to the terminal and collect the reply bytes.
///
/// The queries are written to standard output in order and flushed. Replies are
/// read until `done` accepts the collected bytes — trailing bytes are still
/// drained until the console input queue goes quiet — or `timeout` elapses.
///
/// An empty result means the terminal did not answer before the deadline.
/// Callers that need a verdict should treat that as "unsupported".
///
/// The caller must have verified that standard output is a terminal.
///
/// # Errors
///
/// Returns the underlying I/O error when the console cannot be opened, a query
/// cannot be written, or the console read fails. On platforms other than
/// Windows this returns [`io::ErrorKind::Unsupported`].
pub fn probe(
    queries: &[&[u8]],
    timeout: Duration,
    done: &dyn Fn(&[u8]) -> bool,
) -> io::Result<Vec<u8>> {
    #[cfg(windows)]
    {
        let mut source = win32::ConsoleEvents::open()?;
        let mut stdout = io::stdout();
        collect::transceive(&mut source, &mut stdout, queries, timeout, done)
    }
    #[cfg(not(windows))]
    {
        let _ = (queries, timeout, done);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "terminal queries require the Windows console",
        ))
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn probe_is_unsupported_off_windows() {
        let error = probe(&[b"query"], Duration::from_millis(1), &|_: &[u8]| false)
            .expect_err("non-Windows probing must be unsupported");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }
}
