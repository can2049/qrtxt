//! Terminal rendering: draw symbology-agnostic [`Symbol`] grids in a chosen
//! [`RenderMode`] (FR-3.*).
//!
//! [`render_all`] is the single entry point — the renderer dispatch. The actual
//! drawing is split so each strategy is independently testable and free of the
//! others' concerns:
//!
//! - [`frame`] builds the ink grid (quiet zone, inversion, tile padding).
//! - [`text`] packs it into Unicode block glyphs inside a colour context.
//! - [`ansi`] paints the same grid with background-colour escape codes.
//! - [`bitmap`] scales a symbol to a pixel image for the Kitty/Sixel protocols.
//! - [`style`] holds the shared SGR colour sequences.
//!
//! The public items of the submodules are re-exported here, so callers keep
//! using the `render::` path.

use std::io::{self, Write};

use crate::types::{Format, GlyphSet, RenderMode, Symbol};

mod ansi;
mod bitmap;
mod frame;
mod style;
mod text;

pub use ansi::render_ansi;
pub use bitmap::{render_kitty, render_sixel};
pub use frame::{Frame, build_ink};
pub use text::{render, render_braille, render_half, render_on_field, render_quadrant};

/// Drawing parameters shared by every renderer for one run.
#[derive(Clone, Copy, Debug)]
pub struct RenderOptions {
    /// Quiet-zone width, in modules.
    pub border: u32,
    /// Swap the dark and light colours (FR-3.9).
    pub invert: bool,
    /// Glyph set used by the character renderers.
    pub glyphs: GlyphSet,
}

/// First image id handed to the transmitted bitmaps (FR-3.12).
///
/// Each code takes the next id. The Kitty protocol replaces an image stored under
/// a repeated id, so sharing one id would leave only the last code visible.
const KITTY_IMAGE_ID_BASE: u32 = 424_242;

/// Caption prefix for a multi-symbol run: the symbology's full name, so the label
/// reads unambiguously (`QR Code 1/3`, `Data Matrix 1/3`).
fn format_label(format: Format) -> &'static str {
    match format {
        Format::Qr => "QR Code",
        Format::DataMatrix => "Data Matrix",
    }
}

/// Draw every code in `mode`; when there is more than one, caption each and
/// separate them.
///
/// This is the renderer dispatch: the caller hands over the neutral [`Symbol`]
/// grids and the resolved [`RenderMode`], and every renderer-specific detail —
/// the Kitty image-id sequence, the ANSI glyph pinning, the multi-code captions —
/// lives here instead of leaking into [`crate::cli`].
pub fn render_all(
    codes: &[Symbol],
    format: Format,
    mode: RenderMode,
    options: RenderOptions,
    out: &mut dyn Write,
) -> io::Result<()> {
    let RenderOptions {
        border,
        invert,
        glyphs,
    } = options;
    let total = codes.len();
    let label = format_label(format);
    for (index, symbol) in codes.iter().enumerate() {
        if total > 1 {
            writeln!(out, "{label} {}/{}", index + 1, total)?;
        }
        match mode {
            RenderMode::Kitty => {
                let id = KITTY_IMAGE_ID_BASE.wrapping_add(index as u32);
                render_kitty(symbol, border, invert, id, out)?;
            }
            RenderMode::Sixel => {
                render_sixel(symbol, border, invert, out)?;
            }
            RenderMode::Compact => {
                let frame = build_ink(symbol, border, invert, glyphs);
                render_on_field(&frame, glyphs, invert, out)?;
            }
            RenderMode::Ansi => {
                // ANSI draws one module per two spaces and ignores the glyph set,
                // so pin the frame's tile to the smallest one: the output is then
                // independent of `--glyphs` instead of shifting with it.
                let frame = build_ink(symbol, border, invert, GlyphSet::Half);
                render_ansi(&frame, invert, out)?;
            }
        }
        if total > 1 && index + 1 < total {
            out.write_all(b"\n")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Ec, EncodeOptions};

    /// QR encode options at error level `ec`, for the tests.
    fn qr(ec: Ec) -> EncodeOptions {
        EncodeOptions::Qr { ec }
    }

    /// Default drawing options matching the CLI defaults (border 4, no invert,
    /// half blocks).
    fn options() -> RenderOptions {
        RenderOptions {
            border: 4,
            invert: false,
            glyphs: GlyphSet::Half,
        }
    }

    #[test]
    fn render_all_captions_multiple_codes() {
        let codes = crate::encode::encode_multi(&vec![b'x'; 4000], &qr(Ec::L), None, None).unwrap();
        assert!(codes.len() > 1);
        let mut out = Vec::new();
        render_all(&codes, Format::Qr, RenderMode::Compact, options(), &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("QR Code 1/"), "{text}");
        assert!(
            text.contains(&format!("QR Code {0}/{0}", codes.len())),
            "{text}"
        );
    }

    #[test]
    fn render_all_leaves_a_single_code_uncaptioned() {
        let codes = crate::encode::encode_multi(b"hello", &qr(Ec::L), None, None).unwrap();
        assert_eq!(codes.len(), 1);
        let mut out = Vec::new();
        render_all(&codes, Format::Qr, RenderMode::Compact, options(), &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(!text.contains("QR Code "), "{text}");
    }

    #[test]
    fn ansi_output_is_independent_of_the_glyph_set() {
        // Regression: ANSI ignores the glyph set, but the frame was still padded
        // to the chosen glyph's tile, so `--mode ansi -g braille` gained blank
        // rows over `--mode ansi`. The frame now uses the smallest tile in ANSI
        // mode, so they match.
        let half = options();
        let braille = RenderOptions {
            glyphs: GlyphSet::Braille,
            ..options()
        };
        let codes = crate::encode::encode_multi(b"hi", &qr(Ec::L), None, None).unwrap();
        let draw = |opts: RenderOptions| {
            let mut out = Vec::new();
            render_all(&codes, Format::Qr, RenderMode::Ansi, opts, &mut out).unwrap();
            out
        };
        assert_eq!(draw(half), draw(braille));
    }

    #[test]
    fn render_all_kitty_renders_every_code_with_its_own_image_id() {
        // Regression: every code once shared one image id, so the terminal
        // replaced every bitmap but the last and the earlier codes looked
        // unrendered. Each code must now be transmitted as its own image with a
        // distinct id, in order.
        let codes = crate::encode::encode_multi(&vec![b'x'; 6000], &qr(Ec::L), None, None).unwrap();
        assert!(codes.len() >= 3, "payload should split into several codes");

        let mut out = Vec::new();
        render_all(&codes, Format::Qr, RenderMode::Kitty, options(), &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();

        // Exactly one "transmit and display" control block per code: nothing is
        // skipped, and no image is emitted more than once.
        assert_eq!(
            text.matches("a=T,f=24,").count(),
            codes.len(),
            "each code must be transmitted exactly once"
        );

        // Ids are assigned in order, one per code, so an earlier image is never
        // overwritten by a later one.
        let ids: Vec<u32> = text
            .match_indices(",i=")
            .map(|(start, _)| {
                let rest = &text[start + 3..];
                rest[..rest
                    .find(',')
                    .expect("id is followed by another control key")]
                    .parse()
                    .expect("numeric image id")
            })
            .collect();
        let expected: Vec<u32> = (0..codes.len() as u32)
            .map(|index| KITTY_IMAGE_ID_BASE + index)
            .collect();
        assert_eq!(ids, expected, "image ids must be distinct and in order");
    }

    #[test]
    fn render_all_labels_datamatrix_captions() {
        let codes =
            crate::encode::encode_multi(&vec![b'x'; 4000], &EncodeOptions::DataMatrix, None, None)
                .unwrap();
        assert!(codes.len() > 1, "expected a split");
        let mut out = Vec::new();
        render_all(
            &codes,
            Format::DataMatrix,
            RenderMode::Compact,
            options(),
            &mut out,
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("Data Matrix 1/"), "{text}");
        assert!(!text.contains("QR Code "), "{text}");
    }

    #[test]
    fn render_all_sixel_renders_one_sequence_per_code() {
        let codes = crate::encode::encode_multi(&vec![b'x'; 6000], &qr(Ec::L), None, None).unwrap();
        assert!(codes.len() >= 3, "payload should split into several codes");

        let mut out = Vec::new();
        render_all(&codes, Format::Qr, RenderMode::Sixel, options(), &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        // One Sixel sequence (DCS introducer) per code; nothing skipped or doubled.
        assert_eq!(text.matches("\x1bPq").count(), codes.len());
    }

    #[test]
    fn format_label_names_each_symbology() {
        assert_eq!(format_label(Format::Qr), "QR Code");
        assert_eq!(format_label(Format::DataMatrix), "Data Matrix");
    }
}
