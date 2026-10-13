//! Argument-parsing entry point: build the [`Cli`], injecting the live `--mode`
//! support hint when help is being shown (FR-3.10, FR-3.12, FR-3.13).
//!
//! This is the only place `argv` becomes a [`Cli`], so every clap touch and the
//! help-detection heuristic stay inside `cli` and the binary stays a thin
//! exit-code mapper. It also rewrites the argv so the flat invocations
//! (`qrtxt hello`, `qrtxt -e H hi`) keep working now that formats are subcommands.

use std::ffi::OsString;

use clap::{CommandFactory, FromArgMatches, Parser};

use super::Cli;
use super::help::{add_mode_hint, mode_support_hint};

/// Parse the process arguments into a [`Cli`]. clap prints usage errors (code 2)
/// and `--help`/`--version` (code 0) itself.
///
/// When help is being shown, the `--mode` option gains a live hint about what
/// `auto` will pick on the current terminal. The probe only runs for help, so a
/// normal invocation is unaffected; a piped `--help` yields no hint (no terminal).
pub fn parse() -> Cli {
    let args = inject_default_subcommand(std::env::args_os().collect());
    if wants_help(args.iter().skip(1).cloned()) {
        if let Some(hint) = mode_support_hint() {
            let command = add_mode_hint(Cli::command(), &hint);
            let matches = command.get_matches_from(args.iter().cloned());
            return Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());
        }
    }
    match Cli::try_parse_from(args.iter().cloned()) {
        Ok(cli) => cli,
        Err(error) => error.exit(),
    }
}

/// Prepend `qr` when the invocation names no subcommand, so the flat forms
/// (`qrtxt hello`, `qrtxt -e H hi`, and a bare `cat token.txt | qrtxt`) keep
/// working now that formats are subcommands. A real subcommand or a top-level
/// flag (`-h`, `--version`) is left untouched.
fn inject_default_subcommand(mut args: Vec<OsString>) -> Vec<OsString> {
    let first = args.get(1).and_then(|arg| arg.to_str());
    let keep = matches!(
        first,
        Some(
            "qr" | "datamatrix"
                | "dm"
                | "pdf417"
                | "pdf"
                | "help"
                | "-h"
                | "--help"
                | "-V"
                | "--version"
        )
    );
    if !keep {
        args.insert(1, OsString::from("qr"));
    }
    args
}

/// Whether `args` (argv without the program name) asks for help before the `--`
/// terminator.
///
/// The pure core of the help detection, split out so it can be tested without
/// touching the real process arguments.
fn wants_help<I>(args: I) -> bool
where
    I: IntoIterator<Item = OsString>,
{
    for arg in args {
        match arg.to_str() {
            Some("--") => break,
            Some("-h" | "--help") => return true,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<OsString> {
        items.iter().map(OsString::from).collect()
    }

    /// `inject_default_subcommand` applied to `items`, as lossy strings.
    fn rewritten(items: &[&str]) -> Vec<String> {
        inject_default_subcommand(args(items))
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn wants_help_detects_the_flags() {
        assert!(wants_help(args(&["--help"])));
        assert!(wants_help(args(&["-h"])));
        assert!(wants_help(args(&["hello", "-h"])));
        assert!(wants_help(args(&["--mode", "kitty", "--help"])));
    }

    #[test]
    fn wants_help_ignores_arguments_after_the_terminator() {
        assert!(!wants_help(args(&["--", "--help"])));
        assert!(!wants_help(args(&["--", "-h"])));
    }

    #[test]
    fn wants_help_is_false_without_a_flag() {
        assert!(!wants_help(args(&[])));
        assert!(!wants_help(args(&["hello", "--file", "x"])));
    }

    #[test]
    fn injects_qr_for_a_bare_payload() {
        assert_eq!(rewritten(&["qrtxt", "hello"]), ["qrtxt", "qr", "hello"]);
    }

    #[test]
    fn injects_qr_for_a_leading_flag() {
        assert_eq!(
            rewritten(&["qrtxt", "-e", "H", "hi"]),
            ["qrtxt", "qr", "-e", "H", "hi"]
        );
        assert_eq!(
            rewritten(&["qrtxt", "--mode", "ansi", "hi"]),
            ["qrtxt", "qr", "--mode", "ansi", "hi"]
        );
    }

    #[test]
    fn keeps_a_real_subcommand() {
        for items in [
            &["qrtxt", "qr", "hello"][..],
            &["qrtxt", "datamatrix", "x"][..],
            &["qrtxt", "dm", "x"][..],
            &["qrtxt", "pdf417", "x"][..],
            &["qrtxt", "pdf", "x"][..],
            &["qrtxt", "help"][..],
        ] {
            let expected: Vec<String> = items.iter().map(|arg| arg.to_string()).collect();
            assert_eq!(rewritten(items), expected, "{items:?}");
        }
    }

    #[test]
    fn injects_qr_for_an_empty_argv() {
        // `cat token.txt | qrtxt` must keep encoding stdin as a QR code.
        assert_eq!(rewritten(&["qrtxt"]), ["qrtxt", "qr"]);
    }

    #[test]
    fn keeps_top_level_flags() {
        assert_eq!(rewritten(&["qrtxt", "--help"]), ["qrtxt", "--help"]);
        assert_eq!(rewritten(&["qrtxt", "-V"]), ["qrtxt", "-V"]);
    }
}
