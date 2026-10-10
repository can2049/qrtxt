//! Graphics-protocol capability probes (FR-3.12, FR-3.13).
//!
//! Both probes read from the controlling terminal, so the reply never disturbs
//! the payload stream (which may arrive on stdin). A terminal that cannot draw
//! the bitmap is a hard error: printing escape bytes it cannot interpret would
//! only spew garbage.
//!
//! Kitty answers an `a=q` handshake with `;OK`; Sixel has no such round trip, so
//! its support is inferred from the DA1 (Primary Device Attributes) reply, which
//! lists device attribute `4` when Sixel is available.

use std::io::{IsTerminal, Write};
use std::time::Duration;

use crate::error::AppError;

/// Image id used for the Kitty capability probe, kept well clear of the display
/// range.
const KITTY_PROBE_ID: u32 = 1_000_000;

/// How long to wait for a probe reply before giving up.
const PROBE_TIMEOUT: Duration = Duration::from_millis(300);

/// Ensure the current terminal implements the Kitty graphics protocol (FR-3.12).
///
/// A terminal that cannot draw the bitmap is a hard error: printing escape bytes
/// it cannot interpret would only spew garbage.
pub(super) fn ensure_kitty_supported() -> Result<(), AppError> {
    match probe_kitty_terminal()? {
        Some(true) => Ok(()),
        Some(false) => Err(AppError::Usage(
            "kitty: this terminal does not support the Kitty graphics protocol \
             (try kitty, Ghostty, or WezTerm)"
                .to_string(),
        )),
        None => Err(AppError::Usage(
            "kitty: standard output is not a terminal".to_string(),
        )),
    }
}

/// Ensure the current terminal implements the Sixel graphics protocol
/// (FR-3.13).
///
/// Sixel support is inferred from the DA1 reply (device attribute `4`), which is
/// best-effort: a terminal that supports Sixel without advertising it is treated
/// as unsupported, so we never print escape bytes a terminal cannot draw.
pub(super) fn ensure_sixel_supported() -> Result<(), AppError> {
    match probe_sixel_terminal()? {
        Some(true) => Ok(()),
        Some(false) => Err(AppError::Usage(
            "sixel: this terminal does not advertise Sixel graphics support \
             (try xterm, Konsole, foot, or Windows Terminal)"
                .to_string(),
        )),
        None => Err(AppError::Usage(
            "sixel: standard output is not a terminal".to_string(),
        )),
    }
}

/// Probe the controlling terminal for Kitty graphics support (FR-3.12).
///
/// Returns `Ok(None)` when there is no terminal to ask (standard output is not a
/// terminal); otherwise `Ok(Some(true/false))` from the `a=q` handshake.
pub(super) fn probe_kitty_terminal() -> Result<Option<bool>, AppError> {
    probe_terminal(
        "kitty: cannot open the controlling terminal",
        "kitty: cannot query the terminal",
        crate::kitty::query(KITTY_PROBE_ID),
        b"\x1b\\",
        crate::kitty::response_ok,
    )
}

/// Probe the controlling terminal for Sixel graphics support (FR-3.13).
///
/// Returns `Ok(None)` when there is no terminal to ask; otherwise
/// `Ok(Some(true/false))` from the DA1 reply.
pub(super) fn probe_sixel_terminal() -> Result<Option<bool>, AppError> {
    probe_terminal(
        "sixel: cannot open the controlling terminal",
        "sixel: cannot query the terminal",
        crate::sixel::da1_query(),
        b"c",
        crate::sixel::response_has_sixel,
    )
}

/// Send `query` to the controlling terminal and verdict the reply.
///
/// The two protocol probes differ only in the bytes they send, the terminator
/// that ends the reply, and how the reply is interpreted, so they share this
/// driver. `Ok(None)` means there was no terminal to ask.
fn probe_terminal(
    open_context: &'static str,
    query_context: &'static str,
    query: Vec<u8>,
    terminator: &[u8],
    verdict: impl Fn(&[u8]) -> bool,
) -> Result<Option<bool>, AppError> {
    if !std::io::stdout().is_terminal() {
        return Ok(None);
    }
    let mut tty = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .map_err(|source| AppError::Io {
            context: open_context,
            source,
        })?;
    probe(&mut tty, &query, terminator, verdict)
        .map(Some)
        .map_err(|source| AppError::Io {
            context: query_context,
            source,
        })
}

/// Send `query` and read the reply in raw mode, with a short timeout.
fn probe(
    tty: &mut std::fs::File,
    query: &[u8],
    terminator: &[u8],
    verdict: impl Fn(&[u8]) -> bool,
) -> std::io::Result<bool> {
    use std::os::fd::AsFd as _;

    use rustix::termios::{OptionalActions, SpecialCodeIndex, tcgetattr, tcsetattr};

    let original = tcgetattr(tty.as_fd())?;
    // A duplicate handle restores the attributes on the way out without holding a
    // borrow of `tty`, which stays free for reading and writing.
    let _guard = TermiosGuard {
        tty: tty.try_clone()?,
        original: original.clone(),
    };

    let mut raw = original;
    raw.make_raw();
    raw.special_codes[SpecialCodeIndex::VMIN] = 0;
    raw.special_codes[SpecialCodeIndex::VTIME] = 1; // 0.1s between reads
    tcsetattr(tty.as_fd(), OptionalActions::Now, &raw)?;

    tty.write_all(query)?;
    tty.flush()?;

    let response = read_reply_until(tty, PROBE_TIMEOUT, terminator)?;
    Ok(verdict(&response))
}

/// Read from `tty` until `terminator` arrives or `timeout` elapses, whichever
/// comes first, and return the bytes read.
///
/// The caller must first put `tty` in raw mode with a read timeout (`VMIN` /
/// `VTIME`), so each read returns promptly instead of blocking for input.
fn read_reply_until(
    tty: &mut std::fs::File,
    timeout: Duration,
    terminator: &[u8],
) -> std::io::Result<Vec<u8>> {
    use std::io::Read as _;
    use std::time::Instant;

    let deadline = Instant::now() + timeout;
    let mut response = Vec::new();
    let mut byte = [0u8; 1];
    while Instant::now() < deadline {
        match tty.read(&mut byte) {
            Ok(0) => {} // the read timed out with no data
            Ok(_) => {
                response.push(byte[0]);
                if response.ends_with(terminator) {
                    break;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(response)
}

/// Restores the terminal's saved attributes when dropped, even on early return.
struct TermiosGuard {
    tty: std::fs::File,
    original: rustix::termios::Termios,
}

impl Drop for TermiosGuard {
    fn drop(&mut self) {
        use std::os::fd::AsFd as _;

        let _ = rustix::termios::tcsetattr(
            self.tty.as_fd(),
            rustix::termios::OptionalActions::Now,
            &self.original,
        );
    }
}
