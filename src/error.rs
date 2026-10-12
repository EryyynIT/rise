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

    /// A project name failed crate-name validation.
    #[error("invalid project name `{name}`: {reason}")]
    InvalidProjectName { name: String, reason: &'static str },

    /// A non-interactive run is missing a required option.
    #[error(
        "missing required option {0} in a non-interactive run; pass the flag or run interactively"
    )]
    MissingOptions(String),

    /// CLI flags contradict each other.
    #[error("conflicting options: {0}")]
    ConflictingOptions(String),

    /// The stack described by the spec has no template yet.
    #[error("template `{0}` is not available yet")]
    TemplateNotFound(String),

    /// A template manifest (`template.toml`) failed to parse.
    #[error("invalid template manifest: {0}")]
    Manifest(#[from] toml::de::Error),

    /// Template rendering failed.
    #[error("render failed: {0}")]
    Render(#[from] minijinja::Error),

    /// The target directory already exists and is not empty.
    #[error("target directory `{0}` already exists and is not empty")]
    TargetNotEmpty(String),

    /// A filesystem operation failed.
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
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
