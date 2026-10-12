use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    // Usage errors are reported by clap itself with exit code 2.
    match rise::run(rise::Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        // User cancellation is not an error: conventional 128 + SIGINT.
        Err(rise::Error::Aborted) => {
            eprintln!("aborted");
            ExitCode::from(130)
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}
