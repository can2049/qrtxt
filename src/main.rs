//! Thin binary entry point: parse arguments, run, map the result to an exit code.

use std::process::ExitCode;

use clap::{FromArgMatches, Parser};

use qrtxt::cli::{Cli, command_with_kitty_hint, kitty_support_hint, run_with};

fn main() -> ExitCode {
    let cli = parse();

    match run_with(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("qrtxt: {error}");
            ExitCode::from(u8::try_from(error.exit_code()).unwrap_or(1))
        }
    }
}

/// Parse the arguments. clap prints usage errors (code 2) and `--help`/
/// `--version` (code 0) itself.
///
/// When help is being shown, the `--kitty` option gains a live hint about whether
/// the current terminal supports the protocol. The probe only runs for help, so a
/// normal invocation is unaffected; a piped `--help` yields no hint (no terminal).
fn parse() -> Cli {
    if help_requested() {
        if let Some(hint) = kitty_support_hint() {
            let matches = command_with_kitty_hint(&hint).get_matches();
            return Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());
        }
    }
    match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => error.exit(),
    }
}

/// Whether `-h`/`--help` appears before a `--` terminator.
fn help_requested() -> bool {
    for arg in std::env::args_os().skip(1) {
        match arg.to_str() {
            Some("--") => break,
            Some("-h" | "--help") => return true,
            _ => {}
        }
    }
    false
}
