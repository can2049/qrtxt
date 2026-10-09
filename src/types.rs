//! Shared value types used across the crate.
//!
//! These live outside [`crate::cli`] and are free of `clap`, so the domain
//! modules (`input` / `encode` / `render`) never depend on the argument parser.
//! `cli` reuses them through their [`FromStr`] implementations.

use std::path::PathBuf;
use std::str::FromStr;

/// Exact QR error-correction level (FR-2.3).
///
/// A higher level tolerates more damage but reduces the data capacity, which can
/// bump the symbol to a larger version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ec {
    /// Recovers up to ~7% of the symbol (the default).
    L,
    /// Recovers up to ~15% of the symbol.
    M,
    /// Recovers up to ~25% of the symbol.
    Q,
    /// Recovers up to ~30% of the symbol.
    H,
}

impl FromStr for Ec {
    type Err = String;

    /// Parses a case-insensitive level name (`"L"`, `"m"`, ...).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "L" => Ok(Ec::L),
            "M" => Ok(Ec::M),
            "Q" => Ok(Ec::Q),
            "H" => Ok(Ec::H),
            _ => Err(format!("invalid error level: {s} (expected L|M|Q|H)")),
        }
    }
}

/// How many QR modules are packed into a single terminal character cell (FR-3.11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlyphSet {
    /// Half blocks: `1x2` modules per cell. The most robust, and the default.
    Half,
    /// Quadrant blocks: `2x2` modules per cell.
    Quadrant,
    /// Braille dots: `2x4` modules per cell. The densest, but font-dependent.
    Braille,
}

impl FromStr for GlyphSet {
    type Err = String;

    /// Parses a case-insensitive glyph-set name (`"half"`, `"Braille"`, `"b"`, ...).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "half" | "h" => Ok(GlyphSet::Half),
            "quadrant" | "q" => Ok(GlyphSet::Quadrant),
            "braille" | "b" => Ok(GlyphSet::Braille),
            _ => Err(format!(
                "invalid glyph set: {s} (expected half|quadrant|braille, or h|q|b)"
            )),
        }
    }
}

/// How the QR is drawn on the terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderMode {
    /// Unicode block/glyph rendering (the default).
    Compact,
    /// ANSI escape-code rendering.
    Ansi,
    /// Bitmap rendering via the Kitty graphics protocol (FR-3.12).
    Kitty,
}

/// Where the payload is read from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputSpec {
    /// A literal positional argument, encoded verbatim.
    Literal(String),
    /// A file, read as raw bytes with one trailing newline stripped.
    File(PathBuf),
    /// Standard input, read as raw bytes with one trailing newline stripped.
    Stdin,
}

/// Normalized run configuration, decoupled from `clap`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Where the payload comes from.
    pub input: InputSpec,
    /// Requested error-correction level.
    pub ec: Ec,
    /// Quiet-zone width, in modules.
    pub border: u32,
    /// Invert the ink mapping, for light-background terminals.
    pub invert: bool,
    /// Glyph set used to draw the symbol.
    pub glyphs: GlyphSet,
    /// Terminal rendering mode.
    pub mode: RenderMode,
    /// Keep one trailing newline instead of stripping it.
    pub preserve_newline: bool,
    /// Cap on the payload bytes per QR code; `None` uses the symbol's own limit.
    pub max_size: Option<usize>,
    /// Advisory floor on the number of QR codes to split across; `None` means no
    /// floor beyond what the byte cap requires.
    pub min_chunks: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ec_parses_case_insensitively() {
        assert_eq!("l".parse::<Ec>(), Ok(Ec::L));
        assert_eq!("H".parse::<Ec>(), Ok(Ec::H));
        assert!("Z".parse::<Ec>().is_err());
    }

    #[test]
    fn glyph_set_parses_case_insensitively() {
        assert_eq!("HALF".parse::<GlyphSet>(), Ok(GlyphSet::Half));
        assert_eq!("Braille".parse::<GlyphSet>(), Ok(GlyphSet::Braille));
        assert!("dense".parse::<GlyphSet>().is_err());
    }

    #[test]
    fn glyph_set_accepts_initial_letters() {
        assert_eq!("h".parse::<GlyphSet>(), Ok(GlyphSet::Half));
        assert_eq!("Q".parse::<GlyphSet>(), Ok(GlyphSet::Quadrant));
        assert_eq!("b".parse::<GlyphSet>(), Ok(GlyphSet::Braille));
    }
}
