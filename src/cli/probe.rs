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
//! until both verdicts are final or the deadline passes. The reader then drains
//! any trailing bytes so an unanswered reply is never left in the terminal's
//! input queue, where the shell would echo it. Probing both concurrently is not
//! an option — they share one `/dev/tty` input stream, so concurrent readers
//! would race for the same bytes and the raw-mode termios writes.

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

/// Read from `reader` until `done` accepts the accumulated bytes or `timeout`
/// elapses, whichever comes first, and return the bytes read.
///
/// Once `done` is satisfied the reader keeps draining until the stream goes
/// quiet (a read returns no data) rather than stopping at the very byte that
/// satisfied `done`. More of the reply can follow that byte — the terminator of
/// the Kitty handshake, or a second reply to a query sent in the same round trip
/// (FR-3.12, FR-3.13) — and leaving it in the terminal's input queue would leak
/// it into the shell as if the user had typed it. Draining costs at most one
/// read-timeout window after the verdict is final.
///
/// The caller must first put `reader` in raw mode with a read timeout (`VMIN` /
/// `VTIME`), so each read returns promptly instead of blocking for input.
fn read_until<R, F>(reader: &mut R, timeout: Duration, done: &F) -> std::io::Result<Vec<u8>>
where
    R: std::io::Read,
    F: Fn(&[u8]) -> bool,
{
    use std::time::Instant;

    let deadline = Instant::now() + timeout;
    let mut response = Vec::new();
    let mut byte = [0u8; 1];
    let mut settled = false;
    while Instant::now() < deadline {
        match reader.read(&mut byte) {
            // No data this read. Once the verdict is final the stream has gone
            // quiet, so the replies are fully drained; before that, keep waiting
            // for the first reply.
            Ok(0) => {
                if settled {
                    break;
                }
            }
            Ok(_) => {
                response.push(byte[0]);
                settled = done(&response);
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

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::time::{Duration, Instant};

    use super::*;

    /// A reader that yields its script byte by byte and then reports "no data"
    /// (like a terminal read that timed out), so the drain's quiet detection is
    /// exercisable without a real terminal.
    struct ScriptedReader {
        bytes: VecDeque<u8>,
    }

    impl ScriptedReader {
        fn new(bytes: &[u8]) -> Self {
            Self {
                bytes: bytes.iter().copied().collect(),
            }
        }
    }

    impl std::io::Read for ScriptedReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            match self.bytes.pop_front() {
                Some(byte) => {
                    buf[0] = byte;
                    Ok(1)
                }
                None => Ok(0),
            }
        }
    }

    #[test]
    fn read_until_drains_bytes_after_the_verdict() {
        // Regression: the Kitty verdict (`;OK`) lands mid-reply. Stopping at the
        // byte that satisfied `done` left the rest of that reply and the second
        // reply (the DA1 answer) in the terminal's input queue, which the shell
        // then echoed as garbage (WezTerm: `65;4;6;18;22c`). The reader must
        // drain everything the terminal sent.
        let reply = b"\x1b_Gi=1000000;OK\x1b\\\x1b[?65;4;6;18;22c";
        let mut reader = ScriptedReader::new(reply);
        let done = |buffer: &[u8]| crate::kitty::response_ok(buffer);
        let drained =
            read_until(&mut reader, Duration::from_secs(1), &done).expect("drain should succeed");
        assert_eq!(
            drained, reply,
            "trailing replies must be drained, not leaked"
        );
    }

    #[test]
    fn read_until_returns_at_the_deadline_without_a_verdict() {
        // With no reply the reader must still return (empty) once the deadline
        // passes, rather than spin forever.
        let mut reader = ScriptedReader::new(b"");
        let done = |_: &[u8]| false;
        let start = Instant::now();
        let drained = read_until(&mut reader, Duration::from_millis(50), &done)
            .expect("drain should succeed");
        assert!(drained.is_empty());
        assert!(start.elapsed() >= Duration::from_millis(40));
    }
}
