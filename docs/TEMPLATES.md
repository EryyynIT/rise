# Writing templates for rise

A template is a directory with a `template.toml` manifest plus arbitrary files.
Point rise at it with `--template` — no rebuild needed, edit and re-run:

```bash
rise new myapp --kind cli --template ./my-template
```

```
my-template/
├── template.toml      # the manifest
├── Cargo.toml.j2      # .j2 → rendered, suffix stripped
├── src/
│   └── main.rs.j2
└── logo.png           # no .j2 → copied byte-for-byte
```

The embedded templates under [`assets/templates/`](../assets/templates/) are
the reference implementations — copy one and tweak.

## The manifest

```toml
name = "my-template"
description = "What it scaffolds"

# optional: hide from `rise list` — for templates that only make sense as
# a building block of a multi-crate plan (see "Composition" below)
internal = true

[files.exclude_if]
# output path (after .j2 is stripped) => minijinja expression;
# the file is skipped when the expression evaluates to true
Dockerfile = "backend.deployment != 'docker'"
"templates/index.hbs" = "frontend != 'handlebars'"
```

Unknown manifest fields are rejected, so typos fail loudly. The same goes
for `exclude_if` keys: a key that names no file in the template fails the
render — a typo'd key would otherwise be a silently dead rule.

## Rendering

- `*.j2` files go through [minijinja](https://docs.rs/minijinja) (Jinja2
  syntax); everything else is copied verbatim, binaries included.
- **Undefined variables are errors** (strict mode): a typo in `{{ naem }}`
  fails the generation instead of writing an empty string.
- The template's trailing newline is preserved in the output.
- Use `{%- ... -%}` (whitespace trimming) around conditional blocks, or the
  generated files end up with stray blank lines — see the embedded
  `Cargo.toml.j2` for the idiom.

## Render context

| Variable | Type | Contents |
|---|---|---|
| `name` | string | The validated project name |
| `crate_name` | string | Package name for `Cargo.toml` and binary paths — equals `name`, except for workspace members (`<name>-backend`, `<name>-frontend`) |
| `crate_ident` | string | `crate_name` with `-` replaced by `_` — how Rust code refers to the crate (`use my_app::...`) |
| `backend` | object or `none` | `{ backend, database, deployment, testing, features }` — present when the spec has a backend |
| `frontend` | string or `none` | Kebab-case frontend id — present when the spec has a frontend |
| `deployment` | string or `none` | The deployment choice, shape-independent (the backend's, or the frontend-only app's) |
| `workspace` | bool | `true` when rendering a member of a multi-crate plan — the root step ships project-level files (`.gitignore`, Dockerfile), members should skip theirs |
| `spec` | object | The full spec enum |

Prefer `backend`/`frontend`/`deployment` over digging into `spec`: the spec
enum's shape depends on the web-app variant (backend-only / frontend-only /
fullstack), so paths through it break under strict mode. The flattened
fields are shape-independent.

Enum values are kebab-case strings:

```jinja
{%- if frontend == 'askama' %}
askama = "0.16"
{%- endif %}
{%- if 'web-sockets' in backend.features %}
axum = { version = "0.8", features = ["ws"] }
{%- endif %}
```

Guard every `backend`/`frontend` access: they are `none` when the spec
doesn't have that stack (`{% if backend %}...{% endif %}`).

## Behaviour to rely on

- rise refuses to generate into a directory that exists and is not empty.
- If rendering fails midway, a directory rise created is removed again, and
  a pre-existing empty directory is emptied of what the run wrote — no
  half-written projects.
- Template paths that would escape the target directory (`../`, absolute)
  are rejected.
- Files render in sorted path order (a `--template` directory is sorted on
  load; embedded templates are ordered at compile time).
- `--verify` runs `cargo check` on the output — including for `--template`
  directories, so a custom template should produce a compiling crate; for a
  leptos fullstack spec the check adds `--features ssr`.

## Composition

Most specs map to one template rendered into the project root. Multi-crate
stacks (today: fullstack with a WASM frontend) map to a **plan** of several
embedded templates rendered into subdirectories — e.g. the workspace root
template into `.`, the backend into `backend/`, the frontend into
`frontend/` — each with its own `crate_name`, all sharing the same render
context. Composition is embedded-only: `--template` always renders its
single directory into the project root with `crate_name == name` and
`workspace == false`.
