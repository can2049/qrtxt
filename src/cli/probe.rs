//! Kitty graphics protocol capability probe (FR-3.12).
//!
//! The `a=q` handshake is read from the controlling terminal, so the reply never
//! disturbs the payload stream (which may arrive on stdin). A terminal that
//! cannot draw the bitmap is a hard error: printing escape bytes it cannot
//! interpret would only spew garbage.

use std::io::{IsTerminal, Write};

use crate::error::AppError;

/// Image id used for the capability probe, kept well clear of the display range.
const KITTY_PROBE_ID: u32 = 1_000_000;

/// Ensure the current terminal implements the Kitty graphics protocol (FR-3.12).
///
/// The probe runs an `a=q` handshake over the controlling terminal, so the reply
/// is read without disturbing the payload stream (which may arrive on stdin). A
/// terminal that cannot draw the bitmap is a hard error: printing escape bytes it
/// cannot interpret would only spew garbage.
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

/// Probe the controlling terminal for Kitty graphics support (FR-3.12).
///
/// Returns `Ok(None)` when there is no terminal to ask (standard output is not a
/// terminal); otherwise `Ok(Some(true/false))` from the `a=q` handshake.
pub(super) fn probe_kitty_terminal() -> Result<Option<bool>, AppError> {
    if !std::io::stdout().is_terminal() {
        return Ok(None);
    }
    let mut tty = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .map_err(|source| AppError::Io {
            context: "kitty: cannot open the controlling terminal",
            source,
        })?;
    probe_kitty(&mut tty)
        .map(Some)
        .map_err(|source| AppError::Io {
            context: "kitty: cannot query the terminal",
            source,
        })
}

/// Send the support query and read the reply in raw mode, with a short timeout.
fn probe_kitty(tty: &mut std::fs::File) -> std::io::Result<bool> {
    use std::io::Read as _;
    use std::os::fd::AsFd as _;
    use std::time::{Duration, Instant};

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

    tty.write_all(&crate::kitty::query(KITTY_PROBE_ID))?;
    tty.flush()?;

    let deadline = Instant::now() + Duration::from_millis(300);
    let mut response = Vec::new();
    let mut byte = [0u8; 1];
    while Instant::now() < deadline {
        match tty.read(&mut byte) {
            Ok(0) => {} // the read timed out with no data
            Ok(_) => {
                response.push(byte[0]);
                if response.ends_with(b"\x1b\\") {
                    break;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(crate::kitty::response_ok(&response))
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
