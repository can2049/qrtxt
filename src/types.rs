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

/// PDF417 error-correction level (FR-2.7).
///
/// A level from 0 to 8: each step roughly doubles the correction codewords, so a
/// higher level recovers from more damage but holds less data. PDF417 numbers its
/// levels directly (unlike QR's L/M/Q/H letters), so the plain digits are exposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pdf417Ec(pub(crate) u8);

impl Pdf417Ec {
    /// The level for `digit`, or `None` when it is outside `0..=8`.
    ///
    /// The command line validates the range too; this is the checked constructor
    /// for callers that build a level in code.
    #[must_use]
    pub const fn from_digit(digit: u8) -> Option<Self> {
        if digit <= 8 {
            Some(Pdf417Ec(digit))
        } else {
            None
        }
    }

    /// The numeric level, `0`-`8`.
    #[must_use]
    pub fn get(self) -> u8 {
        self.0
    }
}

impl Default for Pdf417Ec {
    /// Level 2, the standard PDF417 default.
    fn default() -> Self {
        Pdf417Ec(2)
    }
}

impl FromStr for Pdf417Ec {
    type Err = String;

    /// Parses a single digit `0`-`8`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<u8>() {
            Ok(level @ 0..=8) => Ok(Pdf417Ec(level)),
            _ => Err(format!(
                "invalid error-correction level: {s} (expected a digit 0-8)"
            )),
        }
    }
}

/// Aztec error-correction percentage (FR-2.8).
///
/// Aztec expresses its redundancy as a percentage of the data rather than a fixed
/// set of levels: higher values recover from more damage but hold less data.
/// ISO/IEC 24778 recommends at least 23% plus three codewords, and 33% is the
/// usual default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AztecEc(pub(crate) u8);

impl AztecEc {
    /// The level for `percent`, or `None` when it is outside `0..=100`.
    ///
    /// The command line validates the range too; this is the checked constructor
    /// for callers that build a level in code.
    #[must_use]
    pub const fn from_percent(percent: u8) -> Option<Self> {
        if percent <= 100 {
            Some(AztecEc(percent))
        } else {
            None
        }
    }

    /// The percentage, `0`-`100`.
    #[must_use]
    pub fn get(self) -> u8 {
        self.0
    }
}

impl Default for AztecEc {
    /// 33%, the usual Aztec default.
    fn default() -> Self {
        AztecEc(33)
    }
}

impl FromStr for AztecEc {
    type Err = String;

    /// Parses a percentage from `0` to `100`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<u8>() {
            Ok(percent @ 0..=100) => Ok(AztecEc(percent)),
            _ => Err(format!(
                "invalid error-correction percentage: {s} (expected 0-100)"
            )),
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
    /// Bitmap rendering via the Sixel graphics protocol (FR-3.13).
    Sixel,
}

/// Render mode chosen on the command line (FR-3.10).
///
/// The user-facing counterpart of [`RenderMode`]: it adds [`Mode::Auto`], a
/// policy resolved against the terminal's graphics capabilities in
/// [`crate::cli::run_with`]. The concrete [`RenderMode`] it maps to never
/// carries an "auto" state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Use a bitmap when the terminal supports one, else text (the default).
    Auto,
    /// Unicode block/glyph text rendering.
    Text,
    /// ANSI escape-code text rendering.
    Ansi,
    /// Force a bitmap, picking Kitty or Sixel by what the terminal supports.
    Bitmap,
    /// Bitmap via the Kitty graphics protocol.
    Kitty,
    /// Bitmap via the Sixel graphics protocol.
    Sixel,
}

impl FromStr for Mode {
    type Err = String;

    /// Parses a case-insensitive mode name (`"auto"`, `"Text"`, ...).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "auto" => Ok(Mode::Auto),
            "text" => Ok(Mode::Text),
            "ansi" => Ok(Mode::Ansi),
            "bitmap" => Ok(Mode::Bitmap),
            "kitty" => Ok(Mode::Kitty),
            "sixel" => Ok(Mode::Sixel),
            _ => Err(format!(
                "invalid mode: {s} (expected auto|text|ansi|bitmap|kitty|sixel)"
            )),
        }
    }
}

/// The barcode symbology to draw (FR-2.7).
///
/// Chosen by the command-line subcommand (`qr` / `datamatrix` / `pdf417` /
/// `aztec`) and independent of [`Mode`]: `--mode` picks how a symbol is rendered,
/// the subcommand picks which symbol is produced. Kept as a value so renderers can
/// caption multi-symbol runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// An ordinary QR code.
    Qr,
    /// A Data Matrix (ECC 200) symbol.
    DataMatrix,
    /// A PDF417 stacked linear symbol.
    Pdf417,
    /// An Aztec Code symbol.
    Aztec,
}

/// Symbol-encoding parameters, one variant per [`Format`] (FR-2.7).
///
/// Each symbology owns its own options: a QR code takes an error-correction
/// level from its four letters, a PDF417 takes one from its nine numeric levels,
/// while a Data Matrix has none (its correction is fixed by the symbol size).
/// Keeping them here, rather than as flat fields on [`Config`], lets a format
/// carry parameters the others do not understand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeOptions {
    /// QR code parameters.
    Qr {
        /// Requested error-correction level.
        ec: Ec,
    },
    /// Data Matrix (ECC 200) parameters: none are user-selectable.
    DataMatrix,
    /// PDF417 parameters.
    Pdf417 {
        /// Requested error-correction level.
        ec: Pdf417Ec,
        /// Forced data-column count, or `None` to let the encoder pick a balanced
        /// shape. Fewer columns make a narrower, taller symbol. Values outside
        /// PDF417's `1..=30` range are clamped into it, so the command line's
        /// `--columns` and direct callers both stay safe.
        columns: Option<u32>,
    },
    /// Aztec Code parameters.
    Aztec {
        /// Requested error-correction percentage (`0`-`100`).
        ec: AztecEc,
        /// Forced layer count, or `None` to use the smallest symbol that fits. A
        /// positive value selects full-range layers, a negative value selects
        /// compact layers (e.g. `-3` is a three-layer compact symbol). Values
        /// outside Aztec's `-4..=32` range are clamped, so the command line's
        /// `--layers` and direct callers both stay safe.
        layers: Option<i8>,
    },
}

impl EncodeOptions {
    /// The [`Format`] these options encode.
    #[must_use]
    pub fn format(&self) -> Format {
        match self {
            EncodeOptions::Qr { .. } => Format::Qr,
            EncodeOptions::DataMatrix => Format::DataMatrix,
            EncodeOptions::Pdf417 { .. } => Format::Pdf417,
            EncodeOptions::Aztec { .. } => Format::Aztec,
        }
    }
}

/// A decoded 2D symbol as a grid of dark/light modules (FR-2.4).
///
/// This is the neutral currency between encoding and rendering: an encoder turns
/// a payload into a [`Symbol`], and every renderer draws a [`Symbol`] without
/// knowing which symbology (QR, Data Matrix, ...) produced it. `true` marks a
/// dark module, matching the symbologies' own convention (a light module is
/// background).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    width: usize,
    height: usize,
    dark: Vec<bool>,
}

impl Symbol {
    /// Build a symbol from a row-major `dark` grid of `width` x `height` modules.
    ///
    /// # Panics
    /// Panics if `dark.len() != width * height`.
    #[must_use]
    pub fn new(width: usize, height: usize, dark: Vec<bool>) -> Self {
        assert_eq!(
            dark.len(),
            width * height,
            "dark length must equal width * height"
        );
        Symbol {
            width,
            height,
            dark,
        }
    }

    /// Symbol width, in modules.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Symbol height, in modules.
    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// Whether the module at column `x`, row `y` is dark.
    #[must_use]
    pub fn is_dark(&self, x: usize, y: usize) -> bool {
        self.dark[y * self.width + x]
    }
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
    /// Symbol-encoding parameters, chosen by the format.
    pub encode: EncodeOptions,
    /// Quiet-zone width, in modules.
    pub border: u32,
    /// Swap the colors: draw light-on-dark instead of dark-on-light.
    pub invert: bool,
    /// Glyph set used to draw the symbol.
    pub glyphs: GlyphSet,
    /// Terminal rendering mode.
    pub mode: RenderMode,
    /// Keep one trailing newline instead of stripping it.
    pub preserve_newline: bool,
    /// Cap on the payload bytes per symbol; `None` uses the symbol's own limit.
    pub max_bytes: Option<usize>,
    /// Advisory floor on the number of symbols to split across; `None` means no
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

    #[test]
    fn mode_parses_case_insensitively() {
        assert_eq!("auto".parse::<Mode>(), Ok(Mode::Auto));
        assert_eq!("Text".parse::<Mode>(), Ok(Mode::Text));
        assert_eq!("ANSI".parse::<Mode>(), Ok(Mode::Ansi));
        assert_eq!("BitMap".parse::<Mode>(), Ok(Mode::Bitmap));
        assert_eq!("kitty".parse::<Mode>(), Ok(Mode::Kitty));
        assert_eq!("Sixel".parse::<Mode>(), Ok(Mode::Sixel));
    }

    #[test]
    fn mode_rejects_unknown() {
        assert!("blocks".parse::<Mode>().is_err());
        assert!("".parse::<Mode>().is_err());
    }

    #[test]
    fn encode_options_report_their_format() {
        assert_eq!(EncodeOptions::Qr { ec: Ec::M }.format(), Format::Qr);
        assert_eq!(EncodeOptions::DataMatrix.format(), Format::DataMatrix);
        assert_eq!(
            EncodeOptions::Pdf417 {
                ec: Pdf417Ec(4),
                columns: None,
            }
            .format(),
            Format::Pdf417
        );
        assert_eq!(
            EncodeOptions::Aztec {
                ec: AztecEc(33),
                layers: None,
            }
            .format(),
            Format::Aztec
        );
    }

    #[test]
    fn aztec_ec_parses_the_percentage_range() {
        for percent in [0u8, 23, 33, 100] {
            assert_eq!(percent.to_string().parse::<AztecEc>(), Ok(AztecEc(percent)));
        }
    }

    #[test]
    fn aztec_ec_rejects_out_of_range_and_non_numeric() {
        for bad in ["101", "255", "-1", "", "x", "33%"] {
            assert!(bad.parse::<AztecEc>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn aztec_ec_defaults_to_33_percent() {
        assert_eq!(AztecEc::default(), AztecEc(33));
    }

    #[test]
    fn aztec_ec_from_percent_checks_the_range() {
        assert_eq!(AztecEc::from_percent(0), Some(AztecEc(0)));
        assert_eq!(AztecEc::from_percent(100), Some(AztecEc(100)));
        assert_eq!(AztecEc::from_percent(101), None);
    }

    #[test]
    fn pdf417_ec_parses_the_digits_zero_through_eight() {
        for level in 0..=8u8 {
            assert_eq!(level.to_string().parse::<Pdf417Ec>(), Ok(Pdf417Ec(level)));
        }
    }

    #[test]
    fn pdf417_ec_rejects_out_of_range_and_non_numeric() {
        for bad in ["9", "10", "-1", "", "L", "two"] {
            assert!(bad.parse::<Pdf417Ec>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn pdf417_ec_defaults_to_level_two() {
        assert_eq!(Pdf417Ec::default(), Pdf417Ec(2));
    }

    #[test]
    fn pdf417_ec_from_digit_checks_the_range() {
        assert_eq!(Pdf417Ec::from_digit(0), Some(Pdf417Ec(0)));
        assert_eq!(Pdf417Ec::from_digit(8), Some(Pdf417Ec(8)));
        assert_eq!(Pdf417Ec::from_digit(9), None);
    }
}
