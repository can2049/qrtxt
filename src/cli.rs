//! Command-line interface: argument parsing (`Cli`) and orchestration (`run`).

use std::io::{IsTerminal, Write};

use clap::{CommandFactory, Parser};
use qrcode::QrCode;

use crate::error::AppError;
use crate::types::{Config, Ec, GlyphSet, InputSpec, RenderMode};

/// Turn text into a terminal QR code.
#[derive(Debug, Parser)]
#[command(
    name = "qrtxt",
    version,
    about = "Turn text into a terminal QR code",
    long_about = "Turn text into a terminal QR code.\n\n\
Reads a payload from a literal argument, a file, or standard input and prints it as one or \
more QR codes drawn with Unicode block characters, so no graphics environment is needed — \
just a UTF-8 terminal.\n\n\
A payload too large for a single symbol is split automatically across several balanced codes, \
printed in order under a `QR i/N` caption. Use --max-size to cap the bytes per code, or \
--chunk to require a minimum number of codes.",
    after_help = "Source: https://github.com/can2049/qrtxt"
)]
pub struct Cli {
    /// Literal payload to encode.
    ///
    /// Used exactly as given (no trailing-newline stripping). When omitted, the
    /// payload is read from --file, or from standard input when no file is given.
    #[arg(value_name = "DATA")]
    pub data: Option<String>,

    /// Glyph set used to pack modules into character cells: half (h; 1x2 modules
    /// per cell, most robust), quadrant (q; 2x2), or braille (b; 2x4, densest).
    ///
    /// Each set also accepts its initial letter (e.g. `-g b`). Braille needs a font
    /// that renders dots tightly.
    #[arg(
        short = 'g',
        long = "glyphs",
        value_name = "SET",
        default_value = "half"
    )]
    pub glyphs: GlyphSet,

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
        default_value = "L"
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
        long = "max-size",
        value_name = "BYTES",
        value_parser = clap::value_parser!(u32).range(1..)
    )]
    pub max_size: Option<u32>,

    /// Split the payload across at least COUNT QR codes; advisory (default: no
    /// minimum).
    ///
    /// The payload is divided into COUNT balanced codes when the content allows,
    /// otherwise into as many codes as possible (at most one per character). Can be
    /// combined with --max-size; whichever forces more codes wins. The value must be
    /// at least 1.
    #[arg(
        short = 'c',
        long = "chunk",
        value_name = "COUNT",
        value_parser = clap::value_parser!(u32).range(1..)
    )]
    pub chunk: Option<u32>,

    /// Render with ANSI escape codes instead of Unicode block characters.
    ///
    /// Use this on terminals that do not display block glyphs correctly. The output
    /// is wider (two spaces per module) and uses reverse-video escapes.
    #[arg(short = 'a', long = "no-compact")]
    pub no_compact: bool,

    /// Draw a smaller QR as a bitmap through the Kitty graphics protocol.
    ///
    /// A bitmap is not limited by the font, so every module stays crisp and the
    /// code can be much smaller than the block-glyph rendering. It needs a
    /// terminal that implements the protocol — kitty, Ghostty, or WezTerm, for
    /// example. qrtxt probes the terminal with an `a=q` handshake and, if it is
    /// unsupported (or standard output is not a terminal), fails with a usage
    /// error rather than print unusable escape bytes. The glyph set is ignored;
    /// `--invert` swaps the two colours.
    #[arg(short = 'k', long = "kitty", conflicts_with = "no_compact")]
    pub kitty: bool,

    /// Read the payload from a file.
    ///
    /// The file is read as raw bytes. One trailing newline (LF or CRLF) is removed
    /// unless --preserve-newline is set. Cannot be combined with a positional DATA.
    #[arg(
        short = 'f',
        long = "file",
        value_name = "PATH",
        conflicts_with = "data"
    )]
    pub file: Option<std::path::PathBuf>,

    /// Keep one trailing newline from file or piped input.
    ///
    /// By default exactly one trailing newline (LF or CRLF) is stripped from file
    /// and standard-input payloads; this flag keeps it. A literal DATA argument is
    /// never stripped.
    #[arg(short = 'p', long = "preserve-newline")]
    pub preserve_newline: bool,

    /// Quiet-zone width in modules; 0 removes the margin.
    ///
    /// The blank margin drawn around the symbol. The QR specification asks for 4;
    /// smaller values shrink the output but can make scanning less reliable.
    #[arg(short = 'b', long = "border", default_value_t = 4)]
    pub border: u32,

    /// Invert the ink, for light-background terminals.
    ///
    /// Modules are drawn in the terminal's foreground color, so the default suits a
    /// dark background. On a light background the code would appear inverted; pass
    /// this to keep it dark-on-light and scannable.
    #[arg(short = 'i', long = "invert")]
    pub invert: bool,
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
            mode: if self.kitty {
                RenderMode::Kitty
            } else if self.no_compact {
                RenderMode::Ansi
            } else {
                RenderMode::Compact
            },
            preserve_newline: self.preserve_newline,
            max_size: self.max_size.map(|bytes| bytes as usize),
            min_chunks: self.chunk.map(|count| count as usize),
        }
    }
}

/// Default entry point: reads global stdin/stdout and TTY state.
pub fn run_with(cli: &Cli) -> Result<(), AppError> {
    let cfg = cli.to_config();
    if cfg.mode == RenderMode::Kitty {
        ensure_kitty_supported()?;
    }
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    let stdin_is_tty = std::io::stdin().is_terminal();
    run(&cfg, &mut stdin, &mut stdout, stdin_is_tty)
}

/// Orchestrate input -> encode -> render. All side effects live here.
pub fn run(
    cfg: &Config,
    stdin: &mut dyn std::io::Read,
    stdout: &mut dyn std::io::Write,
    stdin_is_tty: bool,
) -> Result<(), AppError> {
    let payload = crate::input::resolve(cfg, stdin, stdin_is_tty)?;
    let codes = crate::encode::encode_multi(&payload, cfg.ec, cfg.max_size, cfg.min_chunks)?;

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
            RenderMode::Compact => {
                let frame = crate::render::build_ink(qr, cfg.border, cfg.invert, cfg.glyphs);
                crate::render::render(&frame, cfg.glyphs, out)?;
            }
            RenderMode::Ansi => {
                let frame = crate::render::build_ink(qr, cfg.border, cfg.invert, cfg.glyphs);
                crate::render::render_ansi(&frame, out)?;
            }
        }
        if total > 1 && index + 1 < total {
            out.write_all(b"\n")?;
        }
    }
    Ok(())
}

/// Image id used for the capability probe, kept well clear of the display range.
const KITTY_PROBE_ID: u32 = 1_000_000;

/// Ensure the current terminal implements the Kitty graphics protocol (FR-3.12).
///
/// The probe runs an `a=q` handshake over the controlling terminal, so the reply
/// is read without disturbing the payload stream (which may arrive on stdin). A
/// terminal that cannot draw the bitmap is a hard error: printing escape bytes it
/// cannot interpret would only spew garbage.
fn ensure_kitty_supported() -> Result<(), AppError> {
    match probe_kitty_terminal()? {
        Some(true) => Ok(()),
        Some(false) => Err(AppError::Usage(
            "kitty: this terminal does not support the Kitty graphics protocol \
             (try kitty, Ghostty, or WezTerm)"
                .to_string(),
        )),
        None => Err(AppError::Usage(
            "kitty: standard output is not a terminal".to_string(),
        )),
    }
}

/// Probe the controlling terminal for Kitty graphics support (FR-3.12).
///
/// Returns `Ok(None)` when there is no terminal to ask (standard output is not a
/// terminal); otherwise `Ok(Some(true/false))` from the `a=q` handshake.
fn probe_kitty_terminal() -> Result<Option<bool>, AppError> {
    if !std::io::stdout().is_terminal() {
        return Ok(None);
    }
    let mut tty = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .map_err(|source| AppError::Io {
            context: "kitty: cannot open the controlling terminal",
            source,
        })?;
    probe_kitty(&mut tty)
        .map(Some)
        .map_err(|source| AppError::Io {
            context: "kitty: cannot query the terminal",
            source,
        })
}

/// A one-line hint about the current terminal's Kitty support, for `--help`
/// (FR-3.12).
///
/// Runs the same `a=q` handshake as [`run_with`], but never fails: it returns
/// `None` when there is no terminal to probe or the probe errors, so the help
/// text stays truthful rather than guessing.
#[must_use]
pub fn kitty_support_hint() -> Option<String> {
    hint_for(probe_kitty_terminal().ok().flatten())
}

/// Map a probe result to the help hint; `None` means there was nothing to probe.
fn hint_for(support: Option<bool>) -> Option<String> {
    match support {
        Some(true) => Some("This terminal supports it.".to_string()),
        Some(false) => Some("This terminal does not support it.".to_string()),
        None => None,
    }
}

/// The clap command with a live Kitty support `hint` appended to the `--kitty`
/// option's help.
///
/// Used only when help is being shown, so the probe cost is paid once. The short
/// and long help both gain the hint.
#[must_use]
pub fn command_with_kitty_hint(hint: &str) -> clap::Command {
    Cli::command().mut_arg("kitty", |arg| {
        let short = arg.get_help().map(ToString::to_string).unwrap_or_default();
        let long = arg
            .get_long_help()
            .map(ToString::to_string)
            .unwrap_or_else(|| short.clone());
        // clap strips the trailing stop from the short help, so restore it before
        // appending, otherwise the two sentences run together.
        arg.help(format!("{} {hint}", end_with_period(short.trim_end())))
            .long_help(format!("{}\n\n{hint}", long.trim_end()))
    })
}

/// `text` with a trailing full stop, adding one only when it is missing.
fn end_with_period(text: &str) -> String {
    if text.ends_with('.') {
        text.to_string()
    } else {
        format!("{text}.")
    }
}

/// Send the support query and read the reply in raw mode, with a short timeout.
fn probe_kitty(tty: &mut std::fs::File) -> std::io::Result<bool> {
    use std::io::Read as _;
    use std::os::fd::AsFd as _;
    use std::time::{Duration, Instant};

    use rustix::termios::{OptionalActions, SpecialCodeIndex, tcgetattr, tcsetattr};

    let original = tcgetattr(tty.as_fd())?;
    // A duplicate handle restores the attributes on the way out without holding a
    // borrow of `tty`, which stays free for reading and writing.
    let _guard = TermiosGuard {
        tty: tty.try_clone()?,
        original: original.clone(),
    };

    let mut raw = original;
    raw.make_raw();
    raw.special_codes[SpecialCodeIndex::VMIN] = 0;
    raw.special_codes[SpecialCodeIndex::VTIME] = 1; // 0.1s between reads
    tcsetattr(tty.as_fd(), OptionalActions::Now, &raw)?;

    tty.write_all(&crate::kitty::query(KITTY_PROBE_ID))?;
    tty.flush()?;

    let deadline = Instant::now() + Duration::from_millis(300);
    let mut response = Vec::new();
    let mut byte = [0u8; 1];
    while Instant::now() < deadline {
        match tty.read(&mut byte) {
            Ok(0) => {} // the read timed out with no data
            Ok(_) => {
                response.push(byte[0]);
                if response.ends_with(b"\x1b\\") {
                    break;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(crate::kitty::response_ok(&response))
}

/// Restores the terminal's saved attributes when dropped, even on early return.
struct TermiosGuard {
    tty: std::fs::File,
    original: rustix::termios::Termios,
}

impl Drop for TermiosGuard {
    fn drop(&mut self) {
        use std::os::fd::AsFd as _;

        let _ = rustix::termios::tcsetattr(
            self.tty.as_fd(),
            rustix::termios::OptionalActions::Now,
            &self.original,
        );
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
        let cfg = parse(&["x", "--no-compact", "--invert", "--glyphs", "braille"]).to_config();
        assert_eq!(cfg.mode, RenderMode::Ansi);
        assert!(cfg.invert);
        assert_eq!(cfg.glyphs, GlyphSet::Braille);
        assert_eq!(cfg.border, 4);
        assert_eq!(cfg.max_size, None);
        assert_eq!(cfg.ec, Ec::L);
    }

    #[test]
    fn short_flags_resolve() {
        let cfg = parse(&[
            "-p", "-i", "-a", "-g", "half", "-e", "H", "-b", "2", "-m", "9", "x",
        ])
        .to_config();
        assert!(cfg.preserve_newline);
        assert!(cfg.invert);
        assert_eq!(cfg.mode, RenderMode::Ansi);
        assert_eq!(cfg.glyphs, GlyphSet::Half);
        assert_eq!(cfg.ec, Ec::H);
        assert_eq!(cfg.border, 2);
        assert_eq!(cfg.max_size, Some(9));
    }

    #[test]
    fn max_size_defaults_to_none() {
        assert_eq!(parse(&["x"]).to_config().max_size, None);
    }

    #[test]
    fn max_size_parses_long_and_short() {
        assert_eq!(
            parse(&["--max-size", "100", "x"]).to_config().max_size,
            Some(100)
        );
        assert_eq!(parse(&["-m", "100", "x"]).to_config().max_size, Some(100));
    }

    #[test]
    fn max_size_zero_is_rejected() {
        assert!(Cli::try_parse_from(["qrtxt", "--max-size", "0", "x"]).is_err());
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
            crate::encode::encode_multi(&vec![b'x'; 4000], cfg.ec, cfg.max_size, cfg.min_chunks)
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
            crate::encode::encode_multi(b"hello", cfg.ec, cfg.max_size, cfg.min_chunks).unwrap();
        assert_eq!(codes.len(), 1);
        let mut out = Vec::new();
        render_all(&cfg, &codes, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(!text.contains("QR "), "{text}");
    }

    #[test]
    fn render_all_kitty_renders_every_code_with_its_own_image_id() {
        // Regression: every code once shared one image id, so the terminal
        // replaced every bitmap but the last and the earlier codes looked
        // unrendered. Each code must now be transmitted as its own image with a
        // distinct id, in order.
        let cfg = parse(&["-k", "-e", "L", "x"]).to_config();
        let codes =
            crate::encode::encode_multi(&vec![b'x'; 6000], cfg.ec, cfg.max_size, cfg.min_chunks)
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
            "--no-compact",
            "--glyphs",
            "quadrant",
            "--error-correction",
            "Q",
            "--border",
            "1",
            "--max-size",
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
        assert_eq!(cfg.max_size, Some(7));
        assert_eq!(cfg.min_chunks, Some(4));
    }

    #[test]
    fn removed_options_are_rejected() {
        let cases: [&[&str]; 6] = [
            &["qrtxt", "--raw", "x"],
            &["qrtxt", "--ansi", "x"],
            &["qrtxt", "--ec", "x"],
            &["qrtxt", "--pad", "x"],
            &["qrtxt", "--size", "2", "x"],
            &["qrtxt", "--multi", "x"],
        ];
        for args in cases {
            assert!(
                Cli::try_parse_from(args).is_err(),
                "{args:?} should no longer be accepted"
            );
        }
    }

    #[test]
    fn kitty_flag_selects_kitty_mode() {
        assert_eq!(parse(&["--kitty", "x"]).to_config().mode, RenderMode::Kitty);
        assert_eq!(parse(&["-k", "x"]).to_config().mode, RenderMode::Kitty);
        assert_ne!(parse(&["x"]).to_config().mode, RenderMode::Kitty);
    }

    #[test]
    fn kitty_conflicts_with_no_compact() {
        assert!(Cli::try_parse_from(["qrtxt", "--kitty", "--no-compact", "x"]).is_err());
    }

    #[test]
    fn support_hint_reflects_the_probe_result() {
        // Tested through the pure mapping: the live probe depends on the ambient
        // terminal, which would make this test fail under an interactive one.
        assert_eq!(hint_for(None), None);
        assert!(hint_for(Some(true)).unwrap().contains("supports it"));
        assert!(hint_for(Some(false)).unwrap().contains("does not support"));
    }

    #[test]
    fn kitty_hint_is_appended_to_both_help_forms() {
        let hint = "This terminal supports the Kitty graphics protocol.";
        let mut command = command_with_kitty_hint(hint);

        let short = command.render_help().to_string();
        assert!(short.contains("--kitty"), "{short}");
        // The stripped stop is restored so the hint starts a new sentence.
        assert!(short.contains(". This terminal supports"), "{short}");

        let long = command.render_long_help().to_string();
        assert!(long.contains("--kitty"), "{long}");
        assert!(long.contains(hint), "{long}");
    }
}
