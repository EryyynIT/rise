//! `rise doctor`: check that the prerequisites of the generated stacks
//! are installed, with install hints for whatever is missing.

use colored::Colorize;

use crate::Error;

/// One prerequisite of one or more generated stacks.
struct Check {
    /// The report section the check belongs to; the report groups by it,
    /// so a gap reads as "only matters if you use that stack", not as a
    /// failure.
    group: &'static str,
    /// What the report calls it.
    name: &'static str,
    /// How to detect it.
    probe: Probe,
    /// How to install or update it; mirrors the one-time setup hints the
    /// generator prints as next steps.
    install_hint: &'static str,
}

/// How a check detects its prerequisite.
enum Probe {
    /// Run `<program> --version`; present when the run succeeds.
    Version(&'static str),
    /// Run `<program> --version` and parse the semver out of it; present
    /// when the run succeeds, outdated when the version is below `min`.
    VersionAtLeast {
        program: &'static str,
        min: (u32, u32, u32),
    },
    /// Present when `rustup target list --installed` names the target.
    RustupTarget(&'static str),
}

/// The prerequisites of everything rise can generate, in report order:
/// the report prints a section header whenever the group changes.
const CHECKS: &[Check] = &[
    Check {
        group: "core — every generated project",
        name: "git",
        probe: Probe::Version("git"),
        install_hint: "https://git-scm.com/downloads",
    },
    Check {
        group: "core — every generated project",
        name: "rustc",
        // Generated projects are edition 2024, which needs rustc 1.85+.
        probe: Probe::VersionAtLeast {
            program: "rustc",
            min: (1, 85, 0),
        },
        install_hint: "rustup update stable",
    },
    Check {
        group: "docker deployment",
        name: "docker",
        probe: Probe::Version("docker"),
        install_hint: "https://docs.docker.com/get-docker/",
    },
    Check {
        group: "yew/leptos frontends",
        name: "trunk",
        probe: Probe::Version("trunk"),
        install_hint: "cargo install trunk",
    },
    Check {
        group: "yew/leptos frontends",
        name: "wasm32-unknown-unknown",
        probe: Probe::RustupTarget("wasm32-unknown-unknown"),
        install_hint: "rustup target add wasm32-unknown-unknown",
    },
    Check {
        group: "leptos fullstack",
        name: "cargo-leptos",
        probe: Probe::Version("cargo-leptos"),
        install_hint: "cargo install --locked cargo-leptos",
    },
    Check {
        group: "the grpc feature",
        name: "protoc",
        probe: Probe::Version("protoc"),
        install_hint: "https://grpc.io/docs/protoc-installation/",
    },
    Check {
        group: "embedded firmware",
        name: "probe-rs",
        probe: Probe::Version("probe-rs"),
        install_hint: "cargo install probe-rs-tools --locked",
    },
    Check {
        group: "embedded firmware",
        name: "thumbv7em-none-eabihf",
        probe: Probe::RustupTarget("thumbv7em-none-eabihf"),
        install_hint: "rustup target add thumbv7em-none-eabihf",
    },
];

/// What one check found.
enum Status {
    /// Installed; carries a short version string (empty for rustup
    /// targets, which have no version line).
    Present(String),
    /// Not installed.
    Missing,
    /// Installed but older than the check requires.
    Outdated { found: String, min: (u32, u32, u32) },
    /// Could not be determined, e.g. a rustup target with no rustup.
    Unknown(&'static str),
}

/// `rise doctor`: probe every prerequisite and print the report.
///
/// Always exits successfully: a missing prerequisite is a fact about the
/// machine, not a failure of this command — it only matters for the
/// stack that needs it.
pub(crate) fn run() -> Result<(), Error> {
    let targets = probe_installed_targets();
    let statuses = evaluate(CHECKS, &probe_version, targets.as_deref());
    crate::print_out(report(CHECKS, &statuses).as_bytes())
}

/// Run every check. Both probes are injectable so the report logic is
/// testable without depending on what this machine has installed.
fn evaluate(
    checks: &[Check],
    version_of: &dyn Fn(&str) -> Option<String>,
    installed_targets: Option<&[String]>,
) -> Vec<Status> {
    checks
        .iter()
        .map(|check| match check.probe {
            Probe::Version(program) => match version_of(program) {
                Some(version) => Status::Present(version),
                None => Status::Missing,
            },
            Probe::VersionAtLeast { program, min } => match version_of(program) {
                Some(version) => match parse_semver(&version) {
                    Some(found) if found >= min => Status::Present(format_version(found)),
                    Some(found) => Status::Outdated {
                        found: format_version(found),
                        min,
                    },
                    None => Status::Unknown("could not parse the version"),
                },
                None => Status::Missing,
            },
            Probe::RustupTarget(target) => match installed_targets {
                Some(installed) if installed.iter().any(|installed| installed == target) => {
                    Status::Present(String::new())
                }
                Some(_) => Status::Missing,
                None => Status::Unknown("could not query rustup"),
            },
        })
        .collect()
}

/// Extract the first `major.minor.patch` triple from a `--version` line
/// (`rustc 1.97.1 (abc 2026-01-01)` → `(1, 97, 1)`).
fn parse_semver(version_line: &str) -> Option<(u32, u32, u32)> {
    let token = version_line
        .split_whitespace()
        .find(|token| token.chars().next().is_some_and(|c| c.is_ascii_digit()))?;
    let mut parts = token.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}

/// `1.85.0` out of a triple.
fn format_version(version: (u32, u32, u32)) -> String {
    format!("{}.{}.{}", version.0, version.1, version.2)
}

/// Render the report: a section per stack, one line per check, an install
/// hint under every gap, and a summary.
fn report(checks: &[Check], statuses: &[Status]) -> String {
    let mut out = String::from("rise doctor — prerequisites of the generated stacks\n");
    let mut present = 0;
    let mut unknown = 0;
    let mut group = "";
    for (check, status) in checks.iter().zip(statuses) {
        if check.group != group {
            group = check.group;
            out.push_str(&format!("\n{group}:\n"));
        }
        let version = match status {
            Status::Present(version) => {
                present += 1;
                if version.is_empty() {
                    String::new()
                } else {
                    format!(" ({version})")
                }
            }
            Status::Outdated { found, .. } => format!(" ({found})"),
            _ => String::new(),
        };
        // Pad before coloring: the escape codes must not count toward the
        // column width.
        let word = match status {
            Status::Present(_) => format!("{:>8}", "ok").green(),
            Status::Missing => format!("{:>8}", "missing").red(),
            Status::Outdated { .. } => format!("{:>8}", "outdated").red(),
            Status::Unknown(_) => {
                unknown += 1;
                format!("{:>8}", "unknown").yellow()
            }
        };
        out.push_str(&format!("  {word} {}{version}\n", check.name));
        match status {
            Status::Missing => {
                out.push_str(&format!("           install: {}\n", check.install_hint));
            }
            Status::Outdated { min, .. } => {
                out.push_str(&format!(
                    "           update: {} (need ≥ {})\n",
                    check.install_hint,
                    format_version(*min)
                ));
            }
            Status::Unknown(reason) => {
                out.push_str(&format!("           cannot check: {reason}\n"));
            }
            Status::Present(_) => {}
        }
    }
    if present == checks.len() {
        out.push_str("\nevery prerequisite is installed\n");
    } else {
        out.push_str(&format!("\n{present} of {} checks passed", checks.len()));
        if unknown > 0 {
            out.push_str(&format!(" ({unknown} could not be checked)"));
        }
        out.push_str(" — a gap only matters for the stack that needs it\n");
    }
    out
}

/// Run `<program> --version`; the first stdout line when it runs
/// successfully, `None` when the program is not installed (or fails).
fn probe_version(program: &str) -> Option<String> {
    let output = std::process::Command::new(program)
        .arg("--version")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned(),
    )
}

/// The rustup targets installed on this machine; `None` when rustup is
/// not around, so target checks report "unknown" rather than "missing".
fn probe_installed_targets() -> Option<Vec<String>> {
    let output = std::process::Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|line| line.trim().to_owned())
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_version(_: &str) -> Option<String> {
        Some("tool 1.90.0".to_owned())
    }

    #[test]
    fn everything_installed_reports_success() {
        let targets = vec![
            "wasm32-unknown-unknown".to_owned(),
            "thumbv7em-none-eabihf".to_owned(),
        ];
        let statuses = evaluate(CHECKS, &every_version, Some(&targets));
        let out = report(CHECKS, &statuses);
        assert!(out.contains("every prerequisite is installed"));
        for check in CHECKS {
            assert!(out.contains(check.name), "report names {}", check.name);
        }
    }

    #[test]
    fn the_report_groups_by_stack() {
        let targets = vec![
            "wasm32-unknown-unknown".to_owned(),
            "thumbv7em-none-eabihf".to_owned(),
        ];
        let statuses = evaluate(CHECKS, &every_version, Some(&targets));
        let out = report(CHECKS, &statuses);
        for group in [
            "core — every generated project:",
            "docker deployment:",
            "yew/leptos frontends:",
            "leptos fullstack:",
            "the grpc feature:",
            "embedded firmware:",
        ] {
            assert!(out.contains(group), "report has the section {group}");
        }
    }

    #[test]
    fn a_missing_tool_gets_its_install_hint() {
        let none_installed: Vec<String> = Vec::new();
        let only_trunk = |program: &str| (program == "trunk").then(|| "trunk 0.21".to_owned());
        let statuses = evaluate(CHECKS, &only_trunk, Some(&none_installed));
        let out = report(CHECKS, &statuses);
        assert!(out.contains("trunk 0.21"));
        assert!(out.contains("install: cargo install --locked cargo-leptos"));
        assert!(out.contains("install: rustup target add wasm32-unknown-unknown"));
        assert!(out.contains("install: https://docs.docker.com/get-docker/"));
        assert!(out.contains("1 of 9 checks passed"));
    }

    #[test]
    fn an_outdated_rustc_gets_an_update_hint() {
        let targets = vec![
            "wasm32-unknown-unknown".to_owned(),
            "thumbv7em-none-eabihf".to_owned(),
        ];
        let old_rustc = |program: &str| {
            if program == "rustc" {
                Some("rustc 1.75.0 (82e1608df 2023-12-21)".to_owned())
            } else {
                Some("tool 1.90.0".to_owned())
            }
        };
        let statuses = evaluate(CHECKS, &old_rustc, Some(&targets));
        let out = report(CHECKS, &statuses);
        assert!(out.contains("outdated rustc (1.75.0)"));
        assert!(out.contains("update: rustup update stable (need ≥ 1.85.0)"));
        assert!(out.contains("8 of 9 checks passed"));
    }

    #[test]
    fn a_modern_rustc_passes_the_version_check() {
        assert_eq!(
            parse_semver("rustc 1.97.1 (abcd 2026-01-01)"),
            Some((1, 97, 1))
        );
        assert_eq!(parse_semver("rustc 1.85.0"), Some((1, 85, 0)));
        assert_eq!(parse_semver("no version here"), None);
    }

    #[test]
    fn without_rustup_the_targets_are_unknown_not_missing() {
        let statuses = evaluate(CHECKS, &every_version, None);
        let out = report(CHECKS, &statuses);
        assert!(out.contains("cannot check: could not query rustup"));
        assert!(out.contains("7 of 9 checks passed (2 could not be checked)"));
    }
}
