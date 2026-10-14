//! Validated project names.

use std::fmt;
use std::str::FromStr;

use crate::error::{Error, Result};

/// A project name, validated once at the input boundary and trusted
/// afterwards.
///
/// A valid name doubles as a crate name: an ASCII letter or `_`, then
/// ASCII letters, digits, `-` or `_`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ProjectName(String);

impl ProjectName {
    /// Wrap `name` after validating it.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidProjectName`] if `name` is not a valid crate name.
    pub(crate) fn new(name: String) -> Result<Self> {
        if let Err(reason) = validate(&name) {
            return Err(Error::InvalidProjectName { name, reason });
        }
        Ok(Self(name))
    }

    /// Check that `name` is a valid crate name, with a static reason on
    /// failure. Shared by the flag parser ([`FromStr`]) and the wizard's
    /// prompt validator.
    pub(crate) fn validate(name: &str) -> std::result::Result<(), &'static str> {
        validate(name)
    }

    /// The name as a string slice.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProjectName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for ProjectName {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::new(s.to_owned())
    }
}

/// A valid crate name: an ASCII letter or `_`, then ASCII letters, digits,
/// `-` or `_`; and not a Rust keyword.
fn validate(name: &str) -> std::result::Result<(), &'static str> {
    match name.chars().next() {
        None => return Err("the name must not be empty"),
        Some(c) if !c.is_ascii_alphabetic() && c != '_' => {
            return Err("the name must start with an ASCII letter or `_`");
        }
        _ => {}
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("only ASCII letters, digits, `-` and `_` are allowed");
    }
    // Cargo accepts some keywords as package names at build time, but
    // crates.io refuses them and generated code puts the crate name into
    // use-statements (`use my_app::...`), where a keyword does not parse.
    if KEYWORDS.contains(&name) {
        return Err("the name is a reserved Rust keyword");
    }
    Ok(())
}

/// Strict and reserved Rust keywords, including `self`/`Self`/`super`/`crate`
/// and the edition-2024 additions `gen` and `try`.
const KEYWORDS: &[&str] = &[
    "Self", "abstract", "as", "async", "await", "become", "box", "break", "const", "continue",
    "crate", "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if",
    "impl", "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_crate_names() {
        for name in ["my-app", "my_app", "_x", "a", "App2"] {
            assert!(ProjectName::new(name.to_owned()).is_ok(), "{name}");
        }
    }

    #[test]
    fn rejects_invalid_crate_names() {
        for name in ["", "2cool", "-x", "my app", "my/app", "töst"] {
            assert!(
                matches!(
                    ProjectName::new(name.to_owned()),
                    Err(Error::InvalidProjectName { .. })
                ),
                "{name}"
            );
        }
    }

    #[test]
    fn rejects_rust_keywords() {
        for name in ["loop", "fn", "Self", "self", "crate", "super", "gen", "try"] {
            let Err(Error::InvalidProjectName { reason, .. }) = ProjectName::new(name.to_owned())
            else {
                panic!("{name} must be rejected");
            };
            assert_eq!(reason, "the name is a reserved Rust keyword");
        }
        // Keyword-flavoured but valid names still pass.
        for name in ["loops", "Loop", "my-loop", "generator"] {
            assert!(ProjectName::new(name.to_owned()).is_ok(), "{name}");
        }
    }

    #[test]
    fn parses_from_str() {
        let name: ProjectName = "demo".parse().expect("valid name");
        assert_eq!(name.as_str(), "demo");
        assert_eq!(name.to_string(), "demo");
    }
}
