//! Command-line interface: the `Cli` schema (`Cli`) and orchestration (`run`).
//!
//! All argument parsing lives here; the argv entry point is [`parse`], the
//! graphics capability probes live in [`probe`] and the `--help` hint plumbing in
//! [`help`]. Their public items are re-exported below so callers keep using the
//! `cli::` path.
//!
//! The CLI is a subcommand per symbology (`qr` / `datamatrix`), so each format
//! exposes only the options it understands; the options shared by every format
//! live in [`CommonArgs`].

use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::error::AppError;
use crate::types::{Config, Ec, EncodeOptions, GlyphSet, InputSpec, Mode, Pdf417Ec, RenderMode};

mod help;
mod parse;
mod probe;

pub use help::{add_mode_hint, mode_support_hint};
pub use parse::parse;

/// Turn text into a terminal QR code, Data Matrix, or PDF417.
#[derive(Debug, Parser)]
#[command(
    name = "qrtxt",
    version,
    about = "Turn text into a terminal QR code, Data Matrix, or PDF417",
    long_about = "Turn text into a terminal QR code, Data Matrix, or PDF417.\n\n\
Reads a payload from a literal argument, a file, or standard input and prints it as one or \
more symbols. The subcommand chooses the symbology: `qr` draws an ordinary QR code, \
`datamatrix` draws a Data Matrix (ECC 200) symbol, and `pdf417` draws a stacked PDF417 \
symbol. By default the best rendering the terminal \
offers is used: a crisp bitmap over the Kitty or Sixel graphics protocol where supported, \
otherwise Unicode block characters, so a plain UTF-8 terminal always works. Use --mode to \
force a specific renderer.\n\n\
A payload too large for a single symbol is split automatically across several balanced \
symbols, printed in order under a `QR Code i/N` (or `Data Matrix i/N`, `PDF417 i/N`) caption. \
Use --max-bytes to cap the bytes per symbol, or --chunk to require a minimum number of symbols.",
    after_help = "Source: https://github.com/can2049/qrtxt"
)]
pub struct Cli {
    /// The options shared by every subcommand, also listed in the top-level help.
    ///
    /// These are `global`, so they are accepted before or after the subcommand and
    /// appear under both `qrtxt -h` and `qrtxt <command> -h`; the per-subcommand
    /// copies exist only to be displayed there, and the values are read from the
    /// chosen subcommand.
    #[command(flatten)]
    common: CommonArgs,

    /// The symbology to produce.
    #[command(subcommand)]
    command: Command,
}

/// The symbology to produce, one subcommand per format.
#[derive(Debug, Subcommand)]
enum Command {
    /// Encode a QR code.
    Qr(QrArgs),
    /// Encode a Data Matrix (ECC 200) symbol.
    #[command(visible_alias = "dm")]
    Datamatrix(DatamatrixArgs),
    /// Encode a PDF417 symbol.
    #[command(visible_alias = "pdf")]
    Pdf417(Pdf417Args),
}

/// Options shared by every subcommand.
#[derive(Debug, Args)]
struct CommonArgs {
    /// Literal payload to encode.
    ///
    /// Used exactly as given (no trailing-newline stripping). When omitted, the
    /// payload is read from --file, or from standard input when no file is given.
    #[arg(value_name = "DATA", help_heading = "Payload")]
    data: Option<String>,

    /// Read the payload from a file.
    ///
    /// The file is read as raw bytes. One trailing newline (LF or CRLF) is removed
    /// unless --preserve-newline is set. Cannot be combined with a positional DATA.
    #[arg(
        short = 'f',
        long = "file",
        value_name = "PATH",
        conflicts_with = "data",
        global = true,
        help_heading = "Payload"
    )]
    file: Option<PathBuf>,

    /// Keep one trailing newline from file or piped input.
    ///
    /// By default exactly one trailing newline (LF or CRLF) is stripped from file
    /// and standard-input payloads; this flag keeps it. A literal DATA argument is
    /// never stripped.
    #[arg(
        short = 'p',
        long = "preserve-newline",
        global = true,
        help_heading = "Payload"
    )]
    preserve_newline: bool,

    /// Cap the payload of each symbol at BYTES bytes; implies splitting (default:
    /// the symbol's own limit).
    ///
    /// Even a payload that fits one symbol is split so that no symbol exceeds
    /// BYTES bytes, keeping the symbols as evenly sized as possible. A value
    /// larger than a symbol can hold is clamped down to the symbol's own limit.
    /// The value must be at least 1.
    #[arg(
        short = 'm',
        long = "max-bytes",
        value_name = "BYTES",
        value_parser = clap::value_parser!(u32).range(1..),
        global = true,
        help_heading = "Encoding"
    )]
    max_bytes: Option<u32>,

    /// Split the payload across at least COUNT symbols; advisory (default: no
    /// minimum).
    ///
    /// The payload is divided into COUNT balanced symbols when the content
    /// allows, otherwise into as many symbols as possible (at most one per
    /// character). Can be combined with --max-bytes; whichever forces more
    /// symbols wins. The value must be at least 1.
    #[arg(
        short = 'c',
        long = "chunk",
        value_name = "COUNT",
        value_parser = clap::value_parser!(u32).range(1..),
        global = true,
        help_heading = "Encoding"
    )]
    chunk: Option<u32>,

    /// How to draw the code: auto, text, ansi, kitty, or sixel.
    ///
    /// `auto` (the default) uses a bitmap when the terminal supports one — it is
    /// smaller and crisper than block characters — and falls back to Unicode block
    /// glyphs otherwise. It never emits a bitmap when standard output is not a
    /// terminal, so piping or redirecting always yields text. `text` draws Unicode
    /// block glyphs (honouring --glyphs); `ansi` draws black-on-white escape codes;
    /// `kitty` and `sixel` force that bitmap protocol and fail with a usage error
    /// if the terminal does not support it.
    #[arg(
        short = 'M',
        long = "mode",
        value_name = "MODE",
        default_value = "auto",
        global = true,
        help_heading = "Rendering"
    )]
    mode: Mode,

    /// Quiet-zone width in modules; 0 removes the margin.
    ///
    /// The blank margin drawn around the symbol. The QR specification asks for 4;
    /// smaller values shrink the output but can make scanning less reliable.
    #[arg(
        short = 'b',
        long = "border",
        default_value_t = 4,
        global = true,
        help_heading = "Rendering"
    )]
    border: u32,

    /// Invert the colors: draw a light-on-dark code instead of dark-on-light.
    ///
    /// The text and ANSI renderers set explicit colors, so the code is a
    /// self-contained dark-on-white image that scans on any terminal. Pass this to
    /// swap them for a light-on-dark code. In the bitmap modes it swaps the two
    /// colors too.
    #[arg(
        short = 'i',
        long = "invert",
        global = true,
        help_heading = "Rendering"
    )]
    invert: bool,

    /// Glyph set used to pack modules into character cells: half (h; 1x2 modules
    /// per cell, most robust), quadrant (q; 2x2), or braille (b; 2x4, densest).
    ///
    /// Each set also accepts its initial letter (e.g. `-g b`). Braille needs a font
    /// that renders dots tightly. Only used by `--mode text`; the bitmap and ANSI
    /// renderers ignore it.
    #[arg(
        short = 'g',
        long = "glyphs",
        value_name = "SET",
        default_value = "half",
        global = true,
        help_heading = "Text rendering"
    )]
    glyphs: GlyphSet,
}

impl CommonArgs {
    /// Build the shared parts of a [`Config`], leaving `encode` to the subcommand.
    fn to_config(&self, mode: RenderMode, encode: EncodeOptions) -> Config {
        let input = match (&self.data, &self.file) {
            (Some(text), _) => InputSpec::Literal(text.clone()),
            (None, Some(path)) => InputSpec::File(path.clone()),
            (None, None) => InputSpec::Stdin,
        };
        Config {
            input,
            encode,
            border: self.border,
            invert: self.invert,
            glyphs: self.glyphs,
            mode,
            preserve_newline: self.preserve_newline,
            max_bytes: self.max_bytes.map(|bytes| bytes as usize),
            min_chunks: self.chunk.map(|count| count as usize),
        }
    }

    /// The requested render [`Mode`], before `auto` is resolved.
    fn mode(&self) -> Mode {
        self.mode
    }
}

/// Options for the `qr` subcommand.
#[derive(Debug, Args)]
struct QrArgs {
    #[command(flatten)]
    common: CommonArgs,

    /// Error-correction level: L (about 7% recoverable), M (15%), Q (25%), or H
    /// (30%).
    ///
    /// A higher level recovers from more damage but holds less data, which can
    /// force the symbol to a larger version. The level is never boosted
    /// automatically, and the value is case-insensitive.
    #[arg(
        short = 'e',
        long = "error-correction",
        value_name = "LEVEL",
        default_value = "L",
        help_heading = "Encoding"
    )]
    error: Ec,
}

/// Options for the `datamatrix` subcommand.
///
/// Data Matrix exposes no encoding options: its error correction is fixed by the
/// symbol size, so there is no `--error-correction` here.
#[derive(Debug, Args)]
struct DatamatrixArgs {
    #[command(flatten)]
    common: CommonArgs,
}

/// Options for the `pdf417` subcommand.
///
/// PDF417 numbers its error-correction levels from 0 to 8 (each step roughly
/// doubles the correction codewords), so `--error-correction` takes a digit here
/// rather than QR's letters.
#[derive(Debug, Args)]
struct Pdf417Args {
    #[command(flatten)]
    common: CommonArgs,

    /// Error-correction level: 0 (least redundancy, most data) to 8 (most).
    ///
    /// Each step roughly doubles the number of correction codewords, so a higher
    /// level recovers from more damage but holds less data. The level is never
    /// raised automatically, and the value must be a single digit from 0 to 8.
    #[arg(
        short = 'e',
        long = "error-correction",
        value_name = "LEVEL",
        default_value = "2",
        help_heading = "Encoding"
    )]
    error: Pdf417Ec,

    /// Force the number of data columns (1-30), shaping the symbol.
    ///
    /// PDF417 stacks its data across a fixed number of columns; fewer columns make
    /// a narrower, taller symbol and more columns a wider, shorter one. Left unset,
    /// the encoder picks a balanced shape. The value must be from 1 to 30.
    ///
    /// Also accepted as `-C` and `--cl`.
    #[arg(
        short = 'C',
        long = "columns",
        visible_alias = "cl",
        value_name = "N",
        value_parser = clap::value_parser!(u32).range(1..=30),
        help_heading = "Encoding"
    )]
    columns: Option<u32>,
}

impl Cli {
    /// Convert parsed arguments into a neutral configuration for `mode`.
    ///
    /// The render mode is supplied by the caller rather than derived here: the
    /// `auto` policy needs the terminal's capabilities, so [`run_with`] resolves
    /// it via [`resolve_mode`] first. Taking it as a parameter keeps this mapping
    /// pure and leaves a single source of truth for `Mode` -> [`RenderMode`].
    #[must_use]
    pub fn to_config(&self, mode: RenderMode) -> Config {
        match &self.command {
            Command::Qr(args) => args
                .common
                .to_config(mode, EncodeOptions::Qr { ec: args.error }),
            Command::Datamatrix(args) => args.common.to_config(mode, EncodeOptions::DataMatrix),
            Command::Pdf417(args) => args.common.to_config(
                mode,
                EncodeOptions::Pdf417 {
                    ec: args.error,
                    columns: args.columns,
                },
            ),
        }
    }

    /// The requested render [`Mode`], before `auto` is resolved.
    fn mode(&self) -> Mode {
        match &self.command {
            Command::Qr(args) => args.common.mode(),
            Command::Datamatrix(args) => args.common.mode(),
            Command::Pdf417(args) => args.common.mode(),
        }
    }
}

/// Default entry point: reads global stdin/stdout and TTY state.
pub fn run_with(cli: &Cli) -> Result<(), AppError> {
    let cfg = cli.to_config(resolve_mode(cli.mode())?);
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    let stdin_is_tty = std::io::stdin().is_terminal();
    run(&cfg, &mut stdin, &mut stdout, stdin_is_tty)
}

/// Resolve the requested [`Mode`] into a concrete [`RenderMode`], probing the
/// terminal where needed.
///
/// `auto` picks a bitmap only when standard output is a terminal and a protocol
/// is supported (Kitty preferred, then Sixel); otherwise it stays text — a
/// redirected or piped run is never handed bitmap escape bytes.
fn resolve_mode(mode: Mode) -> Result<RenderMode, AppError> {
    match mode {
        Mode::Auto => match probe::probe_for_auto()? {
            Some(caps) if caps.kitty => Ok(RenderMode::Kitty),
            Some(caps) if caps.sixel => Ok(RenderMode::Sixel),
            _ => Ok(RenderMode::Compact),
        },
        Mode::Text => Ok(RenderMode::Compact),
        Mode::Ansi => Ok(RenderMode::Ansi),
        Mode::Kitty => {
            probe::ensure_kitty_supported()?;
            Ok(RenderMode::Kitty)
        }
        Mode::Sixel => {
            probe::ensure_sixel_supported()?;
            Ok(RenderMode::Sixel)
        }
    }
}

/// Orchestrate input -> encode -> render. All side effects live here.
pub fn run(
    cfg: &Config,
    stdin: &mut dyn std::io::Read,
    stdout: &mut dyn std::io::Write,
    stdin_is_tty: bool,
) -> Result<(), AppError> {
    let payload = crate::input::resolve(&cfg.input, cfg.preserve_newline, stdin, stdin_is_tty)?;
    let codes = crate::encode::encode_multi(&payload, &cfg.encode, cfg.max_bytes, cfg.min_chunks)?;

    let mut writer = std::io::BufWriter::new(stdout);
    let options = crate::render::RenderOptions {
        border: cfg.border,
        invert: cfg.invert,
        glyphs: cfg.glyphs,
    };
    if let Err(error) =
        crate::render::render_all(&codes, cfg.encode.format(), cfg.mode, options, &mut writer)
    {
        return map_write_error(error);
    }
    match writer.flush() {
        Ok(()) => Ok(()),
        Err(error) => map_write_error(error),
    }
}

fn map_write_error(error: std::io::Error) -> Result<(), AppError> {
    // A closed downstream (e.g. `| head`) is not an error for us (ADR-6).
    if error.kind() == std::io::ErrorKind::BrokenPipe {
        Ok(())
    } else {
        Err(AppError::Io {
            context: "cannot write barcode output",
            source: error,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Format;

    fn parse(args: &[&str]) -> Cli {
        // The first element is argv[0] (the program name).
        let mut argv = vec!["qrtxt"];
        argv.extend_from_slice(args);
        Cli::try_parse_from(argv).expect("arguments should parse")
    }

    /// A config with the default (text) render mode, for the tests that do not
    /// exercise mode resolution.
    fn config(args: &[&str]) -> Config {
        parse(args).to_config(RenderMode::Compact)
    }

    /// The QR error level in `cfg`, failing if it is not a QR config.
    fn qr_ec(cfg: &Config) -> Ec {
        match cfg.encode {
            EncodeOptions::Qr { ec } => ec,
            EncodeOptions::DataMatrix | EncodeOptions::Pdf417 { .. } => {
                panic!("expected QR options")
            }
        }
    }

    #[test]
    fn literal_takes_precedence_over_stdin() {
        let cfg = config(&["qr", "hello"]);
        assert_eq!(cfg.input, InputSpec::Literal("hello".to_string()));
    }

    #[test]
    fn file_input() {
        let cfg = config(&["qr", "--file", "payload.txt"]);
        assert_eq!(cfg.input, InputSpec::File("payload.txt".into()));
    }

    #[test]
    fn no_source_means_stdin() {
        assert_eq!(config(&["qr"]).input, InputSpec::Stdin);
    }

    #[test]
    fn file_and_literal_conflict() {
        assert!(Cli::try_parse_from(["qrtxt", "qr", "--file", "a.txt", "hello"]).is_err());
    }

    #[test]
    fn invalid_values_are_rejected() {
        assert!(Cli::try_parse_from(["qrtxt", "qr", "--error-correction", "Z", "x"]).is_err());
        assert!(Cli::try_parse_from(["qrtxt", "qr", "--glyphs", "dense", "x"]).is_err());
    }

    #[test]
    fn defaults_and_flags() {
        let cli = parse(&[
            "qr", "x", "--mode", "ansi", "--invert", "--glyphs", "braille",
        ]);
        assert_eq!(cli.mode(), Mode::Ansi);
        let cfg = cli.to_config(RenderMode::Ansi);
        assert!(cfg.invert);
        assert_eq!(cfg.glyphs, GlyphSet::Braille);
        assert_eq!(cfg.border, 4);
        assert_eq!(cfg.max_bytes, None);
        assert_eq!(qr_ec(&cfg), Ec::L);
    }

    #[test]
    fn short_flags_resolve() {
        let cli = parse(&[
            "qr", "-p", "-i", "-M", "ansi", "-g", "half", "-e", "H", "-b", "2", "-m", "9", "x",
        ]);
        assert_eq!(cli.mode(), Mode::Ansi);
        let cfg = cli.to_config(RenderMode::Ansi);
        assert!(cfg.preserve_newline);
        assert!(cfg.invert);
        assert_eq!(cfg.glyphs, GlyphSet::Half);
        assert_eq!(qr_ec(&cfg), Ec::H);
        assert_eq!(cfg.border, 2);
        assert_eq!(cfg.max_bytes, Some(9));
    }

    #[test]
    fn max_bytes_defaults_to_none() {
        assert_eq!(config(&["qr", "x"]).max_bytes, None);
    }

    #[test]
    fn max_bytes_parses_long_and_short() {
        assert_eq!(
            config(&["qr", "--max-bytes", "100", "x"]).max_bytes,
            Some(100)
        );
        assert_eq!(config(&["qr", "-m", "100", "x"]).max_bytes, Some(100));
    }

    #[test]
    fn max_bytes_zero_is_rejected() {
        assert!(Cli::try_parse_from(["qrtxt", "qr", "--max-bytes", "0", "x"]).is_err());
    }

    #[test]
    fn chunk_defaults_to_none() {
        assert_eq!(config(&["qr", "x"]).min_chunks, None);
    }

    #[test]
    fn chunk_parses_long_and_short() {
        assert_eq!(config(&["qr", "--chunk", "3", "x"]).min_chunks, Some(3));
        assert_eq!(config(&["qr", "-c", "3", "x"]).min_chunks, Some(3));
    }

    #[test]
    fn chunk_zero_is_rejected() {
        assert!(Cli::try_parse_from(["qrtxt", "qr", "--chunk", "0", "x"]).is_err());
    }

    #[test]
    fn long_flags_resolve() {
        let cli = parse(&[
            "qr",
            "--preserve-newline",
            "--invert",
            "--mode",
            "ansi",
            "--glyphs",
            "quadrant",
            "--error-correction",
            "Q",
            "--border",
            "1",
            "--max-bytes",
            "7",
            "--chunk",
            "4",
            "x",
        ]);
        assert_eq!(cli.mode(), Mode::Ansi);
        let cfg = cli.to_config(RenderMode::Ansi);
        assert!(cfg.preserve_newline);
        assert!(cfg.invert);
        assert_eq!(cfg.glyphs, GlyphSet::Quadrant);
        assert_eq!(qr_ec(&cfg), Ec::Q);
        assert_eq!(cfg.border, 1);
        assert_eq!(cfg.max_bytes, Some(7));
        assert_eq!(cfg.min_chunks, Some(4));
    }

    #[test]
    fn removed_options_are_rejected() {
        let cases: [&[&str]; 10] = [
            &["qrtxt", "qr", "--raw", "x"],
            &["qrtxt", "qr", "--ansi", "x"],
            &["qrtxt", "qr", "--ec", "x"],
            &["qrtxt", "qr", "--pad", "x"],
            &["qrtxt", "qr", "--size", "2", "x"],
            &["qrtxt", "qr", "--multi", "x"],
            &["qrtxt", "qr", "--no-compact", "x"],
            &["qrtxt", "qr", "--kitty", "x"],
            &["qrtxt", "qr", "--sixel", "x"],
            // The old flat format selector is gone: the subcommand picks the format.
            &["qrtxt", "qr", "--format", "dm", "x"],
        ];
        for args in cases {
            assert!(
                Cli::try_parse_from(args).is_err(),
                "{args:?} should no longer be accepted"
            );
        }
    }

    #[test]
    fn mode_flag_selects_kitty() {
        assert_eq!(parse(&["qr", "--mode", "kitty", "x"]).mode(), Mode::Kitty);
        assert_eq!(parse(&["qr", "-M", "kitty", "x"]).mode(), Mode::Kitty);
        assert_ne!(parse(&["qr", "x"]).mode(), Mode::Kitty);
    }

    #[test]
    fn mode_flag_selects_sixel() {
        assert_eq!(parse(&["qr", "--mode", "sixel", "x"]).mode(), Mode::Sixel);
        assert_eq!(parse(&["qr", "-M", "sixel", "x"]).mode(), Mode::Sixel);
        assert_ne!(parse(&["qr", "x"]).mode(), Mode::Sixel);
    }

    #[test]
    fn mode_defaults_to_auto() {
        assert_eq!(parse(&["qr", "x"]).mode(), Mode::Auto);
    }

    #[test]
    fn resolve_mode_maps_the_explicit_text_modes_without_probing() {
        // Text and ANSI need no terminal probe, so their mapping is a pure call;
        // `auto` (which does probe) is covered end to end by tests/cli.rs.
        assert_eq!(resolve_mode(Mode::Text).unwrap(), RenderMode::Compact);
        assert_eq!(resolve_mode(Mode::Ansi).unwrap(), RenderMode::Ansi);
    }

    #[test]
    fn to_config_uses_the_resolved_mode() {
        assert_eq!(
            parse(&["qr", "x"]).to_config(RenderMode::Sixel).mode,
            RenderMode::Sixel
        );
    }

    #[test]
    fn mode_rejects_unknown_value() {
        assert!(Cli::try_parse_from(["qrtxt", "qr", "--mode", "dense", "x"]).is_err());
    }

    #[test]
    fn datamatrix_subcommand_selects_datamatrix() {
        assert_eq!(
            config(&["datamatrix", "x"]).encode,
            EncodeOptions::DataMatrix
        );
        assert_eq!(config(&["dm", "x"]).encode, EncodeOptions::DataMatrix);
        assert_eq!(config(&["qr", "x"]).encode.format(), Format::Qr);
    }

    #[test]
    fn datamatrix_subcommand_rejects_error_correction() {
        // Data Matrix has no error-correction option; `-e` is a QR-only flag.
        assert!(Cli::try_parse_from(["qrtxt", "datamatrix", "-e", "H", "x"]).is_err());
    }

    #[test]
    fn pdf417_subcommand_selects_pdf417() {
        assert_eq!(
            config(&["pdf417", "x"]).encode,
            EncodeOptions::Pdf417 {
                ec: Pdf417Ec(2),
                columns: None,
            }
        );
        assert_eq!(
            config(&["pdf", "x"]).encode,
            EncodeOptions::Pdf417 {
                ec: Pdf417Ec(2),
                columns: None,
            }
        );
        assert_eq!(config(&["pdf417", "x"]).encode.format(), Format::Pdf417);
    }

    #[test]
    fn pdf417_error_correction_parses_zero_through_eight() {
        for level in 0..=8u8 {
            let cfg = config(&["pdf417", "-e", &level.to_string(), "x"]);
            assert_eq!(
                cfg.encode,
                EncodeOptions::Pdf417 {
                    ec: Pdf417Ec(level),
                    columns: None,
                }
            );
        }
    }

    #[test]
    fn pdf417_error_correction_defaults_to_two() {
        assert_eq!(
            config(&["pdf417", "x"]).encode,
            EncodeOptions::Pdf417 {
                ec: Pdf417Ec(2),
                columns: None,
            }
        );
    }

    #[test]
    fn pdf417_rejects_out_of_range_or_letter_levels() {
        for level in ["9", "-1", "H", "L"] {
            assert!(
                Cli::try_parse_from(["qrtxt", "pdf417", "-e", level, "x"]).is_err(),
                "level {level:?} should be rejected"
            );
        }
    }

    #[test]
    fn pdf417_columns_flow_into_the_config() {
        assert_eq!(
            config(&["pdf417", "--columns", "1", "x"]).encode,
            EncodeOptions::Pdf417 {
                ec: Pdf417Ec(2),
                columns: Some(1),
            }
        );
        assert_eq!(
            config(&["pdf417", "--columns", "30", "x"]).encode,
            EncodeOptions::Pdf417 {
                ec: Pdf417Ec(2),
                columns: Some(30),
            }
        );
        // Unset means the encoder picks the shape.
        assert_eq!(
            config(&["pdf417", "x"]).encode,
            EncodeOptions::Pdf417 {
                ec: Pdf417Ec(2),
                columns: None,
            }
        );
    }

    #[test]
    fn pdf417_columns_short_and_alias_forms_match_the_long_flag() {
        let expected = EncodeOptions::Pdf417 {
            ec: Pdf417Ec(2),
            columns: Some(5),
        };
        assert_eq!(config(&["pdf417", "-C", "5", "x"]).encode, expected);
        assert_eq!(config(&["pdf417", "--cl", "5", "x"]).encode, expected);
        assert_eq!(config(&["pdf417", "--columns", "5", "x"]).encode, expected);
    }

    #[test]
    fn pdf417_rejects_out_of_range_columns() {
        for columns in ["0", "31", "sixty"] {
            assert!(
                Cli::try_parse_from(["qrtxt", "pdf417", "--columns", columns, "x"]).is_err(),
                "columns {columns:?} should be rejected"
            );
        }
    }

    #[test]
    fn qr_and_datamatrix_reject_the_pdf417_columns_option() {
        assert!(Cli::try_parse_from(["qrtxt", "qr", "--columns", "5", "x"]).is_err());
        assert!(Cli::try_parse_from(["qrtxt", "datamatrix", "--columns", "5", "x"]).is_err());
    }

    #[test]
    fn qr_rejects_a_numeric_error_correction() {
        // The error-correction domain is per-format: QR still takes letters.
        assert!(Cli::try_parse_from(["qrtxt", "qr", "-e", "2", "x"]).is_err());
    }
}
