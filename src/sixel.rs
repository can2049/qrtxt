//! Sixel graphics protocol framing (FR-3.13).
//!
//! Pure byte-level helpers for the [Sixel graphics format]: they only ever touch
//! a caller-provided [`Write`], so the terminal probe and the image placement
//! live in [`crate::cli`] and [`crate::render`] respectively. Nothing here
//! depends on `qrcode`.
//!
//! A QR code is a black-and-white bitmap, so a whole frame uses just two palette
//! registers and a run-length-encoded column stream — no colour quantisation is
//! needed.
//!
//! [Sixel graphics format]: https://en.wikipedia.org/wiki/Sixel

use std::io::{self, Write};

/// DCS introducer that starts every sequence: `ESC P` immediately followed by
/// the Sixel mode selector `q`.
const DCS_START: &[u8] = b"\x1bPq";
/// String terminator that ends every sequence: `ESC \`.
const ST: &[u8] = b"\x1b\\";

/// Palette register holding the light colour (the quiet zone and light modules).
const LIGHT_REGISTER: u8 = 0;
/// Palette register holding the dark colour (the dark modules).
const DARK_REGISTER: u8 = 1;

/// Palette RGB values, on the protocol's 0..=100 scale.
const WHITE: [u8; 3] = [100, 100, 100];
const BLACK: [u8; 3] = [0, 0, 0];

/// Pixels per sixel band (a band is one column of sixel characters).
const BAND: usize = 6;

/// Shortest run worth compressing: `!Nc` only beats writing the characters once
/// `N >= 4` (the protocol's own recommendation).
const MIN_RUN: usize = 4;

/// The lowest byte a sixel data character can take (`?` = no pixels set).
const DATA_BASE: u8 = 0x3F;

/// Encode `dark` (`width` x `height`, row-major, `true` = dark pixel) as a Sixel
/// sequence drawn at the cursor.
///
/// The image is painted in two solid colours: a light pass covers every light
/// pixel, then a dark pass covers the rest, so the two passes together fill the
/// whole image. `invert` swaps the two colours for light-background terminals
/// (FR-3.9).
pub fn encode(
    dark: &[bool],
    width: usize,
    height: usize,
    invert: bool,
    out: &mut dyn Write,
) -> io::Result<()> {
    debug_assert_eq!(
        dark.len(),
        width * height,
        "bitmap must match the given dimensions"
    );

    // Default: dark modules are black on a white field (FR-3.8); `invert` swaps
    // the two colours (FR-3.9).
    let (light_rgb, dark_rgb) = if invert {
        (BLACK, WHITE)
    } else {
        (WHITE, BLACK)
    };

    out.write_all(DCS_START)?;
    write_palette(out, LIGHT_REGISTER, light_rgb)?;
    write_palette(out, DARK_REGISTER, dark_rgb)?;
    // Raster attributes: square pixels (`1;1`) and the exact image size. Square
    // pixels keep the QR modules square so the printed code still scans.
    write!(out, "\"1;1;{width};{height}")?;

    let mut top = 0;
    while top < height {
        // Light pass first, then carriage-return (`$`) and the dark pass, then
        // graphics newline (`-`) to the next band.
        write!(out, "#{LIGHT_REGISTER}")?;
        write_pass(out, dark, width, height, top, false)?;
        write!(out, "$#{DARK_REGISTER}")?;
        write_pass(out, dark, width, height, top, true)?;
        out.write_all(b"-")?;
        top += BAND;
    }
    out.write_all(ST)
}

/// Write one palette register definition: `#Pc;2;Pr;Pg;Pb` (RGB).
fn write_palette(out: &mut dyn Write, register: u8, [r, g, b]: [u8; 3]) -> io::Result<()> {
    write!(out, "#{register};2;{r};{g};{b}")
}

/// Write the sixel characters for one band and one colour.
///
/// `paint_dark` selects the pass: `false` paints light pixels, `true` paints dark
/// ones. Rows at or beyond `height` (a partial last band) contribute nothing.
fn write_pass(
    out: &mut dyn Write,
    dark: &[bool],
    width: usize,
    height: usize,
    top: usize,
    paint_dark: bool,
) -> io::Result<()> {
    let mut columns = Vec::with_capacity(width);
    for x in 0..width {
        let mut mask = 0u8;
        for bit in 0..BAND {
            let y = top + bit;
            if y < height && dark[y * width + x] == paint_dark {
                mask |= 1 << bit;
            }
        }
        columns.push(DATA_BASE + mask);
    }
    write_rle(out, &columns)
}

/// Write `columns`, collapsing runs of `MIN_RUN` or more into `!N<char>`.
fn write_rle(out: &mut dyn Write, columns: &[u8]) -> io::Result<()> {
    let mut index = 0;
    while index < columns.len() {
        let character = columns[index];
        let mut end = index + 1;
        while end < columns.len() && columns[end] == character {
            end += 1;
        }
        let run = end - index;
        if run >= MIN_RUN {
            write!(out, "!{run}")?;
            out.write_all(&[character])?;
        } else {
            out.write_all(&columns[index..end])?;
        }
        index = end;
    }
    Ok(())
}

/// The support-probe sequence: a Primary Device Attributes (DA1) query.
#[must_use]
pub fn da1_query() -> Vec<u8> {
    b"\x1b[c".to_vec()
}

/// The parameter list between `ESC [ ?` and the terminating `c`, if a DA1 reply
/// is present.
///
/// The reply looks like `ESC [ ? <P1>;<P2>;... c`.
fn da1_params(response: &[u8]) -> Option<&[u8]> {
    let start = find_subslice(response, b"\x1b[?")? + 3;
    let rest = &response[start..];
    let end = rest.iter().position(|&byte| byte == b'c')?;
    Some(&rest[..end])
}

/// Whether `response` contains a complete DA1 reply (`ESC [ ?` ... `c`),
/// regardless of the advertised attributes.
///
/// Distinguishes "the terminal answered the query" (so the verdict is final)
/// from "no reply yet".
#[must_use]
pub fn has_reply(response: &[u8]) -> bool {
    da1_params(response).is_some()
}

/// Whether a DA1 reply advertises Sixel support (device attribute `4`).
///
/// The reply looks like `ESC [ ? <P1>;<P2>;... c`; Sixel-capable terminals list
/// `4` among the parameters. Some terminals support Sixel without advertising
/// it, so a `false` here means "not confirmed" rather than "definitely absent".
#[must_use]
pub fn response_has_sixel(response: &[u8]) -> bool {
    da1_params(response).is_some_and(|params| {
        params
            .split(|&byte| byte == b';')
            .any(|param| param == b"4")
    })
}

/// Find the first occurrence of `needle` in `haystack`.
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bitmap `width` x `height` where `dark(x, y)` decides each pixel.
    fn bitmap(width: usize, height: usize, dark: impl Fn(usize, usize) -> bool) -> Vec<bool> {
        let mut pixels = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                pixels.push(dark(x, y));
            }
        }
        pixels
    }

    fn encode_str(dark: &[bool], width: usize, height: usize, invert: bool) -> String {
        let mut out = Vec::new();
        encode(dark, width, height, invert, &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn frame_is_wrapped_and_declares_the_palette_and_raster() {
        let pixels = bitmap(1, 6, |_, _| true);
        let text = encode_str(&pixels, 1, 6, false);
        assert!(text.starts_with("\x1bPq"), "{text:?}");
        assert!(text.ends_with("\x1b\\"), "{text:?}");
        // Register 0 is white, register 1 black; then the raster attributes.
        assert!(
            text.contains("#0;2;100;100;100#1;2;0;0;0\"1;1;1;6"),
            "{text:?}"
        );
    }

    #[test]
    fn invert_swaps_the_two_registers() {
        let pixels = bitmap(1, 6, |_, _| true);
        let text = encode_str(&pixels, 1, 6, true);
        // Now register 0 (the quiet-zone colour) is black and register 1 white.
        assert!(text.contains("#0;2;0;0;0#1;2;100;100;100"), "{text:?}");
    }

    #[test]
    fn a_fully_dark_band_sets_every_sixel_bit() {
        let pixels = bitmap(1, 6, |_, _| true);
        let text = encode_str(&pixels, 1, 6, false);
        // Light pass: no light pixel -> `?` (no bits). Dark pass: all six -> `~`.
        assert!(text.contains("#0?$#1~-"), "{text:?}");
    }

    #[test]
    fn runs_of_four_or_more_are_compressed() {
        // A full band of light pixels: both passes compress to `!8`.
        let pixels = bitmap(8, 6, |_, _| false);
        let text = encode_str(&pixels, 8, 6, false);
        assert!(text.contains("#0!8~"), "light run not compressed: {text:?}");
        assert!(text.contains("$#1!8?"), "dark run not compressed: {text:?}");
    }

    #[test]
    fn short_runs_are_written_literally() {
        // One dark column among light ones: three-in-a-row stays literal.
        let pixels = bitmap(3, 6, |x, _| x == 1);
        let text = encode_str(&pixels, 3, 6, false);
        assert!(text.contains("#0~?~"), "light pass: {text:?}");
        assert!(text.contains("$#1?~?"), "dark pass: {text:?}");
    }

    #[test]
    fn a_partial_last_band_leaves_trailing_rows_unpainted() {
        // Height 7: the second band has one real row, painted light only.
        let pixels = bitmap(1, 7, |_, _| false);
        let text = encode_str(&pixels, 1, 7, false);
        assert!(text.contains("\"1;1;1;7"), "{text:?}");
        // Second band: row 6 is light -> bit 0 set (0x40 `@`); rows 7.. are skipped.
        assert!(
            text.contains("#0@$#1?-") || text.contains("#0@$#1?"),
            "{text:?}"
        );
    }

    #[test]
    fn da1_query_is_the_primary_device_attributes_request() {
        assert_eq!(da1_query(), b"\x1b[c".to_vec());
    }

    #[test]
    fn response_has_sixel_reads_the_attribute_list() {
        assert!(response_has_sixel(b"\x1b[?62;1;4;22c"));
        assert!(response_has_sixel(b"\x1b[?4c"));
        assert!(!response_has_sixel(b"\x1b[?62;1;22c"));
        assert!(!response_has_sixel(b"\x1b[?1;2;6c"));
        assert!(!response_has_sixel(b""));
    }

    #[test]
    fn has_reply_needs_a_terminated_da1() {
        // Any DA1 reply counts, even when it does not advertise Sixel.
        assert!(has_reply(b"\x1b[?62;1;22c"));
        assert!(has_reply(b"\x1b[?4c"));
        // Unterminated or absent means no reply yet.
        assert!(!has_reply(b"\x1b[?62;1;22"));
        assert!(!has_reply(b"\x1b[?"));
        assert!(!has_reply(b""));
    }

    #[test]
    #[should_panic(expected = "bitmap must match the given dimensions")]
    fn encode_rejects_mismatched_length() {
        let mut out = Vec::new();
        let _ = encode(&[true, false], 2, 2, false, &mut out);
    }
}
