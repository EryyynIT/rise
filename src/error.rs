//! Typed errors for the `rise` library.

use inquire::error::InquireError;
use thiserror::Error;

/// Result alias for fallible `rise` operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur while collecting a [`crate::spec::ProjectSpec`]
/// or scaffolding a project.
#[derive(Debug, Error)]
pub enum Error {
    /// The user aborted an interactive prompt (Esc or Ctrl+C).
    ///
    /// This is not a failure: the binary maps it to a clean exit with
    /// code 130 (128 + SIGINT), per CLI convention.
    #[error("aborted by user")]
    Aborted,

    /// An interactive prompt failed (terminal I/O, stdin not a TTY, ...).
    #[error("prompt failed: {0}")]
    Prompt(#[source] InquireError),
}

impl From<InquireError> for Error {
    fn from(err: InquireError) -> Self {
        match err {
            InquireError::OperationCanceled | InquireError::OperationInterrupted => Error::Aborted,
            other => Error::Prompt(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inquire_cancellation_maps_to_aborted() {
        let canceled = Error::from(InquireError::OperationCanceled);
        let interrupted = Error::from(InquireError::OperationInterrupted);
        assert!(matches!(canceled, Error::Aborted));
        assert!(matches!(interrupted, Error::Aborted));
    }

    #[test]
    fn other_inquire_errors_stay_prompt_errors() {
        let err = Error::from(InquireError::NotTTY);
        assert!(matches!(err, Error::Prompt(_)));
    }
}
