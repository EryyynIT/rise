# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `cli` template: `--kind cli` generates a command-line scaffold on clap
  (derive).
- `library` template: `--kind library` generates a library crate with a test
  stub.
- The next-steps hint after generation matches the project kind: `cargo run`,
  or `cargo test` for libraries.
- The web templates now honor the `database` choice: `sqlx` adds the sqlx
  dependency, a `src/db.rs` connection pool, `.env.example` and a `.env`
  gitignore entry; Diesel, SeaORM and Sled add their dependency only. The
  pool uses sqlx's runtime-checked API, so the project compiles offline.
- The web templates now honor the `testing` choice: `api` adds a smoke test
  (`tower` oneshot for Axum, `actix_web::test` for Actix Web), `sqlx` adds an
  ignored pool-connectivity test, `mock` adds the mockall dev-dependency.
- `--features web-sockets` adds a WebSocket echo endpoint (`/ws`) to both web
  templates.
- Fullstack generation with a server-side template engine: `--backend axum`
  (or `actix`) combined with `--frontend askama`, `tera`, `minijinja` or
  `handlebars` renders the engine dependency, a root handler serving
  `templates/index.html` (`index.hbs` for Handlebars) and the template file
  itself. Runtime engines embed the template into the binary via
  `include_str!`, so the app runs from any working directory, including the
  slim Docker image.
- Next steps for sqlx projects mention `cp .env.example .env`.

### Changed

- `--testing sqlx` without `--db sqlx` is rejected as conflicting options
  (the generated tests would have no sqlx pool to exercise).

### Fixed

- Rendered files keep their trailing newline (minijinja strips it by
  default).

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
