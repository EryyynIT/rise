//! Template loading: manifest parsing and file collection.
//!
//! Files ending in `.j2` are rendered through minijinja (the suffix is
//! stripped from the output name); all other files are copied verbatim.
//! File conditionality is data, not code: the manifest's `exclude_if` rules
//! hold minijinja expressions evaluated against the render context.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::str;

use include_dir::{Dir, include_dir};
use serde::Deserialize;

use crate::error::{Error, Result};

/// Built-in templates, embedded at compile time.
static EMBEDDED: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/templates");

/// Manifest file every template directory must contain.
const MANIFEST_FILE: &str = "template.toml";

/// Suffix of files rendered through minijinja; stripped from the output name.
pub(crate) const RENDER_SUFFIX: &str = ".j2";

/// A loaded template: parsed manifest plus in-memory files.
#[derive(Debug)]
pub(crate) struct Template {
    manifest: Manifest,
    files: Vec<TemplateFile>,
}

impl Template {
    /// Load a built-in template by directory name (e.g. `web-axum`).
    ///
    /// # Errors
    ///
    /// [`Error::TemplateNotFound`] if no embedded template has this name,
    /// [`Error::Manifest`] if its manifest is invalid.
    pub(crate) fn embedded(name: &str) -> Result<Self> {
        let dir = EMBEDDED
            .get_dir(name)
            .ok_or_else(|| Error::TemplateNotFound(name.to_owned()))?;

        let mut files = Vec::new();
        collect_embedded(dir, "", &mut files);
        let pos = files
            .iter()
            .position(|f| f.path == MANIFEST_FILE)
            .expect("embedded templates always contain template.toml");
        let manifest_file = files.remove(pos);
        let text = str::from_utf8(&manifest_file.contents)
            .expect("embedded manifests are committed as UTF-8");
        let manifest = toml::from_str(text)?;

        Ok(Self { manifest, files })
    }

    /// Load a template from a directory on disk (`--template`).
    ///
    /// # Errors
    ///
    /// [`Error::TemplateDirUnreadable`] on read failures (including a
    /// missing manifest), [`Error::Manifest`] on an invalid manifest.
    pub(crate) fn from_dir(root: &Path) -> Result<Self> {
        // Name the directory in every I/O failure: a bare "no such file"
        // leaves the user guessing which path `--template` was resolved to.
        let unreadable = |source: std::io::Error| Error::TemplateDirUnreadable {
            path: root.display().to_string(),
            source,
        };
        let manifest =
            toml::from_str(&fs::read_to_string(root.join(MANIFEST_FILE)).map_err(unreadable)?)?;

        let mut files = Vec::new();
        collect_fs(root, Path::new(""), &mut files).map_err(unreadable)?;
        files.retain(|f| f.path != MANIFEST_FILE);
        // Deterministic render order, independent of the filesystem's
        // directory iteration order (embedded templates are ordered at
        // compile time).
        files.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(Self { manifest, files })
    }

    /// The parsed manifest.
    pub(crate) fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Template files, excluding the manifest itself.
    pub(crate) fn files(&self) -> &[TemplateFile] {
        &self.files
    }
}

/// Parse the manifests of all embedded templates, sorted by name.
/// Internal templates (building blocks of multi-crate plans, not
/// user-selectable stacks) are hidden.
///
/// # Errors
///
/// [`Error::Manifest`] if an embedded manifest is invalid.
pub(crate) fn list_embedded() -> Result<Vec<Manifest>> {
    let mut manifests = Vec::new();
    for dir in EMBEDDED.dirs() {
        let name = dir
            .path()
            .file_name()
            .expect("include_dir paths always have a file name")
            .to_string_lossy();
        let manifest = Template::embedded(&name)?.manifest;
        if !manifest.internal {
            manifests.push(manifest);
        }
    }
    manifests.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(manifests)
}

/// Parsed `template.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    /// Template identifier, matching the directory name.
    pub name: String,
    /// Human-readable summary.
    pub description: String,
    /// Internal templates are rendered only as steps of a multi-crate
    /// plan; `rise list` hides them.
    #[serde(default)]
    pub internal: bool,
    /// File selection rules.
    #[serde(default)]
    pub files: FileRules,
}

/// Conditionality rules for template files.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FileRules {
    /// Output file path (i.e. after `.j2` is stripped) → minijinja
    /// expression. The file is excluded when the expression evaluates to
    /// true against the render context.
    #[serde(default)]
    pub exclude_if: BTreeMap<String, String>,
}

/// A single template file with raw (unrendered) contents.
#[derive(Debug)]
pub(crate) struct TemplateFile {
    /// `/`-separated path relative to the template root, e.g. `src/main.rs.j2`.
    path: String,
    /// Raw file contents.
    contents: Vec<u8>,
}

impl TemplateFile {
    /// Template-relative path, `/`-separated on every platform.
    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    /// Raw file contents.
    pub(crate) fn contents(&self) -> &[u8] {
        &self.contents
    }
}

/// Collect embedded files recursively, building `/`-separated relative paths.
fn collect_embedded(dir: &Dir<'_>, prefix: &str, files: &mut Vec<TemplateFile>) {
    for file in dir.files() {
        let name = file
            .path()
            .file_name()
            .expect("include_dir paths always have a file name")
            .to_string_lossy();
        files.push(TemplateFile {
            path: format!("{prefix}{name}"),
            contents: file.contents().to_vec(),
        });
    }
    for subdir in dir.dirs() {
        let name = subdir
            .path()
            .file_name()
            .expect("include_dir paths always have a file name")
            .to_string_lossy();
        collect_embedded(subdir, &format!("{prefix}{name}/"), files);
    }
}

/// Collect files from disk recursively, building `/`-separated relative
/// paths. Returns a plain I/O result so callers can attach path context.
fn collect_fs(root: &Path, rel: &Path, files: &mut Vec<TemplateFile>) -> std::io::Result<()> {
    for entry in fs::read_dir(root.join(rel))? {
        let entry = entry?;
        let entry_rel = rel.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            collect_fs(root, &entry_rel, files)?;
        } else {
            files.push(TemplateFile {
                path: to_slash_string(&entry_rel),
                contents: fs::read(entry.path())?,
            });
        }
    }
    Ok(())
}

/// `/`-separated path string, so manifests behave the same on every platform.
fn to_slash_string(path: &Path) -> String {
    path.iter()
        .map(|part| part.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest_with_exclude_rules() {
        let manifest: Manifest = toml::from_str(
            r#"
name = "web-axum"
description = "Backend on Axum + tokio"

[files.exclude_if]
Dockerfile = "spec.WebApp.Backend.deployment != 'docker'"
"#,
        )
        .expect("valid manifest");

        assert_eq!(manifest.name, "web-axum");
        assert_eq!(
            manifest
                .files
                .exclude_if
                .get("Dockerfile")
                .map(String::as_str),
            Some("spec.WebApp.Backend.deployment != 'docker'")
        );
    }

    #[test]
    fn manifest_rejects_unknown_fields() {
        let result = toml::from_str::<Manifest>("name = \"x\"\ndescription = \"y\"\nbogus = 1");
        assert!(result.is_err());
    }

    #[test]
    fn manifest_internal_defaults_to_false() {
        let manifest: Manifest =
            toml::from_str("name = \"x\"\ndescription = \"y\"").expect("valid manifest");
        assert!(!manifest.internal);

        let manifest: Manifest =
            toml::from_str("name = \"x\"\ndescription = \"y\"\ninternal = true")
                .expect("valid manifest");
        assert!(manifest.internal);
    }

    #[test]
    fn embedded_web_axum_loads() {
        let template = Template::embedded("web-axum").expect("web-axum is embedded");
        let paths: Vec<&str> = template.files().iter().map(|f| f.path()).collect();

        assert!(paths.contains(&"Cargo.toml.j2"));
        assert!(paths.contains(&"src/main.rs.j2"));
        assert!(paths.contains(&"src/db.rs.j2"));
        assert!(paths.contains(&".gitignore.j2"));
        assert!(paths.contains(&".env.example.j2"));
        assert!(!paths.contains(&"template.toml"));
    }

    #[test]
    fn unknown_embedded_template_is_not_found() {
        let err = Template::embedded("nope").expect_err("no such template");
        assert!(matches!(err, Error::TemplateNotFound(_)));
    }

    #[test]
    fn lists_embedded_template_manifests() {
        let manifests = list_embedded().expect("embedded manifests parse");
        let names: Vec<&str> = manifests.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "cli",
                "embedded-cortex-m",
                "frontend-leptos",
                "frontend-yew",
                "game-macroquad",
                "library",
                "web-actix",
                "web-axum",
                "web-leptos-actix",
                "web-leptos-axum"
            ]
        );
    }

    #[test]
    fn unreadable_template_dir_names_the_path() {
        let err = Template::from_dir(Path::new("/nonexistent/rise-template"))
            .expect_err("missing dir must fail");
        let Error::TemplateDirUnreadable { path, .. } = &err else {
            panic!("expected TemplateDirUnreadable, got {err:?}");
        };
        assert!(path.contains("rise-template"));
    }

    #[test]
    fn loads_template_from_disk() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(
            tmp.path().join("template.toml"),
            "name = \"t\"\ndescription = \"d\"",
        )
        .expect("write manifest");
        fs::create_dir(tmp.path().join("src")).expect("mkdir");
        fs::write(tmp.path().join("src/main.rs.j2"), "fn main() {}").expect("write file");

        let template = Template::from_dir(tmp.path()).expect("load template");
        let paths: Vec<&str> = template.files().iter().map(|f| f.path()).collect();

        assert_eq!(paths, ["src/main.rs.j2"]);
    }
}
