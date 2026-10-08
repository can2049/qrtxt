//! Command-line interface: argument parsing (`Cli`) and orchestration (`run`).

use std::io::{IsTerminal, Write};

use clap::Parser;

use crate::error::AppError;
use crate::types::{Config, Ec, GlyphSet, InputSpec, RenderMode};

/// Turn text into a terminal QR code.
#[derive(Debug, Parser)]
#[command(name = "qrterm", version, about = "Turn text into a terminal QR code")]
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
    #[arg(long = "preserve-newline")]
    pub preserve_newline: bool,

    /// Exact error-correction level: L, M, Q, or H.
    #[arg(short = 'e', long = "error", value_name = "LEVEL", default_value = "M")]
    pub error: Ec,

    /// Quiet-zone width in modules.
    #[arg(short = 'b', long = "border", default_value_t = 4)]
    pub border: u32,

    /// Terminal module scale.
    #[arg(short = 's', long = "scale", default_value_t = 1)]
    pub scale: u32,

    /// Invert the ink mapping, for light-background terminals.
    #[arg(long = "invert")]
    pub invert: bool,

    /// Glyph set: half, quadrant, or braille.
    #[arg(long = "glyphs", value_name = "SET", default_value = "half")]
    pub glyphs: GlyphSet,

    /// Use ANSI rendering instead of Unicode block characters.
    #[arg(long = "no-compact")]
    pub no_compact: bool,
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
            scale: self.scale,
            invert: self.invert,
            glyphs: self.glyphs,
            mode: if self.no_compact {
                RenderMode::Ansi
            } else {
                RenderMode::Compact
            },
            preserve_newline: self.preserve_newline,
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
    let qr = crate::encode::encode(&payload, cfg.ec)?;
    let frame = crate::render::build_ink(&qr, cfg.border, cfg.scale, cfg.invert, cfg.glyphs);

    let mut writer = std::io::BufWriter::new(stdout);
    let rendered = match cfg.mode {
        RenderMode::Compact => crate::render::render(&frame, cfg.glyphs, &mut writer),
        RenderMode::Ansi => crate::render::render_ansi(&frame, &mut writer),
    };
    if let Err(error) = rendered {
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
        let mut argv = vec!["qrterm"];
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
        assert!(Cli::try_parse_from(["qrterm", "--file", "a.txt", "hello"]).is_err());
    }

    #[test]
    fn invalid_values_are_rejected() {
        assert!(Cli::try_parse_from(["qrterm", "--error", "Z", "x"]).is_err());
        assert!(Cli::try_parse_from(["qrterm", "--glyphs", "dense", "x"]).is_err());
    }

    #[test]
    fn defaults_and_flags() {
        let cfg = parse(&["x", "--no-compact", "--invert", "--glyphs", "braille"]).to_config();
        assert_eq!(cfg.mode, RenderMode::Ansi);
        assert!(cfg.invert);
        assert_eq!(cfg.glyphs, GlyphSet::Braille);
        assert_eq!(cfg.border, 4);
        assert_eq!(cfg.scale, 1);
        assert_eq!(cfg.ec, Ec::M);
    }
}
