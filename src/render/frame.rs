//! The ink grid: a [`Symbol`] turned into a boolean "ink" field with the quiet
//! zone, inversion and tile padding applied (FR-3.8, FR-3.9, FR-3.11).
//!
//! This is the geometry half of rendering. It is glyph-agnostic apart from
//! padding the grid to a whole number of glyph tiles, so the character
//! renderers in [`crate::render::text`] and [`crate::render::ansi`] can pack it
//! without knowing how it was built.

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
    pub(super) fn at(&self, x: usize, y: usize) -> bool {
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
///
/// Ink marks the dark modules — the cells a renderer draws in the foreground —
/// while the field (the light modules and the quiet zone) stays background, so
/// dark modules read as black on a white field (FR-3.8). `invert` swaps the two
/// (FR-3.9).
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
                // Default: dark modules are ink (FR-3.8); `invert` flips it (FR-3.9).
                symbol.is_dark(x - b, y - b) ^ invert
            } else {
                false // quiet zone and tile padding stay background
            };
        }
    }
    Frame::new(width, height, ink)
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
    fn frame_reports_its_geometry() {
        let frame = sample();
        assert_eq!((frame.width(), frame.height()), (2, 2));
        assert!(frame.at(0, 0));
        assert!(!frame.at(1, 0));
        assert!(!frame.at(0, 1));
        assert!(frame.at(1, 1));
    }

    #[test]
    fn build_ink_marks_dark_modules_and_leaves_the_quiet_zone_as_field() {
        // A 2x2 all-dark symbol with a 1-module border: the four quiet-zone cells
        // stay background (field) and the four matrix cells are ink.
        let symbol = Symbol::new(2, 2, vec![true, true, true, true]);
        let frame = build_ink(&symbol, 1, false, GlyphSet::Half);
        assert_eq!((frame.width(), frame.height()), (4, 4));
        for y in 0..4 {
            for x in 0..4 {
                let in_matrix = (1..3).contains(&x) && (1..3).contains(&y);
                assert_eq!(frame.at(x, y), in_matrix, "at ({x},{y})");
            }
        }
    }

    #[test]
    fn build_ink_invert_flips_the_module_ink() {
        let symbol = Symbol::new(2, 2, vec![true, true, true, true]);
        let frame = build_ink(&symbol, 1, true, GlyphSet::Half);
        // Inverted: the all-dark matrix becomes background, so only field shows.
        assert!(!frame.at(1, 1));
    }
}
