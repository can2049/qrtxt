//! Bitmap rendering: scale a [`Symbol`] to a crisp pixel image and transmit it
//! over the Kitty (FR-3.12) or Sixel (FR-3.13) graphics protocol.
//!
//! Unlike the character renderers, these bypass the ink [`Frame`]:
//! [`module_bitmap`] turns the symbol straight into a black-and-white pixel
//! bitmap (one solid block of pixels per module, quiet zone in the light colour)
//! and the two `render_*` functions hand it to the matching protocol helper.
//!
//! [`Frame`]: crate::render::Frame

use std::io::{self, Write};

use crate::types::Symbol;

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

    /// An all-light square symbol (`size` x `size` modules), enough to exercise
    /// bitmap geometry without depending on a specific symbology.
    fn blank_symbol(size: usize) -> Symbol {
        Symbol::new(size, size, vec![false; size * size])
    }

    /// The pixel side length `blank_symbol(21)` is rendered at, with border 4.
    fn side_px() -> usize {
        let side = 21 + 2 * 4;
        let module_px = (TARGET_PX / side).clamp(MIN_MODULE_PX, MAX_MODULE_PX);
        side * module_px
    }

    #[test]
    fn kitty_bitmap_is_square_with_light_quiet_zone() {
        let symbol = blank_symbol(21);
        let image_px = side_px();

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
        let image_px = side_px();

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
    fn module_bitmap_keeps_rectangular_modules_square() {
        // A wide, short symbol (as PDF417 produces) must scale to square modules
        // with its quiet zone, not be squashed into a square image.
        let symbol = Symbol::new(30, 4, vec![true; 30 * 4]);
        let bitmap = module_bitmap(&symbol, 2);
        let (side_w, side_h) = (30 + 2 * 2, 4 + 2 * 2);
        let module_px = bitmap.width_px / side_w;
        assert!((MIN_MODULE_PX..=MAX_MODULE_PX).contains(&module_px));
        assert_eq!(bitmap.width_px, side_w * module_px);
        assert_eq!(bitmap.height_px, side_h * module_px);
        // Only the symbol interior is dark: 30 * 4 modules, each `module_px^2`.
        let dark = bitmap.dark.iter().filter(|dark| **dark).count();
        assert_eq!(dark, 30 * 4 * module_px * module_px);
    }
}
