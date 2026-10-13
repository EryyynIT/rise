//! End-to-end CLI tests: the real binary, real arguments, real filesystem.

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn list_prints_embedded_templates() {
    Command::cargo_bin("rise")
        .expect("binary")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("web-axum"));
}

#[test]
fn without_tty_missing_name_is_an_actionable_error() {
    // assert_cmd closes stdin by default, so this run is non-interactive
    // even when the test suite itself runs in a terminal.
    Command::cargo_bin("rise")
        .expect("binary")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("missing required option `NAME`"));
}

#[test]
fn new_generates_backend_project_without_prompts() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "sample-app",
            "--backend",
            "axum",
            "--db",
            "sqlx",
            "--deployment",
            "docker",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("created ./sample-app"));

    let project = tmp.path().join("sample-app");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("name = \"sample-app\""));
    assert!(project.join("src/main.rs").exists());
    assert!(project.join("Dockerfile").exists());
    assert!(project.join(".git").exists(), "git init ran");
}

#[test]
fn new_generates_actix_project_without_prompts() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "sample-actix",
            "--backend",
            "actix",
            "--db",
            "sqlx",
            "--deployment",
            "kubernetes",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("created ./sample-actix"));

    let project = tmp.path().join("sample-actix");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("actix-web"));
    let main_rs = std::fs::read_to_string(project.join("src/main.rs")).expect("read main.rs");
    assert!(main_rs.contains("Hello from sample-actix!"));
    assert!(
        !project.join("Dockerfile").exists(),
        "kubernetes excludes it"
    );
}

#[test]
fn new_reports_the_missing_flag_when_non_interactive() {
    Command::cargo_bin("rise")
        .expect("binary")
        .args(["new", "sample-app", "--backend", "axum", "--no-input"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing required option `--db`"));
}

#[test]
fn conflicting_flags_are_rejected_before_prompts() {
    Command::cargo_bin("rise")
        .expect("binary")
        .args(["new", "sample-app", "--kind", "cli", "--backend", "axum"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("conflicting options"));
}
