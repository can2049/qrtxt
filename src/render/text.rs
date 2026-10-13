//! Pack an ink [`Frame`] into Unicode block glyphs and wrap them in an explicit
//! colour context (FR-3.1, FR-3.8, FR-3.9, FR-3.11).
//!
//! One character cell carries a small tile of modules (`half` 1x2, `quadrant`
//! 2x2, `braille` 2x4); this module maps each tile to its glyph. The colour is
//! fixed here ([`DARK_ON_LIGHT`] by default) so the code is a self-contained
//! image whatever the terminal's own colours are.

use std::io::{self, Write};

use super::frame::Frame;
use super::style::{DARK_ON_LIGHT, LIGHT_ON_DARK, RESET};
use crate::types::GlyphSet;

/// Render a frame using the given glyph set, without any colour control.
///
/// Ink cells become block glyphs and background cells spaces; wrap the result in
/// an explicit colour context — as [`render_on_field`] does — to fix what those
/// two roles look like on the terminal.
pub fn render(frame: &Frame, glyph: GlyphSet, out: &mut dyn Write) -> io::Result<()> {
    for row in 0..text_rows(frame, glyph) {
        let mut line = row_glyphs(frame, glyph, row);
        line.push('\n');
        out.write_all(line.as_bytes())?;
    }
    Ok(())
}

/// Render a frame as block glyphs inside an explicit colour context, so the code
/// is a self-contained dark-on-white image (FR-3.8) rather than one that borrows
/// the terminal's own colours — the quiet zone stays white even on a black
/// terminal. `invert` swaps the two colours (FR-3.9). Prefer this over [`render`]
/// for terminal output.
pub fn render_on_field(
    frame: &Frame,
    glyph: GlyphSet,
    invert: bool,
    out: &mut dyn Write,
) -> io::Result<()> {
    let prologue = if invert { LIGHT_ON_DARK } else { DARK_ON_LIGHT };
    for row in 0..text_rows(frame, glyph) {
        out.write_all(prologue.as_bytes())?;
        out.write_all(row_glyphs(frame, glyph, row).as_bytes())?;
        // Reset before the line feed: a terminal that carries the active colour
        // through the newline would otherwise paint the rest of the row in the
        // field colour, stretching the background across the whole line.
        out.write_all(RESET.as_bytes())?;
        out.write_all(b"\n")?;
    }
    Ok(())
}

/// Number of text rows a frame occupies when packed with `glyph`.
fn text_rows(frame: &Frame, glyph: GlyphSet) -> usize {
    let tile_h = glyph.tile().1;
    debug_assert_eq!(
        frame.height() % tile_h,
        0,
        "build_ink pads to the glyph tile"
    );
    frame.height() / tile_h
}

/// Pack text row `row` of `frame` into glyphs, one character per horizontal cell.
fn row_glyphs(frame: &Frame, glyph: GlyphSet, row: usize) -> String {
    let top = row * glyph.tile().1;
    match glyph {
        // Half blocks: one text row carries a 1x2 tile (FR-3.1).
        GlyphSet::Half => {
            let mut line = String::with_capacity(frame.width());
            for x in 0..frame.width() {
                line.push(match (frame.at(x, top), frame.at(x, top + 1)) {
                    (true, true) => '\u{2588}',  // full block
                    (true, false) => '\u{2580}', // upper half
                    (false, true) => '\u{2584}', // lower half
                    (false, false) => ' ',
                });
            }
            line
        }
        // Quadrant blocks: one text row carries a 2x2 tile (FR-3.11).
        GlyphSet::Quadrant => {
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
            line
        }
        // Braille dots: one text row carries a 2x4 tile (FR-3.11).
        GlyphSet::Braille => {
            let mut line = String::with_capacity(frame.width() / 2);
            for left in (0..frame.width()).step_by(2) {
                let mut bits = 0u32;
                for (column, weights) in BRAILLE_WEIGHTS.iter().enumerate() {
                    for (dot_row, weight) in weights.iter().enumerate() {
                        if frame.at(left + column, top + dot_row) {
                            bits += weight;
                        }
                    }
                }
                line.push(char::from_u32(0x2800 + bits).expect("valid braille code point"));
            }
            line
        }
    }
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

/// Half blocks: one text row carries a 1x2 tile (FR-3.1).
pub fn render_half(frame: &Frame, out: &mut dyn Write) -> io::Result<()> {
    render(frame, GlyphSet::Half, out)
}

/// Quadrant blocks: one text row carries a 2x2 tile (FR-3.11).
pub fn render_quadrant(frame: &Frame, out: &mut dyn Write) -> io::Result<()> {
    render(frame, GlyphSet::Quadrant, out)
}

/// Braille dots: one text row carries a 2x4 tile (FR-3.11).
pub fn render_braille(frame: &Frame, out: &mut dyn Write) -> io::Result<()> {
    render(frame, GlyphSet::Braille, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2x2 ink grid: top-left and bottom-right are ink.
    fn sample() -> Frame {
        Frame::new(2, 2, vec![true, false, false, true])
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
    fn render_on_field_wraps_the_glyphs_in_colours() {
        let mut buffer = Vec::new();
        render_on_field(&sample(), GlyphSet::Half, false, &mut buffer).unwrap();
        assert_eq!(
            String::from_utf8(buffer).unwrap(),
            format!("{DARK_ON_LIGHT}\u{2580}\u{2584}{RESET}\n")
        );

        let mut inverted = Vec::new();
        render_on_field(&sample(), GlyphSet::Half, true, &mut inverted).unwrap();
        assert!(
            String::from_utf8(inverted)
                .unwrap()
                .starts_with(LIGHT_ON_DARK)
        );
    }
}
