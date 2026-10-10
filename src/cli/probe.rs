//! Graphics-protocol capability probes (FR-3.12, FR-3.13).
//!
//! A probe reads from the controlling terminal, so the reply never disturbs the
//! payload stream (which may arrive on stdin). A terminal that cannot draw the
//! bitmap is a hard error: printing escape bytes it cannot interpret would only
//! spew garbage.
//!
//! Kitty answers an `a=q` handshake with `;OK`; Sixel has no such round trip, so
//! its support is inferred from the DA1 (Primary Device Attributes) reply, which
//! lists device attribute `4` when Sixel is available.
//!
//! Both protocols are asked **in a single round trip** ([`probe_capabilities`]):
//! the two queries are written back-to-back and one reader collects the replies
//! until both verdicts are final or the deadline passes. Probing both
//! concurrently is not an option — they share one `/dev/tty` input stream, so
//! concurrent readers would race for the same bytes and the raw-mode termios
//! writes.

use std::io::{IsTerminal, Write};
use std::time::Duration;

use crate::error::AppError;

/// Image id used for the Kitty capability probe, kept well clear of the display
/// range.
const KITTY_PROBE_ID: u32 = 1_000_000;

/// How long to wait for probe replies before giving up.
const PROBE_TIMEOUT: Duration = Duration::from_millis(300);

/// Terminal graphics capabilities discovered by a probe.
pub(super) struct Capabilities {
    /// The terminal implements the Kitty graphics protocol.
    pub(super) kitty: bool,
    /// The terminal advertises Sixel graphics (device attribute `4`).
    pub(super) sixel: bool,
}

/// Probe the terminal for both graphics protocols in one round trip (FR-3.12,
/// FR-3.13).
///
/// Returns `Ok(None)` when there is no terminal to ask (standard output is not a
/// terminal); otherwise both verdicts. Waits for both replies (or the deadline),
/// so the result is complete for `--help`.
pub(super) fn probe_capabilities() -> Result<Option<Capabilities>, AppError> {
    let kitty_query = crate::kitty::query(KITTY_PROBE_ID);
    let da1_query = crate::sixel::da1_query();
    let buffer = round_trip(
        "cannot open the controlling terminal",
        "cannot query the terminal",
        &[&kitty_query, &da1_query],
        |buffer| crate::kitty::has_reply(buffer) && crate::sixel::has_reply(buffer),
    )?;
    Ok(buffer.map(|buffer| capabilities_of(&buffer)))
}

/// Probe the terminal for the best protocol to use for [`crate::types::Mode::Auto`]
/// (FR-3.12, FR-3.13).
///
/// Like [`probe_capabilities`], but stops as soon as Kitty is confirmed (it is
/// strictly preferred, so the Sixel verdict is irrelevant then). Returns
/// `Ok(None)` when there is no terminal to ask.
pub(super) fn probe_for_auto() -> Result<Option<Capabilities>, AppError> {
    let kitty_query = crate::kitty::query(KITTY_PROBE_ID);
    let da1_query = crate::sixel::da1_query();
    let buffer = round_trip(
        "cannot open the controlling terminal",
        "cannot query the terminal",
        &[&kitty_query, &da1_query],
        |buffer| {
            crate::kitty::response_ok(buffer)
                || (crate::kitty::has_reply(buffer) && crate::sixel::has_reply(buffer))
        },
    )?;
    Ok(buffer.map(|buffer| capabilities_of(&buffer)))
}

/// Ensure the current terminal implements the Kitty graphics protocol (FR-3.12).
///
/// A terminal that cannot draw the bitmap is a hard error: printing escape bytes
/// it cannot interpret would only spew garbage.
pub(super) fn ensure_kitty_supported() -> Result<(), AppError> {
    let kitty_query = crate::kitty::query(KITTY_PROBE_ID);
    let buffer = round_trip(
        "kitty: cannot open the controlling terminal",
        "kitty: cannot query the terminal",
        &[&kitty_query],
        crate::kitty::has_reply,
    )?;
    match buffer {
        None => Err(AppError::Usage(
            "kitty: standard output is not a terminal".to_string(),
        )),
        Some(buffer) if crate::kitty::response_ok(&buffer) => Ok(()),
        Some(_) => Err(AppError::Usage(
            "kitty: this terminal does not support the Kitty graphics protocol \
             (try kitty, Ghostty, or WezTerm)"
                .to_string(),
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
    let da1_query = crate::sixel::da1_query();
    let buffer = round_trip(
        "sixel: cannot open the controlling terminal",
        "sixel: cannot query the terminal",
        &[&da1_query],
        crate::sixel::has_reply,
    )?;
    match buffer {
        None => Err(AppError::Usage(
            "sixel: standard output is not a terminal".to_string(),
        )),
        Some(buffer) if crate::sixel::response_has_sixel(&buffer) => Ok(()),
        Some(_) => Err(AppError::Usage(
            "sixel: this terminal does not advertise Sixel graphics support \
             (try xterm, Konsole, foot, or Windows Terminal)"
                .to_string(),
        )),
    }
}

/// Verdict both protocols from a collected reply buffer.
fn capabilities_of(buffer: &[u8]) -> Capabilities {
    Capabilities {
        kitty: crate::kitty::response_ok(buffer),
        sixel: crate::sixel::response_has_sixel(buffer),
    }
}

/// Send every query in `queries` to the controlling terminal at once and read
/// until `done` is satisfied or the deadline passes.
///
/// Returns `Ok(None)` when there is no terminal to ask (standard output is not a
/// terminal). Sharing one round trip means `auto` pays a single timeout window
/// for both protocols instead of chaining two.
fn round_trip<F>(
    open_context: &'static str,
    query_context: &'static str,
    queries: &[&[u8]],
    done: F,
) -> Result<Option<Vec<u8>>, AppError>
where
    F: Fn(&[u8]) -> bool,
{
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
    probe(&mut tty, queries, &done)
        .map(Some)
        .map_err(|source| AppError::Io {
            context: query_context,
            source,
        })
}

/// Write `queries` and read the reply in raw mode, with a short timeout.
fn probe<F>(tty: &mut std::fs::File, queries: &[&[u8]], done: &F) -> std::io::Result<Vec<u8>>
where
    F: Fn(&[u8]) -> bool,
{
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

    for query in queries {
        tty.write_all(query)?;
    }
    tty.flush()?;

    read_until(tty, PROBE_TIMEOUT, done)
}

/// Read from `tty` until `done` accepts the accumulated bytes or `timeout`
/// elapses, whichever comes first, and return the bytes read.
///
/// The caller must first put `tty` in raw mode with a read timeout (`VMIN` /
/// `VTIME`), so each read returns promptly instead of blocking for input.
fn read_until<F>(tty: &mut std::fs::File, timeout: Duration, done: &F) -> std::io::Result<Vec<u8>>
where
    F: Fn(&[u8]) -> bool,
{
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
                if done(&response) {
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
