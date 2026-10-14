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

[files.exclude_if]
# output path (after .j2 is stripped) => minijinja expression;
# the file is skipped when the expression evaluates to true
Dockerfile = "backend.deployment != 'docker'"
"src/db.rs" = "backend.database != 'sqlx'"
```

Unknown manifest fields are rejected, so typos fail loudly.

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
| `backend` | object or `none` | `{ backend, database, deployment, testing, features }` — present when the spec has a backend |
| `frontend` | string or `none` | Kebab-case frontend id — present when the spec has a frontend |
| `spec` | object | The full spec enum |

Prefer `backend`/`frontend` over digging into `spec`: the spec enum's shape
depends on the web-app variant (backend-only / frontend-only / fullstack),
so paths through it break under strict mode. `backend` and `frontend` are
flat and shape-independent.

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
- If rendering fails midway, a directory rise created is removed again — no
  half-written projects.
- Template paths that would escape the target directory (`../`, absolute)
  are rejected.
