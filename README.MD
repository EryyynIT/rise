# 🦀 rise (rust initial setup engine)

> The unofficial, community-driven way to kickstart modern Rust projects.

Whether you're building a web apps, CLI tool, game, or something embedded and weird, 
`rise` helps you spin up opinionated templates fast — with support for async runtimes, 
frameworks like Axum, Bevy, and more.

Choose your stack, generate the boilerplate, and ship something spicy.

--

## 📦 Installation

```bash
cargo install rise-cli --locked
```

The binary is called `rise` — the plain name was already taken on crates.io, so the package is `rise-cli`.

--

## Quick Start (for the impatient)

```bash
rise            # interactive wizard
```

or script it with flags:

```bash
rise new myapp --kind web --backend axum --deployment docker
cd myapp && cargo run
```

Yes, it's that simple. Prebuilt binaries and shell installers live on the [Releases page](https://github.com/EryyynIT/rise/releases).

## Usage

Three ways to drive it:

```bash
rise                                   # interactive wizard — the main mode
rise new myapp --kind cli              # hybrid: flags prefill, the wizard asks the gaps
rise new myapp --kind cli --no-input   # fully scripted: missing options are an error, not a question
```

Everything the wizard asks is also a flag on `rise new`:

| Flag | Values | Notes |
|---|---|---|
| `NAME` | a valid crate name | positional; asked first when omitted |
| `--kind` | `web`, `cli`, `library`, `game`, `embedded` | inferred as `web` when any web flag is passed |
| `--backend` | `axum`, `actix`, `rocket` | alone → backend-only; with `--frontend` → fullstack |
| `--frontend` | `yew`, `leptos`, `askama`, `tera`, `minijinja`, `handlebars` | `yew` → WASM SPA (alone) or workspace member (with a backend); `leptos` → trunk-built csr SPA (alone) or a single-crate SSR + hydration app built with cargo-leptos (with a backend); the rest are server-side template engines |
| `--db` | `sqlx`, `diesel`, `sea-orm`, `sled` | wires a connection module + `.env.example` — sqlx pool, SeaORM connection, Diesel r2d2 pool, sled store; leptos apps get it in their server functions |
| `--deployment` | `docker`, `self-hosted` | `docker` ships a slim multi-stage Dockerfile |
| `--testing` | `api`, `sqlx` | repeatable, comma-separated; adds test modules and dev-dependencies; with a leptos frontend `api` tests the server function endpoint (run `cargo test --features ssr`); `sqlx` needs `--db sqlx` (it tests the pool) |
| `--features` | `web-sockets`, `grpc`, `graph-ql`, `rate-limiting` | repeatable, comma-separated; `/ws` echo endpoint, tonic gRPC Greeter on :50051 (builds need [protoc](https://grpc.io/docs/protoc-installation/)), async-graphql with GraphiQL at `/graphql`, per-IP rate limiting — all of them also next to a leptos frontend's routes |
| `--template` | a template directory | render your own template — see [docs/TEMPLATES.md](docs/TEMPLATES.md) |
| `--no-input` | | never prompt; a non-terminal stdin implies this |

What generates today: Axum, Actix Web and Rocket backends, fullstack variants of
all three with the four template engines above, a Yew WASM frontend (a cargo
workspace
where the backend serves the API and the built frontend bundle) or a Leptos
frontend (a single crate built with cargo-leptos — server-side rendering plus
hydration, database wired into server functions, the same testing and feature
options as the REST backends), frontend-only Yew and Leptos
SPAs built with trunk, plus `--kind cli`, `--kind library`, `--kind game`
(a playable macroquad window — move with arrows/WASD, collect coins) and
`--kind embedded` (bare-metal ARM Cortex-M firmware: cortex-m-rt + defmt
logging, flashed with probe-rs — adjust `.cargo/config.toml` and `memory.x`
to your MCU).
Other combinations the flags accept (`seed`, ...)
end with a typed "not available yet" error — that's the roadmap, not a broken
promise.

Exit codes for scripting: `0` success · `1` error (message on stderr) ·
`2` bad arguments · `130` you pressed Esc/Ctrl+C — no stack traces, no
half-written project left behind.

## Shell completions

```bash
rise completions bash > ~/.local/share/bash-completion/completions/rise
```

`zsh`, `fish`, `powershell` and `elvish` work too — print the script and save it where your shell looks for completions.

## Bring your own templates

Copy an embedded template (or write your own — a directory with a `template.toml` manifest, `.j2` files rendered with the spec), tweak it, and point rise at it:

```bash
rise new myapp --kind web --backend axum --template ./my-template
```

The manifest format, render context and rendering rules are documented in [docs/TEMPLATES.md](docs/TEMPLATES.md).

## Future roadmap teaser

-[x] `cargo install rise-cli` (binary: `rise`)

-[x] Better error handling with crates like `thiserror`

-[x] Hybrid CLI: `rise new` with stack flags (wizard fills the gaps) + `rise list`

Selection for:
-[x] WebApps
-[x] CLI
-[x] Gamedev
-[x] Embedded
-[x] Libraries
- More, you're free to give more ideas!

Generation for:
-[/] WebApps (backend-only Axum, Actix Web and Rocket — database wiring, tests, WebSockets, gRPC, GraphQL and rate limiting generated from your choices — fullstack with Askama, Tera, Minijinja or Handlebars, Leptos SSR + hydration apps built with cargo-leptos (database wired into server functions, same tests and features), and Yew or Leptos WASM frontends: standalone trunk SPAs or — Yew — a fullstack workspace)
-[x] CLI
-[x] Gamedev
-[x] Embedded
-[x] Libraries
- More (if you wish to add something)

## The vision

The goal is to support multiple kinds of Rust projects with plug-and-play templates

Bring your own templates or use the ones bundled in. The vision is modular, and as little boilerplate pain as possible.

Things are moving fast. Expect rough edges. Contributions, feedback, and chaos-testing welcome.

It's not official. It's better.

## 💻 Contributing

Found a panic? Have a killer template idea? Maybe you just want to build this thing too? Open an issue or PR. 
We're building this thing together.

## 📝 License

This project is licensed under the **Apache License 2.0** - see the [LICENSE](./LICENSE) file for details.

