//! Argument-parsing entry point: build the [`Cli`], injecting the live `--mode`
//! support hint when help is being shown (FR-3.10, FR-3.12, FR-3.13).
//!
//! This is the only place `argv` becomes a [`Cli`], so every clap touch and the
//! help-detection heuristic stay inside `cli` and the binary stays a thin
//! exit-code mapper.

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
    if help_requested() {
        if let Some(hint) = mode_support_hint() {
            let command = add_mode_hint(Cli::command(), &hint);
            let matches = command.get_matches();
            return Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());
        }
    }
    match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => error.exit(),
    }
}

/// Whether the process arguments contain `-h`/`--help` before a `--` terminator.
fn help_requested() -> bool {
    wants_help(std::env::args_os().skip(1))
}

/// Whether `args` (argv without the program name) asks for help before the `--`
/// terminator.
///
/// The pure core of [`help_requested`], split out so it can be tested without
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
}
