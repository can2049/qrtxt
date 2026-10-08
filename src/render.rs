//! Turn a QR matrix into an "ink" grid, then into terminal glyphs (FR-3.*).
//!
//! Rendering is split into two independently testable steps:
//! 1. [`build_ink`] turns the QR matrix into a [`Frame`] (a boolean ink grid),
//!    applying quiet zone, scale and inversion.
//! 2. [`render`] packs the grid into character cells per glyph set.

use std::io::{self, Write};

use qrcode::{Color, QrCode};

use crate::types::GlyphSet;

/// A boolean "ink" grid; `true` means "draw as ink" (foreground).
#[derive(Debug)]
pub struct Frame {
    width: usize,
    height: usize,
    ink: Vec<bool>,
}

impl Frame {
    /// Build a frame, enforcing `ink.len() == width * height`.
    ///
    /// # Panics
    /// Panics if `ink.len() != width * height`.
    #[must_use]
    pub fn new(width: usize, height: usize, ink: Vec<bool>) -> Self {
        assert_eq!(
            ink.len(),
            width * height,
            "ink length must equal width * height"
        );
        Frame { width, height, ink }
    }

    /// Frame width, in modules.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Frame height, in modules.
    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    #[inline]
    fn at(&self, x: usize, y: usize) -> bool {
        self.ink[y * self.width + x]
    }
}

impl GlyphSet {
    /// Modules packed into one character cell: `(width, height)`.
    #[must_use]
    pub fn tile(self) -> (usize, usize) {
        match self {
            GlyphSet::Half => (1, 2),
            GlyphSet::Quadrant => (2, 2),
            GlyphSet::Braille => (2, 4),
        }
    }
}

fn round_up(value: usize, multiple: usize) -> usize {
    value.div_ceil(multiple) * multiple
}

/// Build the ink grid: quiet zone (background), optional scale and inversion,
/// padded to a whole number of glyph tiles.
#[must_use]
pub fn build_ink(qr: &QrCode, border: u32, scale: u32, invert: bool, glyph: GlyphSet) -> Frame {
    let src = qr.width();
    let colors = qr.to_colors();
    let b = border as usize;
    let s = scale.max(1) as usize;
    let (tile_w, tile_h) = glyph.tile();
    let inner_w = round_up(src + 2 * b, tile_w);
    let inner_h = round_up(src + 2 * b, tile_h);
    let width = inner_w * s;
    let height = inner_h * s;

    // Default: light modules are ink (FR-3.8); `invert` flips it (FR-3.9).
    let is_ink = |color: Color| -> bool {
        let light = matches!(color, Color::Light);
        light ^ invert
    };

    let mut ink = vec![false; width * height];
    for sy in 0..inner_h {
        let row_in_matrix = sy >= b && sy < b + src;
        for sx in 0..inner_w {
            let value = if row_in_matrix && sx >= b && sx < b + src {
                is_ink(colors[(sy - b) * src + (sx - b)])
            } else {
                false // quiet zone and tile padding stay background
            };
            for dy in 0..s {
                let row = (sy * s + dy) * width;
                for dx in 0..s {
                    ink[row + sx * s + dx] = value;
                }
            }
        }
    }
    Frame::new(width, height, ink)
}

/// Render a frame using the given glyph set.
pub fn render(frame: &Frame, glyph: GlyphSet, out: &mut dyn Write) -> io::Result<()> {
    match glyph {
        GlyphSet::Half => render_half(frame, out),
        GlyphSet::Quadrant => render_quadrant(frame, out),
        GlyphSet::Braille => render_braille(frame, out),
    }
}

/// Half blocks: one text row carries a 1x2 tile (FR-3.1).
pub fn render_half(frame: &Frame, out: &mut dyn Write) -> io::Result<()> {
    debug_assert!(frame.height() % 2 == 0, "build_ink pads to the glyph tile");
    let mut y = 0;
    while y < frame.height() {
        let mut line = String::with_capacity(frame.width());
        for x in 0..frame.width() {
            line.push(match (frame.at(x, y), frame.at(x, y + 1)) {
                (true, true) => '\u{2588}',  // full block
                (true, false) => '\u{2580}', // upper half
                (false, true) => '\u{2584}', // lower half
                (false, false) => ' ',
            });
        }
        line.push('\n');
        out.write_all(line.as_bytes())?;
        y += 2;
    }
    Ok(())
}

/// 2x2 quadrant blocks: 16 combinations -> 16 characters.
const QUADRANT: [char; 16] = [
    ' ', '\u{2598}', '\u{259D}', '\u{2580}', // tl, tr, top
    '\u{2596}', '\u{258C}', '\u{259E}', '\u{259B}', // bl, left, tr+bl, tl+tr+bl
    '\u{2597}', '\u{259A}', '\u{2590}', '\u{259C}', // br, tl+br, right, tl+tr+br
    '\u{2584}', '\u{2599}', '\u{259F}', '\u{2588}', // bottom, tl+bl+br, tr+bl+br, full
];

/// Braille dot weights: `[column][row]`; code point = `U+2800 + sum` (FR-3.11).
const BRAILLE_WEIGHTS: [[u32; 4]; 2] = [
    [0x01, 0x02, 0x04, 0x40], // dots 1, 2, 3, 7
    [0x08, 0x10, 0x20, 0x80], // dots 4, 5, 6, 8
];

/// Quadrant blocks: one text row carries a 2x2 tile (FR-3.11).
pub fn render_quadrant(frame: &Frame, out: &mut dyn Write) -> io::Result<()> {
    for top in (0..frame.height()).step_by(2) {
        let mut line = String::with_capacity(frame.width() / 2);
        for left in (0..frame.width()).step_by(2) {
            let mut index = 0usize;
            if frame.at(left, top) {
                index |= 1;
            }
            if frame.at(left + 1, top) {
                index |= 2;
            }
            if frame.at(left, top + 1) {
                index |= 4;
            }
            if frame.at(left + 1, top + 1) {
                index |= 8;
            }
            line.push(QUADRANT[index]);
        }
        line.push('\n');
        out.write_all(line.as_bytes())?;
    }
    Ok(())
}

/// Braille dots: one text row carries a 2x4 tile (FR-3.11).
pub fn render_braille(frame: &Frame, out: &mut dyn Write) -> io::Result<()> {
    for top in (0..frame.height()).step_by(4) {
        let mut line = String::with_capacity(frame.width() / 2);
        for left in (0..frame.width()).step_by(2) {
            let mut bits = 0u32;
            for (column, weights) in BRAILLE_WEIGHTS.iter().enumerate() {
                for (row, weight) in weights.iter().enumerate() {
                    if frame.at(left + column, top + row) {
                        bits += weight;
                    }
                }
            }
            line.push(char::from_u32(0x2800 + bits).expect("valid braille code point"));
        }
        line.push('\n');
        out.write_all(line.as_bytes())?;
    }
    Ok(())
}

/// ANSI rendering: two spaces per module, reverse-video for ink.
///
/// Like the block renderers, this relies on the terminal's foreground color
/// (see the requirements note on dark/light themes); `--invert` handles the
/// light-background case. Prefer [`render`].
pub fn render_ansi(frame: &Frame, out: &mut dyn Write) -> io::Result<()> {
    const INK: &str = "\x1b[7m";
    const BACKGROUND: &str = "\x1b[49m";
    const RESET: &str = "\x1b[0m";
    for y in 0..frame.height() {
        let mut line = String::with_capacity(frame.width() * 5);
        let mut previous: Option<bool> = None;
        for x in 0..frame.width() {
            let ink = frame.at(x, y);
            if previous != Some(ink) {
                line.push_str(if ink { INK } else { BACKGROUND });
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
    #[should_panic(expected = "ink length must equal width * height")]
    fn frame_new_rejects_mismatched_length() {
        let _ = Frame::new(2, 2, vec![true, false]);
    }

    #[test]
    fn tiles_are_as_documented() {
        assert_eq!(GlyphSet::Half.tile(), (1, 2));
        assert_eq!(GlyphSet::Quadrant.tile(), (2, 2));
        assert_eq!(GlyphSet::Braille.tile(), (2, 4));
    }

    #[test]
    fn half_glyphs() {
        let mut buffer = Vec::new();
        render_half(&sample(), &mut buffer).unwrap();
        assert_eq!(String::from_utf8(buffer).unwrap(), "\u{2580}\u{2584}\n");
    }

    #[test]
    fn quadrant_glyphs() {
        let mut buffer = Vec::new();
        render_quadrant(&sample(), &mut buffer).unwrap();
        // tl + br => U+259A
        assert_eq!(String::from_utf8(buffer).unwrap(), "\u{259A}\n");
    }

    #[test]
    fn braille_glyphs() {
        let frame = Frame::new(
            2,
            4,
            vec![
                true, false, //
                false, false, //
                false, false, //
                false, true, //
            ],
        );
        let mut buffer = Vec::new();
        render_braille(&frame, &mut buffer).unwrap();
        // dot 1 (0x01) + dot 8 (0x80) = 0x81 -> U+2881
        assert_eq!(String::from_utf8(buffer).unwrap(), "\u{2881}\n");
    }

    #[test]
    fn ansi_wraps_each_line_and_resets() {
        let mut buffer = Vec::new();
        render_ansi(&sample(), &mut buffer).unwrap();
        let text = String::from_utf8(buffer).unwrap();
        assert!(text.contains("\x1b[7m"));
        assert!(text.contains("\x1b[49m"));
        assert!(text.ends_with("\x1b[0m\n"));
    }
}
