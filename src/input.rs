//! Input resolution and newline stripping (FR-1.*).

use std::io::Read;

use crate::error::AppError;
use crate::types::InputSpec;

/// Resolve `input` into raw bytes.
///
/// A trailing newline is stripped (exactly one) only for file/stdin input,
/// not for a literal positional argument (FR-1.1, FR-1.6); `preserve_newline`
/// keeps it.
pub fn resolve(
    input: &InputSpec,
    preserve_newline: bool,
    stdin: &mut dyn Read,
    stdin_is_tty: bool,
) -> Result<Vec<u8>, AppError> {
    let bytes = match input {
        InputSpec::Literal(text) => text.as_bytes().to_vec(),
        InputSpec::File(path) => {
            let data = std::fs::read(path).map_err(|source| AppError::Io {
                context: "cannot read input file",
                source,
            })?;
            maybe_strip(data, preserve_newline)
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
            maybe_strip(data, preserve_newline)
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

    fn literal(text: &str) -> InputSpec {
        InputSpec::Literal(text.to_string())
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
        let mut stdin = std::io::empty();
        assert_eq!(
            resolve(&literal("literal\n"), false, &mut stdin, false).unwrap(),
            b"literal\n"
        );
    }

    #[test]
    fn stdin_bytes_pass_through_and_strip() {
        let mut stdin: &[u8] = b"piped\n";
        assert_eq!(
            resolve(&InputSpec::Stdin, false, &mut stdin, false).unwrap(),
            b"piped"
        );
    }

    #[test]
    fn stdin_preserves_binary_and_newline_on_request() {
        let mut stdin: &[u8] = b"\xff\x00\r\n";
        assert_eq!(
            resolve(&InputSpec::Stdin, true, &mut stdin, false).unwrap(),
            b"\xff\x00\r\n"
        );
    }

    #[test]
    fn empty_stdin_is_an_input_error() {
        let mut stdin: &[u8] = b"\n";
        assert!(matches!(
            resolve(&InputSpec::Stdin, false, &mut stdin, false),
            Err(AppError::Input(_))
        ));
    }

    #[test]
    fn whitespace_only_is_valid_payload() {
        let mut stdin: &[u8] = b" \n";
        assert_eq!(
            resolve(&InputSpec::Stdin, false, &mut stdin, false).unwrap(),
            b" "
        );
    }

    #[test]
    fn tty_without_input_is_a_usage_error() {
        let mut stdin = std::io::empty();
        assert!(matches!(
            resolve(&InputSpec::Stdin, false, &mut stdin, true),
            Err(AppError::Usage(_))
        ));
    }
}
