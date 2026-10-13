//! Command-line interface: the `Cli` schema (`Cli`) and orchestration (`run`).
//!
//! All argument parsing lives here; the argv entry point is [`parse`], the
//! graphics capability probes live in [`probe`] and the `--help` hint plumbing in
//! [`help`]. Their public items are re-exported below so callers keep using the
//! `cli::` path.

use std::io::{IsTerminal, Write};

use clap::Parser;

use crate::error::AppError;
use crate::types::{Config, Ec, Format, GlyphSet, InputSpec, Mode, RenderMode};

mod help;
mod parse;
mod probe;

pub use help::{add_mode_hint, mode_support_hint};
pub use parse::parse;

/// Turn text into a terminal QR code.
#[derive(Debug, Parser)]
#[command(
    name = "qrtxt",
    version,
    about = "Turn text into a terminal QR code or Data Matrix",
    long_about = "Turn text into a terminal QR code or Data Matrix.\n\n\
Reads a payload from a literal argument, a file, or standard input and prints it as one or \
more symbols. --format chooses a QR code (the default) or a Data Matrix (ECC 200) symbol. \
By default the best rendering the terminal offers is used: a crisp bitmap over the Kitty or \
Sixel graphics protocol where supported, otherwise Unicode block characters, so a plain UTF-8 \
terminal always works. Use --mode to force a specific renderer.\n\n\
A payload too large for a single symbol is split automatically across several balanced \
symbols, printed in order under a `QR Code i/N` (or `Data Matrix i/N`) caption. Use --max-bytes to cap the \
bytes per symbol, or --chunk to require a minimum number of symbols.",
    after_help = "Source: https://github.com/can2049/qrtxt"
)]
pub struct Cli {
    /// Literal payload to encode.
    ///
    /// Used exactly as given (no trailing-newline stripping). When omitted, the
    /// payload is read from --file, or from standard input when no file is given.
    #[arg(value_name = "DATA", help_heading = "Payload")]
    pub data: Option<String>,

    /// Barcode format to draw: qr code (qr) or datamatrix (dm).
    ///
    /// `qr code` (the default; the short alias `qr` also works) draws a QR code.
    /// `datamatrix` (or `dm`) draws a Data Matrix (ECC 200) symbol, which is
    /// always square. Data Matrix error correction is fixed by the symbol size, so
    /// --error-correction does not apply to it; and a Data Matrix holds far less
    /// data per symbol, so a large payload splits into more codes.
    #[arg(
        short = 'F',
        long = "format",
        value_name = "FORMAT",
        default_value = "qr code",
        help_heading = "Encoding"
    )]
    pub format: Format,

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
    pub error: Ec,

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
        help_heading = "Encoding"
    )]
    pub max_bytes: Option<u32>,

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
        help_heading = "Encoding"
    )]
    pub chunk: Option<u32>,

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
        help_heading = "Rendering"
    )]
    pub mode: Mode,

    /// Read the payload from a file.
    ///
    /// The file is read as raw bytes. One trailing newline (LF or CRLF) is removed
    /// unless --preserve-newline is set. Cannot be combined with a positional DATA.
    #[arg(
        short = 'f',
        long = "file",
        value_name = "PATH",
        conflicts_with = "data",
        help_heading = "Payload"
    )]
    pub file: Option<std::path::PathBuf>,

    /// Keep one trailing newline from file or piped input.
    ///
    /// By default exactly one trailing newline (LF or CRLF) is stripped from file
    /// and standard-input payloads; this flag keeps it. A literal DATA argument is
    /// never stripped.
    #[arg(short = 'p', long = "preserve-newline", help_heading = "Payload")]
    pub preserve_newline: bool,

    /// Quiet-zone width in modules; 0 removes the margin.
    ///
    /// The blank margin drawn around the symbol. The QR specification asks for 4;
    /// smaller values shrink the output but can make scanning less reliable.
    #[arg(
        short = 'b',
        long = "border",
        default_value_t = 4,
        help_heading = "Rendering"
    )]
    pub border: u32,

    /// Invert the colors: draw a light-on-dark code instead of dark-on-light.
    ///
    /// The text and ANSI renderers set explicit colors, so the code is a
    /// self-contained dark-on-white image that scans on any terminal. Pass this to
    /// swap them for a light-on-dark code. In the bitmap modes it swaps the two
    /// colors too.
    #[arg(short = 'i', long = "invert", help_heading = "Rendering")]
    pub invert: bool,

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
        help_heading = "Text rendering"
    )]
    pub glyphs: GlyphSet,
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
        let input = match (&self.data, &self.file) {
            (Some(text), _) => InputSpec::Literal(text.clone()),
            (None, Some(path)) => InputSpec::File(path.clone()),
            (None, None) => InputSpec::Stdin,
        };
        Config {
            input,
            ec: self.error,
            format: self.format,
            border: self.border,
            invert: self.invert,
            glyphs: self.glyphs,
            mode,
            preserve_newline: self.preserve_newline,
            max_bytes: self.max_bytes.map(|bytes| bytes as usize),
            min_chunks: self.chunk.map(|count| count as usize),
        }
    }
}

/// Default entry point: reads global stdin/stdout and TTY state.
pub fn run_with(cli: &Cli) -> Result<(), AppError> {
    let cfg = cli.to_config(resolve_mode(cli.mode)?);
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
    let codes =
        crate::encode::encode_multi(&payload, cfg.format, cfg.ec, cfg.max_bytes, cfg.min_chunks)?;

    let mut writer = std::io::BufWriter::new(stdout);
    let options = crate::render::RenderOptions {
        border: cfg.border,
        invert: cfg.invert,
        glyphs: cfg.glyphs,
    };
    if let Err(error) =
        crate::render::render_all(&codes, cfg.format, cfg.mode, options, &mut writer)
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
            context: "cannot write QR output",
            source: error,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn literal_takes_precedence_over_stdin() {
        let cfg = config(&["hello"]);
        assert_eq!(cfg.input, InputSpec::Literal("hello".to_string()));
    }

    #[test]
    fn file_input() {
        let cfg = config(&["--file", "payload.txt"]);
        assert_eq!(cfg.input, InputSpec::File("payload.txt".into()));
    }

    #[test]
    fn no_source_means_stdin() {
        assert_eq!(config(&[]).input, InputSpec::Stdin);
    }

    #[test]
    fn file_and_literal_conflict() {
        assert!(Cli::try_parse_from(["qrtxt", "--file", "a.txt", "hello"]).is_err());
    }

    #[test]
    fn invalid_values_are_rejected() {
        assert!(Cli::try_parse_from(["qrtxt", "--error-correction", "Z", "x"]).is_err());
        assert!(Cli::try_parse_from(["qrtxt", "--glyphs", "dense", "x"]).is_err());
    }

    #[test]
    fn defaults_and_flags() {
        let cli = parse(&["x", "--mode", "ansi", "--invert", "--glyphs", "braille"]);
        assert_eq!(cli.mode, Mode::Ansi);
        let cfg = cli.to_config(RenderMode::Ansi);
        assert!(cfg.invert);
        assert_eq!(cfg.glyphs, GlyphSet::Braille);
        assert_eq!(cfg.border, 4);
        assert_eq!(cfg.max_bytes, None);
        assert_eq!(cfg.ec, Ec::L);
    }

    #[test]
    fn short_flags_resolve() {
        let cli = parse(&[
            "-p", "-i", "-M", "ansi", "-g", "half", "-e", "H", "-b", "2", "-m", "9", "x",
        ]);
        assert_eq!(cli.mode, Mode::Ansi);
        let cfg = cli.to_config(RenderMode::Ansi);
        assert!(cfg.preserve_newline);
        assert!(cfg.invert);
        assert_eq!(cfg.glyphs, GlyphSet::Half);
        assert_eq!(cfg.ec, Ec::H);
        assert_eq!(cfg.border, 2);
        assert_eq!(cfg.max_bytes, Some(9));
    }

    #[test]
    fn max_bytes_defaults_to_none() {
        assert_eq!(config(&["x"]).max_bytes, None);
    }

    #[test]
    fn max_bytes_parses_long_and_short() {
        assert_eq!(config(&["--max-bytes", "100", "x"]).max_bytes, Some(100));
        assert_eq!(config(&["-m", "100", "x"]).max_bytes, Some(100));
    }

    #[test]
    fn max_bytes_zero_is_rejected() {
        assert!(Cli::try_parse_from(["qrtxt", "--max-bytes", "0", "x"]).is_err());
    }

    #[test]
    fn chunk_defaults_to_none() {
        assert_eq!(config(&["x"]).min_chunks, None);
    }

    #[test]
    fn chunk_parses_long_and_short() {
        assert_eq!(config(&["--chunk", "3", "x"]).min_chunks, Some(3));
        assert_eq!(config(&["-c", "3", "x"]).min_chunks, Some(3));
    }

    #[test]
    fn chunk_zero_is_rejected() {
        assert!(Cli::try_parse_from(["qrtxt", "--chunk", "0", "x"]).is_err());
    }

    #[test]
    fn long_flags_resolve() {
        let cli = parse(&[
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
        assert_eq!(cli.mode, Mode::Ansi);
        let cfg = cli.to_config(RenderMode::Ansi);
        assert!(cfg.preserve_newline);
        assert!(cfg.invert);
        assert_eq!(cfg.glyphs, GlyphSet::Quadrant);
        assert_eq!(cfg.ec, Ec::Q);
        assert_eq!(cfg.border, 1);
        assert_eq!(cfg.max_bytes, Some(7));
        assert_eq!(cfg.min_chunks, Some(4));
    }

    #[test]
    fn removed_options_are_rejected() {
        let cases: [&[&str]; 9] = [
            &["qrtxt", "--raw", "x"],
            &["qrtxt", "--ansi", "x"],
            &["qrtxt", "--ec", "x"],
            &["qrtxt", "--pad", "x"],
            &["qrtxt", "--size", "2", "x"],
            &["qrtxt", "--multi", "x"],
            &["qrtxt", "--no-compact", "x"],
            &["qrtxt", "--kitty", "x"],
            &["qrtxt", "--sixel", "x"],
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
        assert_eq!(parse(&["--mode", "kitty", "x"]).mode, Mode::Kitty);
        assert_eq!(parse(&["-M", "kitty", "x"]).mode, Mode::Kitty);
        assert_ne!(parse(&["x"]).mode, Mode::Kitty);
    }

    #[test]
    fn mode_flag_selects_sixel() {
        assert_eq!(parse(&["--mode", "sixel", "x"]).mode, Mode::Sixel);
        assert_eq!(parse(&["-M", "sixel", "x"]).mode, Mode::Sixel);
        assert_ne!(parse(&["x"]).mode, Mode::Sixel);
    }

    #[test]
    fn mode_defaults_to_auto() {
        assert_eq!(parse(&["x"]).mode, Mode::Auto);
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
            parse(&["x"]).to_config(RenderMode::Sixel).mode,
            RenderMode::Sixel
        );
    }

    #[test]
    fn mode_rejects_unknown_value() {
        assert!(Cli::try_parse_from(["qrtxt", "--mode", "dense", "x"]).is_err());
    }

    #[test]
    fn format_flag_selects_datamatrix() {
        assert_eq!(
            config(&["--format", "datamatrix", "x"]).format,
            Format::DataMatrix
        );
        assert_eq!(config(&["-F", "dm", "x"]).format, Format::DataMatrix);
        assert_eq!(config(&["x"]).format, Format::Qr);
    }

    #[test]
    fn format_rejects_unknown_value() {
        assert!(Cli::try_parse_from(["qrtxt", "--format", "pdf417", "x"]).is_err());
    }
}
