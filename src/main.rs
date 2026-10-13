//! Thin binary entry point: parse arguments, run, map the result to an exit code.

use std::process::ExitCode;

use qrtxt::cli::{parse, run_with};

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
