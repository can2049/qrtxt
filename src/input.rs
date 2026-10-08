//! Input resolution and newline stripping (FR-1.*).

use std::io::Read;

use crate::error::AppError;
use crate::types::{Config, InputSpec};

/// Resolve the configured input source into raw bytes.
///
/// A trailing newline is stripped (exactly one) only for file/stdin input,
/// not for a literal positional argument (FR-1.1, FR-1.6).
pub fn resolve(
    cfg: &Config,
    stdin: &mut dyn Read,
    stdin_is_tty: bool,
) -> Result<Vec<u8>, AppError> {
    let bytes = match &cfg.input {
        InputSpec::Literal(text) => text.as_bytes().to_vec(),
        InputSpec::File(path) => {
            let data = std::fs::read(path).map_err(|source| AppError::Io {
                context: "cannot read input file",
                source,
            })?;
            maybe_strip(data, cfg.preserve_newline)
        }
        InputSpec::Stdin => {
            // FR-1.8: an interactive terminal with no other source: fail fast.
            if stdin_is_tty {
                return Err(AppError::Usage(
                    "no input; provide DATA, --file, or pipe data on stdin".to_string(),
                ));
            }
            let mut data = Vec::new();
            stdin
                .read_to_end(&mut data)
                .map_err(|source| AppError::Io {
                    context: "cannot read stdin",
                    source,
                })?;
            maybe_strip(data, cfg.preserve_newline)
        }
    };

    if bytes.is_empty() {
        return Err(AppError::Input(
            "input is empty; pass DATA, --file, or pipe data on stdin".to_string(),
        ));
    }
    Ok(bytes)
}

fn maybe_strip(mut bytes: Vec<u8>, preserve: bool) -> Vec<u8> {
    if !preserve {
        bytes = strip_one_line_ending(bytes);
    }
    bytes
}

/// Remove exactly one trailing `LF` or `CRLF` sequence (FR-1.6).
#[must_use]
pub fn strip_one_line_ending(mut bytes: Vec<u8>) -> Vec<u8> {
    if bytes.ends_with(b"\r\n") {
        bytes.truncate(bytes.len() - 2);
    } else if bytes.ends_with(b"\n") {
        bytes.truncate(bytes.len() - 1);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Ec, GlyphSet, RenderMode};

    fn literal(text: &str, preserve_newline: bool) -> Config {
        Config {
            input: InputSpec::Literal(text.to_string()),
            ec: Ec::M,
            border: 4,
            size: 1,
            invert: false,
            glyphs: GlyphSet::Half,
            mode: RenderMode::Compact,
            preserve_newline,
        }
    }

    #[test]
    fn strips_exactly_one_line_ending() {
        assert_eq!(strip_one_line_ending(b"payload\n".to_vec()), b"payload");
        assert_eq!(strip_one_line_ending(b"payload\r\n".to_vec()), b"payload");
        assert_eq!(strip_one_line_ending(b"payload\n\n".to_vec()), b"payload\n");
        assert_eq!(strip_one_line_ending(b"payload".to_vec()), b"payload");
    }

    #[test]
    fn literal_is_not_newline_stripped() {
        let cfg = literal("literal\n", false);
        let mut stdin = std::io::empty();
        assert_eq!(resolve(&cfg, &mut stdin, false).unwrap(), b"literal\n");
    }

    #[test]
    fn stdin_bytes_pass_through_and_strip() {
        let cfg = Config {
            input: InputSpec::Stdin,
            ..literal("", false)
        };
        let mut stdin: &[u8] = b"piped\n";
        assert_eq!(resolve(&cfg, &mut stdin, false).unwrap(), b"piped");
    }

    #[test]
    fn stdin_preserves_binary_and_newline_on_request() {
        let cfg = Config {
            input: InputSpec::Stdin,
            preserve_newline: true,
            ..literal("", false)
        };
        let mut stdin: &[u8] = b"\xff\x00\r\n";
        assert_eq!(resolve(&cfg, &mut stdin, false).unwrap(), b"\xff\x00\r\n");
    }

    #[test]
    fn empty_stdin_is_an_input_error() {
        let cfg = Config {
            input: InputSpec::Stdin,
            ..literal("", false)
        };
        let mut stdin: &[u8] = b"\n";
        assert!(matches!(
            resolve(&cfg, &mut stdin, false),
            Err(AppError::Input(_))
        ));
    }

    #[test]
    fn whitespace_only_is_valid_payload() {
        let cfg = Config {
            input: InputSpec::Stdin,
            ..literal("", false)
        };
        let mut stdin: &[u8] = b" \n";
        assert_eq!(resolve(&cfg, &mut stdin, false).unwrap(), b" ");
    }

    #[test]
    fn tty_without_input_is_a_usage_error() {
        let cfg = Config {
            input: InputSpec::Stdin,
            ..literal("", false)
        };
        let mut stdin = std::io::empty();
        assert!(matches!(
            resolve(&cfg, &mut stdin, true),
            Err(AppError::Usage(_))
        ));
    }
}
