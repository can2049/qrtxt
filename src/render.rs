//! Turn a QR matrix into an "ink" grid, then into terminal glyphs (FR-3.*).
//!
//! Rendering is split into two independently testable steps:
//! 1. [`build_ink`] turns the QR matrix into a [`Frame`] (a boolean ink grid),
//!    applying quiet zone, size and inversion.
//! 2. [`render`] packs the grid into character cells per glyph set.

use std::io::{self, Write};

use crate::types::{GlyphSet, Symbol};

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

/// Build the ink grid: quiet zone (background), optional inversion, padded to a
/// whole number of glyph tiles.
#[must_use]
pub fn build_ink(symbol: &Symbol, border: u32, invert: bool, glyph: GlyphSet) -> Frame {
    let src_w = symbol.width();
    let src_h = symbol.height();
    let b = border as usize;
    let (tile_w, tile_h) = glyph.tile();
    let width = round_up(src_w + 2 * b, tile_w);
    let height = round_up(src_h + 2 * b, tile_h);

    let mut ink = vec![false; width * height];
    for y in 0..height {
        let row_in_matrix = y >= b && y < b + src_h;
        for x in 0..width {
            let col_in_matrix = x >= b && x < b + src_w;
            ink[y * width + x] = if row_in_matrix && col_in_matrix {
                // Default: light modules are ink (FR-3.8); `invert` flips it (FR-3.9).
                let light = !symbol.is_dark(x - b, y - b);
                light ^ invert
            } else {
                false // quiet zone and tile padding stay background
            };
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

/// Target rendered size, in pixels; the per-module scale is derived from it.
const TARGET_PX: usize = 300;
/// Bounds on the pixels-per-module scale.
const MIN_MODULE_PX: usize = 3;
const MAX_MODULE_PX: usize = 8;

/// A symbol scaled to a pixel bitmap: `width_px` x `height_px` pixels, with
/// `true` marking a dark pixel.
struct Bitmap {
    width_px: usize,
    height_px: usize,
    dark: Vec<bool>,
}

/// Scale `symbol`, plus its quiet zone, to a pixel bitmap — one crisp block of
/// pixels per module. The module size is chosen from the longer side, so square
/// and rectangular symbols both keep square modules.
fn module_bitmap(symbol: &Symbol, border: u32) -> Bitmap {
    let src_w = symbol.width();
    let src_h = symbol.height();
    let b = border as usize;
    let side_w = src_w + 2 * b;
    let side_h = src_h + 2 * b;
    let module_px = (TARGET_PX / side_w.max(side_h)).clamp(MIN_MODULE_PX, MAX_MODULE_PX);
    let width_px = side_w * module_px;
    let height_px = side_h * module_px;

    let mut dark = vec![false; width_px * height_px];
    for y in 0..height_px {
        let module_y = y / module_px;
        for x in 0..width_px {
            let module_x = x / module_px;
            dark[y * width_px + x] = module_x >= b
                && module_x < b + src_w
                && module_y >= b
                && module_y < b + src_h
                && symbol.is_dark(module_x - b, module_y - b);
        }
    }
    Bitmap {
        width_px,
        height_px,
        dark,
    }
}

/// Render `symbol` as a black-and-white bitmap through the Kitty graphics
/// protocol (FR-3.12).
///
/// An RGB image is scaled so every module is a crisp block of pixels, with the
/// quiet zone drawn in the light colour. `invert` swaps the two colours.
///
/// `id` is the protocol image id. Each transmission must use a fresh id: with
/// `a=T` the terminal replaces any image already stored under the same id and
/// drops its placements, so reusing one id would erase earlier codes instead of
/// showing them side by side.
pub fn render_kitty(
    symbol: &Symbol,
    border: u32,
    invert: bool,
    id: u32,
    out: &mut dyn Write,
) -> io::Result<()> {
    let bitmap = module_bitmap(symbol, border);

    // Default: dark modules are black on a white field (FR-3.8); `invert` swaps
    // the two colours (FR-3.9).
    let (dark, light) = if invert {
        ([255, 255, 255], [0, 0, 0])
    } else {
        ([0, 0, 0], [255, 255, 255])
    };

    let mut rgb = Vec::with_capacity(bitmap.dark.len() * 3);
    for is_dark in &bitmap.dark {
        let colour = if *is_dark { dark } else { light };
        rgb.extend_from_slice(&colour);
    }

    crate::kitty::transmit_rgb(&rgb, bitmap.width_px, bitmap.height_px, id, out)?;
    out.write_all(b"\n")
}

/// Render `symbol` as a black-and-white bitmap through the Sixel graphics
/// protocol (FR-3.13).
///
/// Works like [`render_kitty`] — each module is a crisp block of pixels on a light
/// quiet zone — but emits a Sixel sequence instead of a Kitty one, and needs no
/// image id: Sixel draws at the cursor the moment the sequence is written.
/// `invert` swaps the two colours (FR-3.9).
pub fn render_sixel(
    symbol: &Symbol,
    border: u32,
    invert: bool,
    out: &mut dyn Write,
) -> io::Result<()> {
    let bitmap = module_bitmap(symbol, border);
    crate::sixel::encode(&bitmap.dark, bitmap.width_px, bitmap.height_px, invert, out)?;
    out.write_all(b"\n")
}

/// Decode a rendered Kitty sequence's first chunk payload (test helper).
#[cfg(test)]
fn first_payload(text: &str) -> &str {
    let start = text.find(';').expect("control separator") + 1;
    let end = start + text[start..].find('\x1b').expect("payload terminator");
    &text[start..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2x2 ink grid: top-left and bottom-right are ink.
    fn sample() -> Frame {
        Frame::new(2, 2, vec![true, false, false, true])
    }

    /// An all-light square symbol (`size` x `size` modules), enough to exercise
    /// bitmap geometry without depending on a specific symbology.
    fn blank_symbol(size: usize) -> Symbol {
        Symbol::new(size, size, vec![false; size * size])
    }

    #[test]
    fn kitty_bitmap_is_square_with_light_quiet_zone() {
        let symbol = blank_symbol(21);
        let side = symbol.width() + 2 * 4;
        let module_px = (TARGET_PX / side).clamp(MIN_MODULE_PX, MAX_MODULE_PX);
        let image_px = side * module_px;

        let mut out = Vec::new();
        render_kitty(&symbol, 4, false, 424_242, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("\x1b_G"), "missing APC introducer");
        // The image spans many chunks, so the first chunk carries the full control
        // block and sets `m=1` (more to come).
        assert!(
            text.contains(&format!("a=T,f=24,s={image_px},v={image_px},i=424242,m=1;")),
            "missing or incorrect first-chunk control block"
        );
        // The top-left pixel is the quiet zone, drawn light (white) by default.
        assert!(
            first_payload(&text).starts_with("////"),
            "quiet zone is not white"
        );
    }

    #[test]
    fn kitty_invert_swaps_the_quiet_zone_pixel() {
        let symbol = blank_symbol(21);
        let mut out = Vec::new();
        render_kitty(&symbol, 4, true, 7, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        // Inverted: the light quiet zone becomes black.
        assert!(
            first_payload(&text).starts_with("AAAA"),
            "quiet zone is not black"
        );
    }

    #[test]
    fn sixel_bitmap_declares_a_square_raster_and_light_quiet_zone() {
        let symbol = blank_symbol(21);
        let side = symbol.width() + 2 * 4;
        let module_px = (TARGET_PX / side).clamp(MIN_MODULE_PX, MAX_MODULE_PX);
        let image_px = side * module_px;

        let mut out = Vec::new();
        render_sixel(&symbol, 4, false, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("\x1bPq"), "missing DCS introducer");
        assert!(text.ends_with("\x1b\\\n"), "missing string terminator");
        assert!(
            text.contains(&format!("\"1;1;{image_px};{image_px}")),
            "missing or incorrect raster attributes"
        );
        // Register 0 (quiet zone) is white, register 1 black.
        assert!(text.contains("#0;2;100;100;100#1;2;0;0;0"), "{text:?}");
    }

    #[test]
    fn sixel_invert_swaps_the_quiet_zone_colour() {
        let symbol = blank_symbol(21);
        let mut out = Vec::new();
        render_sixel(&symbol, 4, true, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("#0;2;0;0;0#1;2;100;100;100"), "{text:?}");
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
