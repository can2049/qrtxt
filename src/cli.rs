//! Command-line interface: argument parsing (`Cli`) and orchestration (`run`).

use std::io::{IsTerminal, Write};

use clap::Parser;
use qrcode::QrCode;

use crate::error::AppError;
use crate::types::{Config, Ec, GlyphSet, InputSpec, RenderMode};

/// Turn text into a terminal QR code.
#[derive(Debug, Parser)]
#[command(name = "qrtxt", version, about = "Turn text into a terminal QR code")]
pub struct Cli {
    /// Literal payload; when omitted, read from --file or standard input.
    #[arg(value_name = "DATA")]
    pub data: Option<String>,

    /// Read the payload from a file.
    #[arg(
        short = 'f',
        long = "file",
        value_name = "PATH",
        conflicts_with = "data"
    )]
    pub file: Option<std::path::PathBuf>,

    /// Do not strip one trailing newline from piped or file input.
    #[arg(short = 'p', long = "preserve-newline")]
    pub preserve_newline: bool,

    /// Exact error-correction level: L, M, Q, or H.
    #[arg(
        short = 'e',
        long = "error-correction",
        value_name = "LEVEL",
        default_value = "L"
    )]
    pub error: Ec,

    /// Quiet-zone width in modules.
    #[arg(short = 'b', long = "border", default_value_t = 4)]
    pub border: u32,

    /// Invert the ink mapping, for light-background terminals.
    #[arg(short = 'i', long = "invert")]
    pub invert: bool,

    /// Glyph set: half (h), quadrant (q), or braille (b).
    #[arg(
        short = 'g',
        long = "glyphs",
        value_name = "SET",
        default_value = "half"
    )]
    pub glyphs: GlyphSet,

    /// Use ANSI rendering instead of Unicode block characters.
    #[arg(short = 'a', long = "no-compact")]
    pub no_compact: bool,

    /// Cap the payload in each QR code at BYTES (implies splitting).
    #[arg(
        short = 'm',
        long = "max-size",
        value_name = "BYTES",
        value_parser = clap::value_parser!(u32).range(1..)
    )]
    pub max_size: Option<u32>,

    /// Split the payload into at least COUNT QR codes when the content allows.
    #[arg(
        short = 'c',
        long = "chunk",
        value_name = "COUNT",
        value_parser = clap::value_parser!(u32).range(1..)
    )]
    pub chunk: Option<u32>,
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
            mode: if self.no_compact {
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
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    let stdin_is_tty = std::io::stdin().is_terminal();
    run(&cli.to_config(), &mut stdin, &mut stdout, stdin_is_tty)
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

/// Draw every code; when there is more than one, caption each and separate them.
fn render_all(cfg: &Config, codes: &[QrCode], out: &mut dyn std::io::Write) -> std::io::Result<()> {
    let total = codes.len();
    for (index, qr) in codes.iter().enumerate() {
        if total > 1 {
            writeln!(out, "QR {}/{}", index + 1, total)?;
        }
        let frame = crate::render::build_ink(qr, cfg.border, cfg.invert, cfg.glyphs);
        match cfg.mode {
            RenderMode::Compact => crate::render::render(&frame, cfg.glyphs, out)?,
            RenderMode::Ansi => crate::render::render_ansi(&frame, out)?,
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
}
