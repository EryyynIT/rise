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
        .stdout(predicate::str::contains("created ./sample-app"))
        .stdout(predicate::str::contains(".env.example"));

    let project = tmp.path().join("sample-app");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("name = \"sample-app\""));
    assert!(project.join("src/main.rs").exists());
    assert!(project.join("Dockerfile").exists());
    assert!(project.join(".git").exists(), "git init ran");

    // --db sqlx wires the full database stack.
    assert!(cargo_toml.contains("sqlx"));
    assert!(project.join("src/db.rs").exists());
    assert!(project.join(".env.example").exists());
    let gitignore = std::fs::read_to_string(project.join(".gitignore")).expect("read .gitignore");
    assert!(gitignore.contains(".env"));
}

#[test]
fn new_with_non_sqlx_database_adds_only_the_dependency() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "diesel-app",
            "--backend",
            "axum",
            "--db",
            "diesel",
            "--deployment",
            "self-hosted",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let project = tmp.path().join("diesel-app");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("diesel"));
    assert!(!cargo_toml.contains("sqlx"));
    assert!(!project.join("src/db.rs").exists(), "sqlx-only content");
    assert!(!project.join(".env.example").exists(), "sqlx-only content");
}

#[test]
fn new_with_testing_adds_test_module_and_dev_dependencies() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "tested-app",
            "--backend",
            "axum",
            "--db",
            "sqlx",
            "--deployment",
            "docker",
            "--testing",
            "api,sqlx,mock",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let project = tmp.path().join("tested-app");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("[dev-dependencies]"));
    assert!(cargo_toml.contains("tower"));
    assert!(cargo_toml.contains("mockall"));
    let main_rs = std::fs::read_to_string(project.join("src/main.rs")).expect("read main.rs");
    assert!(main_rs.contains("mod tests"));
    assert!(main_rs.contains("mod db_tests"));
}

#[test]
fn new_with_websockets_adds_ws_endpoint() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "ws-app",
            "--backend",
            "axum",
            "--db",
            "sqlx",
            "--deployment",
            "docker",
            "--features",
            "web-sockets",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let project = tmp.path().join("ws-app");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("features = [\"ws\"]"));
    let main_rs = std::fs::read_to_string(project.join("src/main.rs")).expect("read main.rs");
    assert!(main_rs.contains("route(\"/ws\", get(ws_handler))"));
}

#[test]
fn sqlx_testing_without_sqlx_database_is_rejected() {
    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "sample-app",
            "--backend",
            "axum",
            "--db",
            "diesel",
            "--testing",
            "sqlx",
            "--no-input",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "`--testing sqlx` requires `--db sqlx`",
        ));
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
fn new_generates_cli_project_without_prompts() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args(["new", "greeter", "--kind", "cli", "--no-input"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("created ./greeter"))
        .stdout(predicate::str::contains("cargo run"));

    let project = tmp.path().join("greeter");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("name = \"greeter\""));
    assert!(cargo_toml.contains("clap"));
    assert!(project.join("src/main.rs").exists());
    assert!(project.join(".git").exists(), "git init ran");
}

#[test]
fn new_generates_library_project_without_prompts() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args(["new", "mathlib", "--kind", "library", "--no-input"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("created ./mathlib"))
        .stdout(predicate::str::contains("cargo test"));

    let project = tmp.path().join("mathlib");
    let lib_rs = std::fs::read_to_string(project.join("src/lib.rs")).expect("read lib.rs");
    assert!(lib_rs.contains("mathlib"));
    assert!(lib_rs.contains("#[cfg(test)]"));
    assert!(!project.join("src/main.rs").exists());
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
