# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Generated Axum/Actix backends load `.env` at startup (via `dotenvy`), so
  the `cp .env.example .env` next step rise prints is now sufficient to run
  the app — previously nothing read the file and the app panicked on the
  missing `DATABASE_URL`.
- Every Docker template ships a `.dockerignore` (`target/`, `.git`, `.env`,
  `dist/`), keeping local build artifacts and secrets out of the image
  build context.
- Two integration tests pin the crash-only guarantee: a failed generation
  removes the half-written project directory, and an existing *empty*
  target directory is accepted.

### Changed

- Project names that are Rust keywords (`loop`, `fn`, `Self`, `gen`, ...)
  are rejected up front. Cargo accepts some of them as package names, but
  crates.io refuses them and generated use-statements (`use loop::...`)
  do not parse.
- The wizard no longer offers Leptos for a frontend-only web app; that
  combination has no template yet and used to fail at generation after
  the full question flow.
- `--template` directories render in sorted path order instead of
  filesystem iteration order, matching the deterministic embedded
  templates.

### Fixed

- `rise list` (and the banner/next-steps output) no longer panics with
  exit code 101 when stdout closes early (`rise list | head -1`).

## [0.5.0] - 2026-10-14

### Added

- Leptos fullstack apps: `--backend axum --frontend leptos` (or `actix`)
  generates a single crate modeled on the official start-axum/start-actix
  templates — server-side rendering plus hydration, built with
  cargo-leptos (`[package.metadata.leptos]`, ssr/hydrate features), with a
  multi-stage Dockerfile when `--deployment docker`.
- The render context gains `crate_ident`: `crate_name` with `-` replaced
  by `_`, for Rust use-statements inside generated code.

### Changed

- `BackendSpec.database` is now optional: a Leptos fullstack app carries
  no database, because server functions replace the REST stack that
  `--db`, `--testing` and `--features` configure. Passing those flags
  with a leptos frontend is rejected with an actionable error instead of
  being silently ignored; the wizard skips those questions for leptos.

## [0.4.0] - 2026-10-14

### Added

- WASM frontends with Yew. `--frontend yew` alone generates a trunk-built
  single-page app (a trunk+nginx Dockerfile ships with
  `--deployment docker`). Combined with `--backend axum` or
  `--backend actix` it generates a cargo workspace: `<name>-backend` serves
  the API under `/api` and the built trunk bundle from `frontend/dist`,
  `<name>-frontend` carries the Yew app with a `Trunk.toml` dev proxy, and
  the workspace root adds a README and a multi-stage Dockerfile with a
  trunk build stage.
- The render context gains `crate_name` (the package name a template
  produces — differs from `name` for workspace members), `deployment`
  (shape-independent, unlike spec paths) and `workspace` (true for members
  of a multi-crate plan). Template manifests can set `internal = true` to
  stay out of `rise list`.
- Every `--db` value now wires its database into the generated backend,
  not just the dependency: SeaORM connects a `DatabaseConnection`, Diesel
  builds an r2d2 pool (generated Dockerfiles install libpq for it) and
  sled opens an embedded store from `DB_PATH`. All of them ship
  `.env.example` and `.gitignore` entries like sqlx already did.
- New implemented `--features`: `rate-limiting` (per-IP limiting via
  tower-governor on axum, actix-governor on actix) and `graph-ql`
  (async-graphql with a `hello` query, `POST /graphql` and the GraphiQL
  IDE on `GET /graphql`). `grpc` stays rejected as not implemented — it
  would require protoc at build time in generated projects.

### Changed

- `TemplateNotFound` errors for unsupported frontends name the missing
  combination (`web-leptos`, `web-fullstack-leptos`, ...) instead of a
  generic `web-frontend`/`web-fullstack`.

### Fixed

- `Feature::GraphQL` serialized into the render context as `graph-q-l`
  (heck splits the acronym); the serde name now matches the CLI spelling
  `graph-ql`.

## [0.3.0] - 2026-10-14

### Added

- `rise completions <shell>` prints a shell completion script for bash,
  zsh, fish, powershell or elvish to stdout.
- `--template <DIR>` on `rise new` loads the template from a directory on
  disk instead of the embedded one for the selected stack; the spec is
  still collected as usual and forms the render context. Unreadable
  template directories report the offending path.
- Usage reference in the README and a template authoring guide in
  `docs/TEMPLATES.md`.

### Changed

- `--features grpc`, `graph-ql` and `rate-limiting` are rejected up front
  ("features not available yet") instead of being silently ignored; only
  `web-sockets` has template content so far. Interactive prompts offer
  only implemented features.

## [0.2.0] - 2026-10-14

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

[Unreleased]: https://github.com/EryyynIT/rise/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/EryyynIT/rise/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/EryyynIT/rise/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/EryyynIT/rise/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/EryyynIT/rise/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/EryyynIT/rise/releases/tag/v0.1.0
