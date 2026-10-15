//! clap [`ValueEnum`] implementations for the domain value types.
//!
//! The types themselves live in [`crate::types`], which must stay free of
//! `clap`; the parser-facing enumeration is layered on here instead. Because
//! `Mode`, `GlyphSet`, and `Ec` are local to this crate, a foreign trait
//! (`clap::ValueEnum`) may be implemented for them from any module. Listing the
//! values here (with per-value help and the short aliases) is what lets `--help`
//! print `[possible values: ...]`.
//!
//! [`ValueEnum`]: clap::ValueEnum

use clap::ValueEnum;
use clap::builder::PossibleValue;

use crate::types::{Ec, GlyphSet, Mode};

impl ValueEnum for Mode {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Mode::Auto,
            Mode::Text,
            Mode::Ansi,
            Mode::Bitmap,
            Mode::Kitty,
            Mode::Sixel,
        ]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(match self {
            Mode::Auto => PossibleValue::new("auto")
                .help("pick the best the terminal offers: a bitmap when supported, else text"),
            Mode::Text => {
                PossibleValue::new("text").help("Unicode block glyphs (honours --glyphs)")
            }
            Mode::Ansi => PossibleValue::new("ansi")
                .help("black-on-white ANSI codes, for terminals without block glyphs"),
            Mode::Bitmap => PossibleValue::new("bitmap")
                .help("force a bitmap, choosing Kitty or Sixel by terminal support"),
            Mode::Kitty => {
                PossibleValue::new("kitty").help("force a bitmap over the Kitty graphics protocol")
            }
            Mode::Sixel => {
                PossibleValue::new("sixel").help("force a bitmap over the Sixel graphics protocol")
            }
        })
    }
}

impl ValueEnum for GlyphSet {
    fn value_variants<'a>() -> &'a [Self] {
        &[GlyphSet::Half, GlyphSet::Quadrant, GlyphSet::Braille]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(match self {
            GlyphSet::Half => PossibleValue::new("half")
                .help("1x2 modules per cell; most robust (h)")
                .alias("h"),
            GlyphSet::Quadrant => PossibleValue::new("quadrant")
                .help("2x2 modules per cell (q)")
                .alias("q"),
            GlyphSet::Braille => PossibleValue::new("braille")
                .help("2x4 modules per cell; densest (b)")
                .alias("b"),
        })
    }
}

impl ValueEnum for Ec {
    fn value_variants<'a>() -> &'a [Self] {
        &[Ec::L, Ec::M, Ec::Q, Ec::H]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(match self {
            Ec::L => PossibleValue::new("L")
                .help("about 7% recoverable")
                .alias("l"),
            Ec::M => PossibleValue::new("M")
                .help("about 15% recoverable")
                .alias("m"),
            Ec::Q => PossibleValue::new("Q")
                .help("about 25% recoverable")
                .alias("q"),
            Ec::H => PossibleValue::new("H")
                .help("about 30% recoverable")
                .alias("h"),
        })
    }
}
