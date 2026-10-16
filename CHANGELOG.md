# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `--db sqlite`: an embedded SQLite database over sqlx — a local file at
  `data/app.db` (created on first run, `/data` gitignored), no database
  server and no `.env` needed. Wired into every web template: the
  connection pool, the JWT auth module (sqlite SQL dialect), and the
  leptos server functions. `--testing sqlx` accepts it; the pool test runs
  live against an in-memory database instead of being `#[ignore]`d.
- `compose.yaml` for Docker projects with a server database
  (sqlx/diesel/sea-orm): `docker compose up --build` starts the app
  together with a Postgres service — the container's `DATABASE_URL` points
  at the service, the `.env` value keeps working for a local `cargo run`.
  Embedded databases (sqlite, sled) get no compose file: there is no
  second service to orchestrate.
- Config file (`~/.config/rise/config.toml`, or `$RISE_CONFIG`): defaults
  for the web wizard — `backend`, `frontend`, `db`, `deployment`,
  `testing`, `features`. Precedence: CLI flag > config > built-in default.
  Interactive prompts are preselected with the configured values, `--yes`
  takes them over the built-in defaults, and a config naming a backend or
  frontend fills the shape itself. Unknown keys and values are rejected
  with the file named; the project name and `--kind` keep having no
  defaults.
- New `--features` values: `cors` (permissive CORS — tower-http on Axum,
  actix-cors on Actix Web, a rocket_cors fairing on Rocket, ssr-gated in the
  Leptos templates; restrict the origin before deploying) and `auth`
  (JWT auth — `/auth/register` and `/auth/login` issuing tokens signed with
  `JWT_SECRET` from `.env.example`, a protected `/api/me` example, users
  stored in the project's database with argon2-hashed passwords).
- `rise doctor`: check the prerequisites of the generated stacks and get
  an install hint for every gap — no more discovering a missing protoc
  when the generated project fails to build. The report is grouped by
  stack (core, docker deployment, frontends, gRPC, embedded); it checks
  git and rustc ≥ 1.85 (generated projects are edition 2024 — an older
  compiler is reported with an update hint), docker, and the per-stack
  toolchain (trunk, wasm32 target, cargo-leptos, protoc, probe-rs,
  thumbv7em target). Always exits 0: a gap only matters for the stack
  that needs it.
- Windows CI: the test workflow now builds and runs the full test suite on
  `windows-latest` too — the Windows binaries the releases have shipped
  since 0.1.0 are finally exercised. Line endings are pinned to LF via
  `.gitattributes` so the embedded templates and snapshots stay
  byte-identical across platforms; the PTY tests remain unix-only.

### Changed

- **The built-in default database is `sqlite` instead of `sqlx`** —
  `rise new myapp --kind web --yes` (and an Enter-only interactive run)
  now generates an Axum app that answers `curl http://localhost:3000`
  immediately after `cargo run`, with zero external services. The Postgres
  stacks are unchanged behind `--db sqlx` / `diesel` / `sea-orm`.
- The "copy `.env.example`" next step is only printed when it carries
  something the app needs: a `DATABASE_URL` for server databases, or
  `JWT_SECRET` when the `auth` feature is on. An embedded database without
  auth needs no setup at all.

## [0.7.0] - 2026-10-16

### Added

- `--yes`: take the built-in defaults for every option the flags leave out,
  silently — backend `axum`, database `sqlx`, deployment `docker`, no
  testing/features. The project name and `--kind` have no defaults and are
  still required; `--yes` conflicts with `--no-input`, which errors on
  every gap instead.
- PTY tests for the interactive wizard: the prompts and exit codes that
  only a TTY run produces are now pinned — Enter-only answers complete the
  whole flow and generate the first-option stack, and Esc on the first
  prompt exits 130 without generating anything.
- `--backend rocket`: a new `web-rocket` template scaffolds a Rocket 0.5
  backend with the same content depth as the Axum/Actix ones — all four
  databases wired into managed state, `rocket::local::blocking::Client` API
  tests and the sqlx pool test (no dev-dependencies needed), and every
  feature: a `rocket_ws` echo endpoint, per-IP rate limiting (an in-process
  request+response fairing with token buckets), async-graphql endpoints with
  GraphiQL at `/graphql`, and the tonic Greeter on :50051. The template
  overrides Rocket's defaults to listen on `0.0.0.0:3000`, ships `.env`
  loading via dotenvy, Docker/Dockerfile support and the four server-side
  template-engine frontends (askama/tera/minijinja/handlebars, templates
  embedded via `include_str!` so the generated binary runs in the slim
  image). As a Yew-fullstack backend member it serves `frontend/dist` with
  a `FileServer` plus an SPA-fallback route. The wizard offers Rocket in
  the backend prompt; Leptos is not offered for it (no integration exists)
  and a flag-passed `--frontend leptos --backend rocket` still ends in a
  typed `TemplateNotFound`.
- `--kind game`: a new `game-macroquad` template generates a playable
  macroquad game — a window, a game loop with delta-time movement
  (arrows/WASD), coins spawning on a timer to collect for score. The
  wizard's kind prompt offers the game kind too.
- `--kind embedded`: a new `embedded-cortex-m` template generates
  bare-metal ARM Cortex-M firmware — a cortex-m-rt entry point, defmt
  logging over RTT (defmt-rtt + panic-probe), a probe-rs runner and a
  `memory.x` placeholder (STM32F401CC) with the target pinned to
  `thumbv7em-none-eabihf` in `.cargo/config.toml`; `cargo run` flashes
  the board. The wizard's kind prompt offers the embedded kind, and the
  next steps spell out the one-time setup. Verified to build (debug +
  release) for the pinned target; flashing needs real hardware.
- Testing and features for Leptos fullstack apps: `--testing` and
  `--features` are no longer rejected with `--frontend leptos`, and the
  wizard asks those questions for leptos too. `api` generates an
  ssr-gated test that POSTs to the generated server function's endpoint
  (live for sled, `#[ignore]`d for the Postgres databases, mirroring the
  REST templates); `sqlx` generates the pool test. All four features work
  next to the leptos routes: `web-sockets` (axum `ws` route / actix-ws
  service), `rate-limiting` (tower_governor / actix-governor around the
  leptos router), `graph-ql` (POST /graphql + GraphiQL; the schema is a
  request extension on axum, an `app_data` on actix) and `grpc` (the
  tonic Greeter on :50051, protoc prerequisite and Dockerfile wiring like
  the REST templates). Everything server-side is optional and gated on
  the `ssr` feature, so the hydrate/WASM build never sees it.
  `--testing mock` stays rejected for leptos — a Leptos app has no REST
  layer to mock — with a dedicated "testing options not available for
  this stack" error.

### Changed

- The wizard now offers only stacks that have templates: `axum`/`actix`
  backends, the `web`/`cli`/`library`/`embedded`/`game` kinds,
  `yew`/`leptos` for
  frontend-only apps plus the four template engines with a backend.
  Previously most offered choices (five of seven backends, the embedded
  and game kinds, seven of nine frontend-only options) walked the user
  through the whole questionnaire and failed at generation. Explicit
  flags for unimplemented stacks still get the typed "not available yet"
  error.
- The wizard's testing options follow the chosen database: the `sqlx`
  pool test is offered only with the sqlx database (it silently rendered
  nothing with another one when picked interactively).
- Yew fullstack next steps copy `.env.example` to the workspace root, not
  `backend/`: `dotenvy` searches the working directory and its parents,
  and the printed `cargo run -p <name>-backend` runs from the root, so
  the old location was never found and the app panicked on
  `DATABASE_URL`.

### Removed

- `--testing mock`: it emitted a dead `mockall` dev-dependency with no
  mock-based test anywhere (the generated code has no trait seam to
  mock). The value is now rejected up front on every stack with the
  "testing options not available for this stack" error.
- `--deployment kubernetes` and `--deployment serverless`: they generated
  byte-identical output to `self-hosted` — only `docker` adds files.

### Fixed

- A template manifest's `exclude_if` key that names no file in the
  template (a typo) now fails generation with an error naming the key —
  the rule was previously dead and the file shipped in every project.
- A failed generation into a pre-existing empty directory now removes
  everything the run wrote, keeping the directory itself — half-written
  output was previously left behind there (only directories rise created
  were cleaned up).
- The leptos templates' `.dockerignore` now excludes `.env`: both
  Dockerfiles `COPY . .` into the builder stage, so a real
  `DATABASE_URL` leaked into the build context and builder-stage layers.
- The actix and leptos-actix rate limiting now spawns the periodic
  limiter cleanup task the axum templates already had — the governor's
  per-IP state store grew unboundedly without it.
- Project names that are Windows device names (`con`, `aux`, `nul`,
  `com1`–`com9`, `lpt1`–`lpt9`) are rejected at validation instead of
  failing with an obscure OS error when the directory is created.

## [0.6.0] - 2026-10-15

### Added

- gRPC for Axum and Actix Web backends: `--features grpc` generates a
  tonic-based Greeter service (`proto/hello.proto`, `build.rs` with
  tonic-prost-build, `src/grpc.rs`) served on port 50051 next to the HTTP
  server. The generated project needs protoc at build time — a documented
  prerequisite, like trunk for Yew — and its Dockerfile installs
  `protobuf-compiler` in the builder stage. With this, every `--features`
  value is implemented.
- Frontend-only Leptos: `--frontend leptos` without `--backend` generates a
  client-side rendered SPA built with trunk (new embedded template
  `frontend-leptos`), and the wizard offers Leptos for frontend-only apps
  again.
- Leptos fullstack apps wire the chosen database end to end: a `db` module
  (sqlx pool, SeaORM connection, Diesel r2d2 pool or sled store),
  `.env.example` + `.gitignore` entries, libpq in the Dockerfile for diesel,
  and an example `#[server]` function that reads the connection from the
  request context and streams a status string onto the home page. The wizard
  asks for a database like for any other backend and `--db` is accepted with
  `--frontend leptos`; `--testing`/`--features` stay rejected (they configure
  the REST stack server functions replace).

### Changed

- `BackendSpec.database` is a plain `Database` again — every backend-carrying
  spec now has one, so the field no longer needs to be optional.

## [0.5.1] - 2026-10-14

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

[Unreleased]: https://github.com/EryyynIT/rise/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/EryyynIT/rise/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/EryyynIT/rise/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/EryyynIT/rise/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/EryyynIT/rise/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/EryyynIT/rise/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/EryyynIT/rise/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/EryyynIT/rise/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/EryyynIT/rise/releases/tag/v0.1.0
