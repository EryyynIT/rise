//! User-wide wizard defaults from a config file.
//!
//! The file lives at `$RISE_CONFIG` when that variable is set, else at
//! `config.toml` inside the platform config dir (`dirs::config_dir()`:
//! `~/.config/rise/config.toml` on Linux). Every field is optional; a
//! missing file means "no defaults configured". Precedence is always:
//! CLI flag > config > built-in default. A file that exists but does not
//! read or parse is an error, not silence — a typo'd config that is
//! quietly ignored looks like rise ignoring the user.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::spec::{Backend, Database, Deployment, Feature, Frontend, Testing};

/// Wizard defaults for the web stack. All fields optional; unknown keys
/// are rejected so a typo fails loudly instead of being ignored.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    /// Default `--backend`.
    pub(crate) backend: Option<Backend>,
    /// Default `--frontend`.
    pub(crate) frontend: Option<Frontend>,
    /// Default `--db` (`database` accepted as an alias).
    #[serde(rename = "db", alias = "database")]
    pub(crate) database: Option<Database>,
    /// Default `--deployment`.
    pub(crate) deployment: Option<Deployment>,
    /// Default `--testing` selection.
    #[serde(default)]
    pub(crate) testing: Vec<Testing>,
    /// Default `--features` selection.
    #[serde(default)]
    pub(crate) features: Vec<Feature>,
}

/// Load the config file; `Ok(None)` when none exists. An explicitly
/// configured path (`RISE_CONFIG`) that is missing IS an error: the user
/// pointed somewhere, and a typo'd path must not fall back to anything.
pub(crate) fn load() -> Result<Option<Config>> {
    match std::env::var_os("RISE_CONFIG") {
        // An empty RISE_CONFIG (a script's unset variable) means unset.
        Some(path) if !path.is_empty() => load_from(Path::new(&path), true),
        Some(_) => Ok(None),
        None => match default_path() {
            Some(path) => load_from(&path, false),
            // No platform config dir (bare environment): no defaults.
            None => Ok(None),
        },
    }
}

/// The discovered config path: `<platform config dir>/rise/config.toml`.
fn default_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("rise").join("config.toml"))
}

/// Read and parse one config file. `missing_is_error` distinguishes an
/// explicitly requested path (error when absent) from discovery (absent
/// means unconfigured).
fn load_from(path: &Path, missing_is_error: bool) -> Result<Option<Config>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !missing_is_error => {
            return Ok(None);
        }
        Err(error) => return Err(config_error(path, error.to_string())),
    };
    toml::from_str(&text)
        .map(Some)
        .map_err(|error| config_error(path, error.to_string()))
}

/// Attach the file path to any load failure.
fn config_error(path: &Path, reason: String) -> Error {
    Error::Config {
        path: path.display().to_string(),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write `text` as config.toml inside a fresh tempdir.
    fn write_config(dir: &tempfile::TempDir, text: &str) -> PathBuf {
        let path = dir.path().join("config.toml");
        std::fs::write(&path, text).expect("write config");
        path
    }

    #[test]
    fn a_missing_discovered_file_is_not_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let config = load_from(&path, false).expect("missing file is fine");
        assert!(config.is_none());
    }

    #[test]
    fn a_missing_explicit_path_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let err = load_from(&path, true).expect_err("explicit path must exist");
        let Error::Config { path: named, .. } = err else {
            panic!("expected Error::Config");
        };
        assert!(named.contains("config.toml"));
    }

    #[test]
    fn an_empty_file_gives_no_defaults() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_config(&dir, "");
        let config = load_from(&path, true)
            .expect("empty parses")
            .expect("present");
        assert!(config.backend.is_none());
        assert!(config.testing.is_empty());
    }

    #[test]
    fn a_full_config_parses() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_config(
            &dir,
            r#"
backend = "actix"
frontend = "yew"
db = "sled"
deployment = "self-hosted"
testing = ["api"]
features = ["cors", "auth"]
"#,
        );
        let config = load_from(&path, true).expect("parses").expect("present");
        assert_eq!(config.backend, Some(Backend::Actix));
        assert_eq!(config.frontend, Some(Frontend::Yew));
        assert_eq!(config.database, Some(Database::Sled));
        assert_eq!(config.deployment, Some(Deployment::SelfHosted));
        assert_eq!(config.testing, vec![Testing::Api]);
        assert_eq!(config.features, vec![Feature::Cors, Feature::Auth]);
    }

    #[test]
    fn the_database_alias_parses() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_config(&dir, r#"database = "sea-orm""#);
        let config = load_from(&path, true).expect("parses").expect("present");
        assert_eq!(config.database, Some(Database::SeaORM));
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_config(&dir, r#"bakend = "axum""#);
        let err = load_from(&path, true).expect_err("typo must fail loudly");
        assert!(matches!(err, Error::Config { .. }));
    }

    #[test]
    fn unknown_values_are_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_config(&dir, r#"backend = "wordpress""#);
        let err = load_from(&path, true).expect_err("unknown backend must fail");
        assert!(matches!(err, Error::Config { .. }));
    }
}
