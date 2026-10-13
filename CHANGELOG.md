# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-10-13

First published release. The package is `rise-cli`; the binary is `rise`.

### Added

- Interactive wizard (`rise` with no arguments): asks for the project name
  first (validated as a crate name, target directory must be free), then
  project kind and the stack options for that kind.
- Hybrid `rise new [NAME] [flags]`: clap flags prefill the spec, prompts ask
  only for the gaps; conflicting or incomplete flag sets produce typed errors
  naming the flag to pass.
- Non-interactive mode for scripts and CI: `--no-input`, or automatically
  when stdin is not a TTY.
- `rise list` prints the embedded templates.
- Project generation from embedded templates (Jinja2 via minijinja) with
  manifest-driven conditional files (`exclude_if` in `template.toml`).
- Backend-only web templates: `web-axum` (Axum + tokio) and `web-actix`
  (Actix Web); a multi-stage Dockerfile is generated when deployment is
  `docker`.
- `git init` in the generated project and next-steps hints after generation.
- Typed errors with clear messages; exit code 130 on abort (Esc/Ctrl+C).

[Unreleased]: https://github.com/EryyynIT/rise/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/EryyynIT/rise/releases/tag/v0.1.0
