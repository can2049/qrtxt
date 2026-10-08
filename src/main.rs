//! Thin binary entry point: parse arguments, run, map the result to an exit code.

use std::process::ExitCode;

use clap::Parser;

use qrtxt::cli::{Cli, run_with};

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        // clap prints usage errors (code 2) and --help/--version (code 0) itself.
        Err(error) => error.exit(),
    };

    match run_with(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("qrtxt: {error}");
            ExitCode::from(u8::try_from(error.exit_code()).unwrap_or(1))
        }
    }
}
