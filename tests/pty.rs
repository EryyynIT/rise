//! PTY tests: the interactive wizard driven over a real pseudo-terminal.
//!
//! The wizard's pure seams are unit-tested and the non-interactive paths
//! are pinned by `tests/cli.rs`; these scenarios cover the parts that only
//! exist with a TTY: the rendered prompts, the first-option defaults
//! picked with plain Enter, and Esc cancelling with exit code 130.
//!
//! Answers are sent with an explicit `\r`: crossterm maps `\n` to Enter
//! only outside raw mode (in raw mode it is Ctrl+J), and expectrl's
//! `send_line` appends `\n`.

#![cfg(unix)]

use std::{
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

use expectrl::{
    Expect,
    process::{Healthcheck, unix::WaitStatus},
    session::OsSession,
};

const ENTER: &[u8] = b"\r";

/// Spawn bare `rise` inside `dir`, attached to a pty (80x24 by default).
/// `RISE_CONFIG` points at an empty file: the interactive prompts must
/// come from the binary alone, never from a developer's own config.
fn rise_in(dir: &Path) -> OsSession {
    let empty_config = dir.join("empty-config.toml");
    std::fs::write(&empty_config, "").expect("write empty config");
    rise_with(dir, &empty_config)
}

/// Spawn bare `rise` inside `dir` with `RISE_CONFIG` set to `config`.
fn rise_with(dir: &Path, config: &Path) -> OsSession {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rise"));
    command.current_dir(dir).env("RISE_CONFIG", config);
    expectrl::Session::spawn(command).expect("spawn rise under a pty")
}

/// Block until the wizard exits; `get_status` is WNOHANG-style, so a
/// still-running wizard reports `StillAlive`.
fn exit_status(session: &mut OsSession) -> WaitStatus {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match session.get_process().get_status().expect("process status") {
            WaitStatus::StillAlive => {
                assert!(Instant::now() < deadline, "the wizard never exited");
                std::thread::sleep(Duration::from_millis(50));
            }
            status => return status,
        }
    }
}

#[test]
fn the_wizard_completes_over_a_pty() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut p = rise_in(tmp.path());

    // Enter-only answers walk the first option of every prompt: the web
    // kind, a backend-only shape, axum, sqlite (the zero-setup embedded
    // default), docker, no testing, no features — the same stack the
    // `--yes` defaults pin in tests/cli.rs.
    p.expect("name of your project").expect("name prompt");
    p.send(b"pty-app")
        .and_then(|_| p.send(ENTER))
        .expect("send the name");
    for needle in [
        "type of project",
        "webapp kind",
        "Select a backend",
        "Select a database",
        "deployment option",
        "testing options",
        "Select features",
    ] {
        p.expect(needle)
            .unwrap_or_else(|e| panic!("prompt `{needle}` did not appear: {e}"));
        p.send(ENTER)
            .unwrap_or_else(|e| panic!("answer to `{needle}` failed: {e}"));
    }

    p.expect("created ./pty-app")
        .expect("the wizard printed the result");
    let status = exit_status(&mut p);
    assert!(
        matches!(status, WaitStatus::Exited(_, 0)),
        "expected a clean exit, got {status:?}"
    );

    let manifest =
        std::fs::read_to_string(tmp.path().join("pty-app/Cargo.toml")).expect("read manifest");
    assert!(manifest.contains("axum"), "Enter picked the first backend");
    assert!(
        manifest.contains("\"sqlite\""),
        "Enter picked the first database"
    );
}

#[test]
fn the_config_file_preselects_the_prompts() {
    // A config preset turns Enter into "accept my defaults": rocket and
    // sled are NOT the first options, so only a working preselection
    // (inquire's starting cursor) generates them.
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = tmp.path().join("config.toml");
    std::fs::write(
        &config,
        "backend = \"rocket\"\ndb = \"sled\"\ndeployment = \"docker\"\n",
    )
    .expect("write config");
    let mut p = rise_with(tmp.path(), &config);

    p.expect("name of your project").expect("name prompt");
    p.send(b"preset-app")
        .and_then(|_| p.send(ENTER))
        .expect("send the name");
    for needle in [
        "type of project",
        "webapp kind",
        "Select a backend",
        "Select a database",
        "deployment option",
        "testing options",
        "Select features",
    ] {
        p.expect(needle)
            .unwrap_or_else(|e| panic!("prompt `{needle}` did not appear: {e}"));
        p.send(ENTER)
            .unwrap_or_else(|e| panic!("answer to `{needle}` failed: {e}"));
    }

    p.expect("created ./preset-app")
        .expect("the wizard printed the result");
    let status = exit_status(&mut p);
    assert!(
        matches!(status, WaitStatus::Exited(_, 0)),
        "expected a clean exit, got {status:?}"
    );

    let manifest =
        std::fs::read_to_string(tmp.path().join("preset-app/Cargo.toml")).expect("read manifest");
    assert!(
        manifest.contains("rocket"),
        "Enter accepted the config's backend"
    );
    assert!(manifest.contains("sled"), "Enter accepted the config's db");
}

#[test]
fn escape_cancels_the_wizard_with_130() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut p = rise_in(tmp.path());

    p.expect("name of your project").expect("name prompt");
    p.send(b"\x1b").expect("send Esc");

    let status = exit_status(&mut p);
    assert!(
        matches!(status, WaitStatus::Exited(_, 130)),
        "expected a clean 130 cancellation, got {status:?}"
    );
    assert!(
        !tmp.path().join("pty-app").exists(),
        "a cancelled run generates nothing"
    );
}
