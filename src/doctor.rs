//! `rise doctor`: check that the prerequisites of the generated stacks
//! are installed, with install hints for whatever is missing.

use colored::Colorize;

use crate::Error;

/// One prerequisite of one or more generated stacks.
struct Check {
    /// What the report calls it.
    name: &'static str,
    /// How to detect it.
    probe: Probe,
    /// Which generated stack(s) need it, so a gap reads as "only matters
    /// if you use that stack", not as a failure.
    needed_for: &'static str,
    /// How to install it; mirrors the one-time setup hints the generator
    /// prints as next steps.
    install_hint: &'static str,
}

/// How a check detects its prerequisite.
enum Probe {
    /// Run `<program> --version`; present when the run succeeds.
    Version(&'static str),
    /// Present when `rustup target list --installed` names the target.
    RustupTarget(&'static str),
}

/// The prerequisites of everything rise can generate.
const CHECKS: &[Check] = &[
    Check {
        name: "trunk",
        probe: Probe::Version("trunk"),
        needed_for: "yew/leptos frontends",
        install_hint: "cargo install trunk",
    },
    Check {
        name: "wasm32-unknown-unknown",
        probe: Probe::RustupTarget("wasm32-unknown-unknown"),
        needed_for: "yew/leptos frontends",
        install_hint: "rustup target add wasm32-unknown-unknown",
    },
    Check {
        name: "cargo-leptos",
        probe: Probe::Version("cargo-leptos"),
        needed_for: "leptos fullstack",
        install_hint: "cargo install --locked cargo-leptos",
    },
    Check {
        name: "protoc",
        probe: Probe::Version("protoc"),
        needed_for: "the grpc feature",
        install_hint: "https://grpc.io/docs/protoc-installation/",
    },
    Check {
        name: "probe-rs",
        probe: Probe::Version("probe-rs"),
        needed_for: "embedded firmware",
        install_hint: "cargo install probe-rs-tools --locked",
    },
    Check {
        name: "thumbv7em-none-eabihf",
        probe: Probe::RustupTarget("thumbv7em-none-eabihf"),
        needed_for: "embedded firmware",
        install_hint: "rustup target add thumbv7em-none-eabihf",
    },
];

/// What one check found.
enum Status {
    /// Installed; carries the first line of `--version` output (empty for
    /// rustup targets, which have no version line).
    Present(String),
    /// Not installed.
    Missing,
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
            Probe::RustupTarget(target) => match installed_targets {
                Some(installed) if installed.iter().any(|installed| installed == target) => {
                    Status::Present(String::new())
                }
                Some(_) => Status::Missing,
                None => Status::Unknown("rustup not found"),
            },
        })
        .collect()
}

/// Render the report: one line per check, an install hint under every
/// gap, and a summary.
fn report(checks: &[Check], statuses: &[Status]) -> String {
    let mut out = String::from("rise doctor — prerequisites of the generated stacks\n\n");
    let mut present = 0;
    let mut unknown = 0;
    for (check, status) in checks.iter().zip(statuses) {
        let version = match status {
            Status::Present(version) => {
                present += 1;
                if version.is_empty() {
                    String::new()
                } else {
                    format!(" ({version})")
                }
            }
            _ => String::new(),
        };
        // Pad before coloring: the escape codes must not count toward the
        // column width.
        let word = match status {
            Status::Present(_) => format!("{:>8}", "ok").green(),
            Status::Missing => format!("{:>8}", "missing").red(),
            Status::Unknown(_) => {
                unknown += 1;
                format!("{:>8}", "unknown").yellow()
            }
        };
        out.push_str(&format!(
            "  {word} {}{version} — {}\n",
            check.name, check.needed_for
        ));
        match status {
            Status::Missing => {
                out.push_str(&format!("           install: {}\n", check.install_hint));
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
        Some("tool 1.0".to_owned())
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
    fn a_missing_tool_gets_its_install_hint() {
        let none_installed: Vec<String> = Vec::new();
        let only_trunk = |program: &str| (program == "trunk").then(|| "trunk 0.21".to_owned());
        let statuses = evaluate(CHECKS, &only_trunk, Some(&none_installed));
        let out = report(CHECKS, &statuses);
        assert!(out.contains("trunk 0.21"));
        assert!(out.contains("install: cargo install --locked cargo-leptos"));
        assert!(out.contains("install: rustup target add wasm32-unknown-unknown"));
        assert!(out.contains("1 of 6 checks passed"));
    }

    #[test]
    fn without_rustup_the_targets_are_unknown_not_missing() {
        let statuses = evaluate(CHECKS, &every_version, None);
        let out = report(CHECKS, &statuses);
        assert!(out.contains("cannot check: rustup not found"));
        assert!(out.contains("4 of 6 checks passed (2 could not be checked)"));
    }
}
