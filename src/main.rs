use std::process::ExitCode;

fn main() -> ExitCode {
    match rise::run() {
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
