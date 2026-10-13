//! Draw an ink [`Frame`] with ANSI background-colour escape codes (FR-3.8,
//! FR-3.9).
//!
//! Each module is two spaces painted with an explicit background colour, so the
//! code is a self-contained dark-on-white image and the quiet zone stays white
//! even on a black terminal. Run-length collapsing of adjacent same-colour
//! modules keeps the escape count down.

use std::io::{self, Write};

use super::frame::Frame;
use super::style::RESET;

/// ANSI rendering: two spaces per module, coloured by an explicit background —
/// dark modules black on a white field (FR-3.8) — so the code is self-contained
/// and the quiet zone stays white even on a black terminal. `invert` swaps the
/// two colours (FR-3.9).
pub fn render_ansi(frame: &Frame, invert: bool, out: &mut dyn Write) -> io::Result<()> {
    // Each module is two spaces, so the background colour they carry paints one
    // square module. `ink` marks dark modules by default; `invert` flips that.
    let (module, field) = if invert {
        ("\x1b[107m", "\x1b[40m")
    } else {
        ("\x1b[40m", "\x1b[107m")
    };
    for y in 0..frame.height() {
        let mut line = String::with_capacity(frame.width() * 5);
        let mut previous: Option<bool> = None;
        for x in 0..frame.width() {
            let ink = frame.at(x, y);
            if previous != Some(ink) {
                line.push_str(if ink { module } else { field });
                previous = Some(ink);
            }
            line.push_str("  ");
        }
        line.push_str(RESET);
        line.push('\n');
        out.write_all(line.as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2x2 ink grid: top-left and bottom-right are ink.
    fn sample() -> Frame {
        Frame::new(2, 2, vec![true, false, false, true])
    }

    #[test]
    fn ansi_paints_dark_modules_black_on_a_white_field() {
        let mut buffer = Vec::new();
        render_ansi(&sample(), false, &mut buffer).unwrap();
        let text = String::from_utf8(buffer).unwrap();
        // Ink (a dark module) carries the black background; the field the white one.
        assert_eq!(
            text,
            "\x1b[40m  \x1b[107m  \x1b[0m\n\x1b[107m  \x1b[40m  \x1b[0m\n"
        );
    }

    #[test]
    fn ansi_invert_swaps_the_two_backgrounds() {
        let mut buffer = Vec::new();
        render_ansi(&sample(), true, &mut buffer).unwrap();
        let text = String::from_utf8(buffer).unwrap();
        assert!(
            text.starts_with("\x1b[107m  \x1b[40m  \x1b[0m\n"),
            "{text:?}"
        );
    }
}
