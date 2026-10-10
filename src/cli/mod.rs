//! Command-line interface: argument parsing (`Cli`) and orchestration (`run`).
//!
//! All argument parsing lives here; the graphics capability probes live in
//! [`probe`] and the `--help` hint plumbing in [`help`]. Their public items are
//! re-exported below so callers keep using the `cli::` path.

use std::io::{IsTerminal, Write};

use clap::Parser;
use qrcode::QrCode;

use crate::error::AppError;
use crate::types::{Config, Ec, GlyphSet, InputSpec, Mode, RenderMode};

mod help;
mod probe;

pub use help::{add_mode_hint, mode_support_hint};

/// Turn text into a terminal QR code.
#[derive(Debug, Parser)]
#[command(
    name = "qrtxt",
    version,
    about = "Turn text into a terminal QR code",
    long_about = "Turn text into a terminal QR code.\n\n\
Reads a payload from a literal argument, a file, or standard input and prints it as one or \
more QR codes. By default it picks the best rendering the terminal offers: a crisp bitmap \
over the Kitty or Sixel graphics protocol where supported, otherwise Unicode block \
characters, so a plain UTF-8 terminal always works. Use --mode to force a specific renderer.\n\n\
A payload too large for a single symbol is split automatically across several balanced codes, \
printed in order under a `QR i/N` caption. Use --max-bytes to cap the bytes per code, or \
--chunk to require a minimum number of codes.",
    after_help = "Source: https://github.com/can2049/qrtxt"
)]
pub struct Cli {
    /// Literal payload to encode.
    ///
    /// Used exactly as given (no trailing-newline stripping). When omitted, the
    /// payload is read from --file, or from standard input when no file is given.
    #[arg(value_name = "DATA", help_heading = "Payload")]
    pub data: Option<String>,

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

    /// Cap the payload of each QR code at BYTES bytes; implies splitting (default:
    /// the symbol's own limit).
    ///
    /// Even a payload that fits one symbol is split so that no code exceeds BYTES
    /// bytes, keeping the codes as evenly sized as possible. A value larger than a
    /// symbol can hold is clamped down to the symbol's own limit. The value must be
    /// at least 1.
    #[arg(
        short = 'm',
        long = "max-bytes",
        value_name = "BYTES",
        value_parser = clap::value_parser!(u32).range(1..),
        help_heading = "Encoding"
    )]
    pub max_bytes: Option<u32>,

    /// Split the payload across at least COUNT QR codes; advisory (default: no
    /// minimum).
    ///
    /// The payload is divided into COUNT balanced codes when the content allows,
    /// otherwise into as many codes as possible (at most one per character). Can be
    /// combined with --max-bytes; whichever forces more codes wins. The value must
    /// be at least 1.
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
    /// block glyphs (honouring --glyphs); `ansi` draws reverse-video escape codes;
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

    /// Invert the ink, for light-background terminals.
    ///
    /// Modules are drawn in the terminal's foreground color, so the default suits a
    /// dark background. On a light background the code would appear inverted; pass
    /// this to keep it dark-on-light and scannable. In the bitmap modes it swaps
    /// the two colors instead.
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
    /// Convert parsed arguments into a neutral configuration.
    #[must_use]
    pub fn to_config(&self) -> Config {
        let input = match (&self.data, &self.file) {
            (Some(text), _) => InputSpec::Literal(text.clone()),
            (None, Some(path)) => InputSpec::File(path.clone()),
            (None, None) => InputSpec::Stdin,
        };
        Config {
            input,
            ec: self.error,
            border: self.border,
            invert: self.invert,
            glyphs: self.glyphs,
            mode: base_render_mode(self.mode),
            preserve_newline: self.preserve_newline,
            max_bytes: self.max_bytes.map(|bytes| bytes as usize),
            min_chunks: self.chunk.map(|count| count as usize),
        }
    }
}

/// The render mode a [`Mode`] maps to without a terminal to probe.
///
/// `auto` becomes text here: this is the interpretation [`Cli::to_config`] hands
/// to the pure [`run`] path. [`run_with`] refines `auto` against the terminal's
/// capabilities before running.
fn base_render_mode(mode: Mode) -> RenderMode {
    match mode {
        Mode::Auto | Mode::Text => RenderMode::Compact,
        Mode::Ansi => RenderMode::Ansi,
        Mode::Kitty => RenderMode::Kitty,
        Mode::Sixel => RenderMode::Sixel,
    }
}

/// Default entry point: reads global stdin/stdout and TTY state.
pub fn run_with(cli: &Cli) -> Result<(), AppError> {
    let mut cfg = cli.to_config();
    cfg.mode = resolve_mode(cli.mode)?;
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
    let payload = crate::input::resolve(cfg, stdin, stdin_is_tty)?;
    let codes = crate::encode::encode_multi(&payload, cfg.ec, cfg.max_bytes, cfg.min_chunks)?;

    let mut writer = std::io::BufWriter::new(stdout);
    if let Err(error) = render_all(cfg, &codes, &mut writer) {
        return map_write_error(error);
    }
    match writer.flush() {
        Ok(()) => Ok(()),
        Err(error) => map_write_error(error),
    }
}

/// First image id handed to the transmitted bitmaps (FR-3.12).
///
/// Each code takes the next id. The Kitty protocol replaces an image stored under
/// a repeated id, so sharing one id would leave only the last code visible.
const KITTY_IMAGE_ID_BASE: u32 = 424_242;

/// Draw every code; when there is more than one, caption each and separate them.
fn render_all(cfg: &Config, codes: &[QrCode], out: &mut dyn std::io::Write) -> std::io::Result<()> {
    let total = codes.len();
    for (index, qr) in codes.iter().enumerate() {
        if total > 1 {
            writeln!(out, "QR {}/{}", index + 1, total)?;
        }
        match cfg.mode {
            RenderMode::Kitty => {
                let id = KITTY_IMAGE_ID_BASE.wrapping_add(index as u32);
                crate::render::render_kitty(qr, cfg.border, cfg.invert, id, out)?;
            }
            RenderMode::Sixel => {
                crate::render::render_sixel(qr, cfg.border, cfg.invert, out)?;
            }
            RenderMode::Compact => {
                let frame = crate::render::build_ink(qr, cfg.border, cfg.invert, cfg.glyphs);
                crate::render::render(&frame, cfg.glyphs, out)?;
            }
            RenderMode::Ansi => {
                // ANSI draws one module per two spaces and ignores the glyph set,
                // so pin the frame's tile to the smallest one: the output is then
                // independent of `--glyphs` instead of shifting with it.
                let frame = crate::render::build_ink(qr, cfg.border, cfg.invert, GlyphSet::Half);
                crate::render::render_ansi(&frame, out)?;
            }
        }
        if total > 1 && index + 1 < total {
            out.write_all(b"\n")?;
        }
    }
    Ok(())
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

    #[test]
    fn literal_takes_precedence_over_stdin() {
        let cfg = parse(&["hello"]).to_config();
        assert_eq!(cfg.input, InputSpec::Literal("hello".to_string()));
    }

    #[test]
    fn file_input() {
        let cfg = parse(&["--file", "payload.txt"]).to_config();
        assert_eq!(cfg.input, InputSpec::File("payload.txt".into()));
    }

    #[test]
    fn no_source_means_stdin() {
        let cfg = parse(&[]).to_config();
        assert_eq!(cfg.input, InputSpec::Stdin);
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
        let cfg = parse(&["x", "--mode", "ansi", "--invert", "--glyphs", "braille"]).to_config();
        assert_eq!(cfg.mode, RenderMode::Ansi);
        assert!(cfg.invert);
        assert_eq!(cfg.glyphs, GlyphSet::Braille);
        assert_eq!(cfg.border, 4);
        assert_eq!(cfg.max_bytes, None);
        assert_eq!(cfg.ec, Ec::L);
    }

    #[test]
    fn short_flags_resolve() {
        let cfg = parse(&[
            "-p", "-i", "-M", "ansi", "-g", "half", "-e", "H", "-b", "2", "-m", "9", "x",
        ])
        .to_config();
        assert!(cfg.preserve_newline);
        assert!(cfg.invert);
        assert_eq!(cfg.mode, RenderMode::Ansi);
        assert_eq!(cfg.glyphs, GlyphSet::Half);
        assert_eq!(cfg.ec, Ec::H);
        assert_eq!(cfg.border, 2);
        assert_eq!(cfg.max_bytes, Some(9));
    }

    #[test]
    fn max_bytes_defaults_to_none() {
        assert_eq!(parse(&["x"]).to_config().max_bytes, None);
    }

    #[test]
    fn max_bytes_parses_long_and_short() {
        assert_eq!(
            parse(&["--max-bytes", "100", "x"]).to_config().max_bytes,
            Some(100)
        );
        assert_eq!(parse(&["-m", "100", "x"]).to_config().max_bytes, Some(100));
    }

    #[test]
    fn max_bytes_zero_is_rejected() {
        assert!(Cli::try_parse_from(["qrtxt", "--max-bytes", "0", "x"]).is_err());
    }

    #[test]
    fn chunk_defaults_to_none() {
        assert_eq!(parse(&["x"]).to_config().min_chunks, None);
    }

    #[test]
    fn chunk_parses_long_and_short() {
        assert_eq!(
            parse(&["--chunk", "3", "x"]).to_config().min_chunks,
            Some(3)
        );
        assert_eq!(parse(&["-c", "3", "x"]).to_config().min_chunks, Some(3));
    }

    #[test]
    fn chunk_zero_is_rejected() {
        assert!(Cli::try_parse_from(["qrtxt", "--chunk", "0", "x"]).is_err());
    }

    #[test]
    fn render_all_captions_multiple_codes() {
        let cfg = parse(&["-e", "L", "x"]).to_config();
        let codes =
            crate::encode::encode_multi(&vec![b'x'; 4000], cfg.ec, cfg.max_bytes, cfg.min_chunks)
                .unwrap();
        assert!(codes.len() > 1);
        let mut out = Vec::new();
        render_all(&cfg, &codes, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("QR 1/"), "{text}");
        assert!(text.contains(&format!("QR {0}/{0}", codes.len())), "{text}");
    }

    #[test]
    fn render_all_leaves_a_single_code_uncaptioned() {
        let cfg = parse(&["x"]).to_config();
        let codes =
            crate::encode::encode_multi(b"hello", cfg.ec, cfg.max_bytes, cfg.min_chunks).unwrap();
        assert_eq!(codes.len(), 1);
        let mut out = Vec::new();
        render_all(&cfg, &codes, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(!text.contains("QR "), "{text}");
    }

    #[test]
    fn ansi_output_is_independent_of_the_glyph_set() {
        // Regression: ANSI ignores the glyph set, but the frame was still padded
        // to the chosen glyph's tile, so `--mode ansi -g braille` gained blank
        // rows over `--mode ansi`. The frame now uses the smallest tile in ANSI
        // mode, so they match.
        let ansi = parse(&["--mode", "ansi", "hi"]).to_config();
        let braille = parse(&["--mode", "ansi", "-g", "braille", "hi"]).to_config();
        assert_eq!(braille.mode, RenderMode::Ansi);
        let codes =
            crate::encode::encode_multi(b"hi", ansi.ec, ansi.max_bytes, ansi.min_chunks).unwrap();
        let draw = |cfg: &Config| {
            let mut out = Vec::new();
            render_all(cfg, &codes, &mut out).unwrap();
            out
        };
        assert_eq!(draw(&ansi), draw(&braille));
    }

    #[test]
    fn render_all_kitty_renders_every_code_with_its_own_image_id() {
        // Regression: every code once shared one image id, so the terminal
        // replaced every bitmap but the last and the earlier codes looked
        // unrendered. Each code must now be transmitted as its own image with a
        // distinct id, in order.
        let cfg = parse(&["--mode", "kitty", "-e", "L", "x"]).to_config();
        let codes =
            crate::encode::encode_multi(&vec![b'x'; 6000], cfg.ec, cfg.max_bytes, cfg.min_chunks)
                .unwrap();
        assert!(codes.len() >= 3, "payload should split into several codes");

        let mut out = Vec::new();
        render_all(&cfg, &codes, &mut out).unwrap();
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
    fn long_flags_resolve() {
        let cfg = parse(&[
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
        ])
        .to_config();
        assert!(cfg.preserve_newline);
        assert!(cfg.invert);
        assert_eq!(cfg.mode, RenderMode::Ansi);
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
        assert_eq!(
            parse(&["--mode", "kitty", "x"]).to_config().mode,
            RenderMode::Kitty
        );
        assert_eq!(
            parse(&["-M", "kitty", "x"]).to_config().mode,
            RenderMode::Kitty
        );
        assert_ne!(parse(&["x"]).to_config().mode, RenderMode::Kitty);
    }

    #[test]
    fn mode_flag_selects_sixel() {
        assert_eq!(
            parse(&["--mode", "sixel", "x"]).to_config().mode,
            RenderMode::Sixel
        );
        assert_eq!(
            parse(&["-M", "sixel", "x"]).to_config().mode,
            RenderMode::Sixel
        );
        assert_ne!(parse(&["x"]).to_config().mode, RenderMode::Sixel);
    }

    #[test]
    fn mode_defaults_to_auto_then_text_without_a_terminal() {
        // `auto` is the parsed default; without a terminal to probe it lands on
        // text, which is what the pure `run` path and these tests observe.
        assert_eq!(parse(&["x"]).mode, Mode::Auto);
        assert_eq!(parse(&["x"]).to_config().mode, RenderMode::Compact);
        assert_eq!(
            parse(&["--mode", "text", "x"]).to_config().mode,
            RenderMode::Compact
        );
    }

    #[test]
    fn mode_rejects_unknown_value() {
        assert!(Cli::try_parse_from(["qrtxt", "--mode", "dense", "x"]).is_err());
    }

    #[test]
    fn render_all_sixel_renders_one_sequence_per_code() {
        let cfg = parse(&["--mode", "sixel", "-e", "L", "x"]).to_config();
        let codes =
            crate::encode::encode_multi(&vec![b'x'; 6000], cfg.ec, cfg.max_bytes, cfg.min_chunks)
                .unwrap();
        assert!(codes.len() >= 3, "payload should split into several codes");

        let mut out = Vec::new();
        render_all(&cfg, &codes, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        // One Sixel sequence (DCS introducer) per code; nothing skipped or doubled.
        assert_eq!(text.matches("\x1bPq").count(), codes.len());
    }
}
