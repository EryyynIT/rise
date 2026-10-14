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
| `--kind` | `web`, `cli`, `library` | inferred as `web` when any web flag is passed |
| `--backend` | `axum`, `actix` | alone → backend-only; with `--frontend` → fullstack |
| `--frontend` | `askama`, `tera`, `minijinja`, `handlebars` | server-side template engine rendered by the backend |
| `--db` | `sqlx`, `diesel`, `sea-orm`, `sled` | `sqlx` wires a pool + `.env.example`; the others add the dependency |
| `--deployment` | `docker`, `kubernetes`, `serverless`, `self-hosted` | `docker` ships a slim multi-stage Dockerfile |
| `--testing` | `api`, `sqlx`, `mock` | repeatable, comma-separated; adds test modules and dev-dependencies |
| `--features` | `web-sockets` | repeatable, comma-separated; adds a `/ws` echo endpoint |
| `--template` | a template directory | render your own template — see [docs/TEMPLATES.md](docs/TEMPLATES.md) |
| `--no-input` | | never prompt; a non-terminal stdin implies this |

What generates today: Axum and Actix Web backends, fullstack variants of both
with the four template engines above, plus `--kind cli` and `--kind library`.
Other combinations the flags accept (`rocket`, `yew`, `embedded`, `game`, ...)
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
-[] Gamedev
-[] Embedded
-[x] Libraries
- More, you're free to give more ideas!

Generation for:
-[/] WebApps (backend-only Axum and Actix Web — sqlx pool, tests and WebSockets wired from your choices — plus fullstack with Askama, Tera, Minijinja or Handlebars)
-[x] CLI
-[] Gamedev
-[] Embedded
-[x] Libraries
- More (if you wish to add something)

## The vision

The goal is to support multiple kinds of Rust projects with plug-and-play templates

Bring your own templates or use the ones bundled in. The vision is modular, and as little boilerplate pain as possible.

Things are moving fast. Expect rough edges, TODOs, and probably a few panics. Contributions, feedback, and chaos-testing welcome.

It's not official. It's better.

Let me know if you want badges, install sections for the future, or a snarkier "FAQ" for people who inevitably ask why it's not on crates.io yet.

## 💻 Contributing

Found a panic? Have a killer template idea? Maybe you just want to build this thing too? Open an issue or PR. 
We're building this thing together.

## 📝 License

This project is licensed under the **Apache License 2.0** - see the [LICENSE](./LICENSE) file for details.

