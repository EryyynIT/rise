//! The template engine: a complete [`ProjectSpec`] in, files on disk out.

use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use minijinja::value::Serde;
use minijinja::{Environment, ErrorKind as JinjaErrorKind, Value};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::name::ProjectName;
use crate::spec::{Backend, BackendSpec, Deployment, Frontend, ProjectSpec, WebAppSpec};
use crate::templates::template::{RENDER_SUFFIX, Template, TemplateFile};

/// Where to load the template from.
#[derive(Debug, Default)]
pub(crate) enum TemplateSource {
    /// Embedded templates implementing the spec's stack.
    #[default]
    Embedded,
    /// A template directory on disk, used instead of the embedded plan
    /// (`--template`). The spec is still collected as usual and forms the
    /// render context. A custom directory is always a single template
    /// rendered into the project root; composition is embedded-only.
    Dir(PathBuf),
}

/// One step of a generation plan: a single-crate project has exactly one
/// step rendering into the root; a workspace project has one step per
/// crate plus a root step.
#[derive(Debug, PartialEq, Eq)]
struct TemplateStep {
    /// Embedded template directory name.
    id: &'static str,
    /// Target subdirectory, relative to the project root (`""` = root).
    subdir: &'static str,
    /// Package name of the crate this step renders.
    crate_name: String,
}

impl TemplateStep {
    /// A step rendering into the project root; the crate takes the
    /// project name.
    fn root(id: &'static str, name: &ProjectName) -> Self {
        Self {
            id,
            subdir: "",
            crate_name: name.to_string(),
        }
    }

    /// A workspace member step: renders into `subdir`, the crate is named
    /// `<project>-<suffix>`.
    fn member(id: &'static str, subdir: &'static str, name: &ProjectName, suffix: &str) -> Self {
        Self {
            id,
            subdir,
            crate_name: format!("{name}-{suffix}"),
        }
    }
}

/// Generate a project from a complete spec into `./<name>/`.
///
/// The target directory is created; if it already exists it must be empty.
/// Generation is crash-only: when rendering or writing fails, a directory
/// created by this call is removed again.
///
/// # Errors
///
/// [`Error::TemplateNotFound`] if the spec's stack has no template yet,
/// [`Error::TemplateDirUnreadable`] if a `--template` directory cannot be
/// read, [`Error::TargetNotEmpty`] if `./<name>/` exists and is not empty,
/// and [`Error::Manifest`]/[`Error::Render`]/[`Error::Io`] for template and
/// filesystem failures.
pub(crate) fn generate(
    spec: &ProjectSpec,
    name: &ProjectName,
    source: &TemplateSource,
) -> Result<PathBuf> {
    let target = PathBuf::from(name.as_str());
    let target_existed = prepare_target(&target)?;

    let result = match source {
        TemplateSource::Embedded => {
            template_plan(spec, name).and_then(|plan| render_plan(&plan, spec, name, &target))
        }
        TemplateSource::Dir(path) => Template::from_dir(path).and_then(|template| {
            render_into(
                &template,
                spec,
                name.as_str(),
                name.as_str(),
                false,
                &target,
            )
        }),
    };
    if result.is_err() && !target_existed {
        // Never leave a half-written project behind.
        let _ = fs::remove_dir_all(&target);
    }
    result?;
    Ok(target)
}

/// Render every step of an embedded plan into its subdirectory.
fn render_plan(
    steps: &[TemplateStep],
    spec: &ProjectSpec,
    name: &ProjectName,
    target: &Path,
) -> Result<()> {
    // Multi-step plans are workspaces; member templates use the flag to
    // drop files the workspace root already provides.
    let workspace = steps.len() > 1;
    for step in steps {
        let template = Template::embedded(step.id)?;
        render_into(
            &template,
            spec,
            name.as_str(),
            &step.crate_name,
            workspace,
            &target.join(step.subdir),
        )?;
    }
    Ok(())
}

/// Map a spec to the plan of embedded templates implementing its stack.
fn template_plan(spec: &ProjectSpec, name: &ProjectName) -> Result<Vec<TemplateStep>> {
    let backend_template = |backend: Backend| match backend {
        Backend::Axum => Ok("web-axum"),
        Backend::Actix => Ok("web-actix"),
        other => Err(Error::TemplateNotFound(format!(
            "web-{}",
            other.to_string().to_lowercase()
        ))),
    };
    match spec {
        ProjectSpec::WebApp(WebAppSpec::Backend(backend)) => Ok(vec![TemplateStep::root(
            backend_template(backend.backend)?,
            name,
        )]),
        ProjectSpec::WebApp(WebAppSpec::Frontend(frontend)) => match frontend.frontend {
            Frontend::Yew => Ok(vec![TemplateStep::root("frontend-yew", name)]),
            // Frontend-only Leptos is a client-side rendered SPA built with
            // trunk — a different template from the cargo-leptos fullstack.
            Frontend::Leptos => Ok(vec![TemplateStep::root("frontend-leptos", name)]),
            other => Err(Error::TemplateNotFound(format!(
                "web-{}",
                other.to_string().to_lowercase()
            ))),
        },
        // A fullstack app with a template engine is the backend template
        // plus server-side rendering; the `frontend` context field drives
        // the engine-specific blocks.
        ProjectSpec::WebApp(WebAppSpec::Fullstack(fullstack))
            if fullstack.frontend.is_template_engine() =>
        {
            Ok(vec![TemplateStep::root(
                backend_template(fullstack.backend.backend)?,
                name,
            )])
        }
        // A fullstack app with a WASM frontend is a workspace: the backend
        // template renders into `backend/` (its `frontend`-conditional
        // blocks add static serving), the frontend template into
        // `frontend/`, and a small internal template wires up the root.
        ProjectSpec::WebApp(WebAppSpec::Fullstack(fullstack))
            if fullstack.frontend == Frontend::Yew =>
        {
            Ok(vec![
                TemplateStep::root("web-wasm-workspace", name),
                TemplateStep::member(
                    backend_template(fullstack.backend.backend)?,
                    "backend",
                    name,
                    "backend",
                ),
                TemplateStep::member("frontend-yew", "frontend", name, "frontend"),
            ])
        }
        // A Leptos fullstack app is a single crate again: leptos_axum /
        // leptos_actix integrate the frontend into the backend, and
        // cargo-leptos builds both targets from one manifest.
        ProjectSpec::WebApp(WebAppSpec::Fullstack(fullstack))
            if fullstack.frontend == Frontend::Leptos =>
        {
            let id = match fullstack.backend.backend {
                Backend::Axum => "web-leptos-axum",
                Backend::Actix => "web-leptos-actix",
                other => {
                    return Err(Error::TemplateNotFound(format!(
                        "web-leptos-{}",
                        other.to_string().to_lowercase()
                    )));
                }
            };
            Ok(vec![TemplateStep::root(id, name)])
        }
        ProjectSpec::WebApp(WebAppSpec::Fullstack(fullstack)) => {
            Err(Error::TemplateNotFound(format!(
                "web-fullstack-{}",
                fullstack.frontend.to_string().to_lowercase()
            )))
        }
        ProjectSpec::CliApp => Ok(vec![TemplateStep::root("cli", name)]),
        ProjectSpec::Library => Ok(vec![TemplateStep::root("library", name)]),
        ProjectSpec::Embedded => Err(Error::TemplateNotFound("embedded".into())),
        ProjectSpec::Game => Err(Error::TemplateNotFound("game".into())),
    }
}

/// Variables available to every template: the project name, the crate name
/// of the package being rendered, the full spec, and the web stacks
/// flattened out of the spec's enum nesting.
///
/// Templates use `backend`/`frontend`/`deployment` rather than digging into
/// `spec.WebApp.Backend` vs `spec.WebApp.Fullstack.backend`: strict
/// undefined behavior makes spec access shape-dependent, the flattened
/// fields are not. `None` serializes as a minijinja `none`, so
/// `{% if backend %}` guards every access. `deployment` is the backend's
/// deployment when there is one, the frontend-only app's otherwise.
///
/// `crate_name` equals `name` for a single-crate project; multi-crate
/// plans (workspaces) give each step its own crate name, so `Cargo.toml`
/// and binary paths must use `crate_name`, prose should use `name`.
/// `crate_ident` is `crate_name` with `-` replaced by `_` — the identifier
/// Rust code uses to refer to the crate (`use my_app::...`), since hyphens
/// are not valid in Rust identifiers. `workspace` tells member templates
/// that a root template already provides project-level files (`.gitignore`,
/// Dockerfile).
#[derive(Serialize)]
struct RenderContext<'a> {
    name: &'a str,
    crate_name: &'a str,
    crate_ident: String,
    spec: &'a ProjectSpec,
    backend: Option<&'a BackendSpec>,
    frontend: Option<Frontend>,
    deployment: Option<Deployment>,
    workspace: bool,
}

impl<'a> RenderContext<'a> {
    fn new(name: &'a str, crate_name: &'a str, workspace: bool, spec: &'a ProjectSpec) -> Self {
        let (backend, frontend, deployment) = match spec {
            ProjectSpec::WebApp(WebAppSpec::Backend(backend)) => {
                (Some(backend), None, Some(backend.deployment))
            }
            ProjectSpec::WebApp(WebAppSpec::Frontend(frontend)) => {
                (None, Some(frontend.frontend), frontend.deployment)
            }
            ProjectSpec::WebApp(WebAppSpec::Fullstack(fullstack)) => (
                Some(&fullstack.backend),
                Some(fullstack.frontend),
                Some(fullstack.backend.deployment),
            ),
            _ => (None, None, None),
        };
        Self {
            name,
            crate_name,
            crate_ident: crate_name.replace('-', "_"),
            spec,
            backend,
            frontend,
            deployment,
            workspace,
        }
    }
}

/// Ensure `target` is usable: nonexistent or an empty directory.
/// Returns whether it already existed.
fn prepare_target(target: &Path) -> Result<bool> {
    match fs::read_dir(target) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                return Err(Error::TargetNotEmpty(target.display().to_string()));
            }
            Ok(true)
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
        Err(err) => Err(Error::Io(err)),
    }
}

/// Render every template file into `target`. `crate_name` is the package
/// name this render produces — the project name for single-crate plans,
/// a member name (`<name>-backend`, ...) for workspace steps.
fn render_into(
    template: &Template,
    spec: &ProjectSpec,
    name: &str,
    crate_name: &str,
    workspace: bool,
    target: &Path,
) -> Result<()> {
    let mut env = Environment::new();
    // A typo in a template variable must fail the render, not silently
    // produce empty strings.
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
    // Generated text files should end with a newline; minijinja strips
    // the template's final one by default (Jinja2 compatibility).
    let syntax = minijinja::syntax::SyntaxConfig::builder()
        .keep_trailing_newline(true)
        .build()
        .expect("default syntax config is valid");
    env.set_syntax(syntax);

    let ctx = Value::from(Serde(RenderContext::new(name, crate_name, workspace, spec)));
    let excluded = excluded_paths(template, &env, &ctx)?;

    for file in template.files() {
        let rel = output_path(file.path());
        if excluded.contains(rel) {
            continue;
        }
        let contents = render_contents(&env, file, &ctx)?;
        let out = safe_join(target, rel)?;
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&out, contents)?;
    }
    Ok(())
}

/// Output path of a template file: the `.j2` suffix stripped if present.
fn output_path(path: &str) -> &str {
    path.strip_suffix(RENDER_SUFFIX).unwrap_or(path)
}

/// Evaluate every `exclude_if` rule once against the render context.
fn excluded_paths<'a>(
    template: &'a Template,
    env: &Environment,
    ctx: &Value,
) -> Result<HashSet<&'a str>> {
    let mut excluded = HashSet::new();
    for (path, source) in &template.manifest().files.exclude_if {
        let expr = env.compile_expression(source)?;
        if expr.eval(ctx.clone())?.is_true() {
            excluded.insert(path.as_str());
        }
    }
    Ok(excluded)
}

/// Render one file's contents. `.j2` files go through minijinja; everything
/// else is copied verbatim.
fn render_contents(env: &Environment, file: &TemplateFile, ctx: &Value) -> Result<Vec<u8>> {
    if file.path().ends_with(RENDER_SUFFIX) {
        let source = std::str::from_utf8(file.contents()).map_err(|_| {
            Error::Render(minijinja::Error::new(
                JinjaErrorKind::InvalidOperation,
                format!("{}: template files must be valid UTF-8", file.path()),
            ))
        })?;
        return Ok(env.render_str(source, ctx.clone())?.into_bytes());
    }
    Ok(file.contents().to_vec())
}

/// Join a template-relative path onto the target, refusing paths that
/// would escape it.
fn safe_join(target: &Path, rel: &str) -> Result<PathBuf> {
    let escapes = Path::new(rel).components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    });
    if escapes {
        return Err(Error::Render(minijinja::Error::new(
            JinjaErrorKind::InvalidOperation,
            format!("unsafe path in template: {rel}"),
        )));
    }
    Ok(target.join(rel))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{
        BackendSpec, Database, Deployment, Feature, FrontendSpec, FullstackSpec, Testing,
    };

    fn project_name(name: &str) -> ProjectName {
        ProjectName::new(name.to_owned()).expect("valid project name")
    }

    /// Single-crate renders: the crate takes the project name.
    fn render_test(
        template: &Template,
        spec: &ProjectSpec,
        name: &str,
        target: &Path,
    ) -> Result<()> {
        render_into(template, spec, name, name, false, target)
    }

    fn backend_spec(backend: Backend, deployment: Deployment) -> ProjectSpec {
        backend_spec_full(backend, Database::SQLx, deployment, vec![], vec![])
    }

    fn backend_spec_full(
        backend: Backend,
        database: Database,
        deployment: Deployment,
        testing: Vec<Testing>,
        features: Vec<Feature>,
    ) -> ProjectSpec {
        ProjectSpec::WebApp(WebAppSpec::Backend(BackendSpec {
            backend,
            database,
            deployment,
            testing,
            features,
        }))
    }

    fn fullstack_spec(backend: Backend, frontend: Frontend) -> ProjectSpec {
        ProjectSpec::WebApp(WebAppSpec::Fullstack(FullstackSpec {
            frontend,
            backend: BackendSpec {
                backend,
                database: Database::SQLx,
                deployment: Deployment::Docker,
                testing: vec![],
                features: vec![],
            },
        }))
    }

    fn frontend_spec(frontend: Frontend, deployment: Deployment) -> ProjectSpec {
        ProjectSpec::WebApp(WebAppSpec::Frontend(FrontendSpec {
            frontend,
            deployment: Some(deployment),
        }))
    }

    fn fullstack_spec_tested(backend: Backend, frontend: Frontend) -> ProjectSpec {
        fullstack_spec_full(
            backend,
            frontend,
            Database::SQLx,
            Deployment::Docker,
            vec![Testing::Api],
        )
    }

    fn fullstack_spec_with_db(
        backend: Backend,
        frontend: Frontend,
        database: Database,
    ) -> ProjectSpec {
        fullstack_spec_full(backend, frontend, database, Deployment::Docker, vec![])
    }

    fn fullstack_spec_featured(
        backend: Backend,
        frontend: Frontend,
        features: Vec<Feature>,
    ) -> ProjectSpec {
        ProjectSpec::WebApp(WebAppSpec::Fullstack(FullstackSpec {
            frontend,
            backend: BackendSpec {
                backend,
                database: Database::SQLx,
                deployment: Deployment::Docker,
                testing: vec![],
                features,
            },
        }))
    }

    fn fullstack_spec_full(
        backend: Backend,
        frontend: Frontend,
        database: Database,
        deployment: Deployment,
        testing: Vec<Testing>,
    ) -> ProjectSpec {
        ProjectSpec::WebApp(WebAppSpec::Fullstack(FullstackSpec {
            frontend,
            backend: BackendSpec {
                backend,
                database,
                deployment,
                testing,
                features: vec![],
            },
        }))
    }

    #[test]
    fn axum_backend_plans_a_single_root_step() {
        let name = project_name("app");
        let spec = backend_spec(Backend::Axum, Deployment::Docker);
        let plan = template_plan(&spec, &name).expect("plan");
        assert_eq!(plan, vec![TemplateStep::root("web-axum", &name)]);
    }

    #[test]
    fn actix_backend_plans_a_single_root_step() {
        let name = project_name("app");
        let spec = backend_spec(Backend::Actix, Deployment::Docker);
        let plan = template_plan(&spec, &name).expect("plan");
        assert_eq!(plan, vec![TemplateStep::root("web-actix", &name)]);
    }

    #[test]
    fn cli_app_plans_the_cli_template() {
        let name = project_name("app");
        let plan = template_plan(&ProjectSpec::CliApp, &name).expect("plan");
        assert_eq!(plan, vec![TemplateStep::root("cli", &name)]);
    }

    #[test]
    fn library_plans_the_library_template() {
        let name = project_name("app");
        let plan = template_plan(&ProjectSpec::Library, &name).expect("plan");
        assert_eq!(plan, vec![TemplateStep::root("library", &name)]);
    }

    #[test]
    fn unsupported_stack_is_template_not_found() {
        let name = project_name("app");
        let err = template_plan(&backend_spec(Backend::Rocket, Deployment::Docker), &name)
            .expect_err("no rocket template yet");
        assert!(matches!(err, Error::TemplateNotFound(id) if id == "web-rocket"));

        let err =
            template_plan(&ProjectSpec::Embedded, &name).expect_err("no embedded template yet");
        assert!(matches!(err, Error::TemplateNotFound(id) if id == "embedded"));
    }

    #[test]
    fn fullstack_with_template_engine_plans_the_backend_template() {
        let name = project_name("app");
        let spec = fullstack_spec(Backend::Axum, Frontend::Askama);
        let plan = template_plan(&spec, &name).expect("plan");
        assert_eq!(plan, vec![TemplateStep::root("web-axum", &name)]);

        let spec = fullstack_spec(Backend::Actix, Frontend::Tera);
        let plan = template_plan(&spec, &name).expect("plan");
        assert_eq!(plan, vec![TemplateStep::root("web-actix", &name)]);
    }

    #[test]
    fn yew_frontend_plans_the_frontend_template() {
        let name = project_name("app");
        let spec = frontend_spec(Frontend::Yew, Deployment::SelfHosted);
        let plan = template_plan(&spec, &name).expect("plan");
        assert_eq!(plan, vec![TemplateStep::root("frontend-yew", &name)]);
    }

    #[test]
    fn leptos_frontend_plans_the_csr_template() {
        let name = project_name("app");
        let spec = frontend_spec(Frontend::Leptos, Deployment::SelfHosted);
        let plan = template_plan(&spec, &name).expect("plan");
        assert_eq!(plan, vec![TemplateStep::root("frontend-leptos", &name)]);
    }

    #[test]
    fn other_frontend_only_stacks_are_template_not_found() {
        let name = project_name("app");
        let spec = frontend_spec(Frontend::Seed, Deployment::SelfHosted);
        let err = template_plan(&spec, &name).expect_err("no seed template yet");
        assert!(matches!(err, Error::TemplateNotFound(id) if id == "web-seed"));
    }

    #[test]
    fn leptos_fullstack_plans_a_single_root_step() {
        let name = project_name("app");
        let spec = fullstack_spec(Backend::Axum, Frontend::Leptos);
        let plan = template_plan(&spec, &name).expect("plan");
        assert_eq!(plan, vec![TemplateStep::root("web-leptos-axum", &name)]);

        let spec = fullstack_spec(Backend::Actix, Frontend::Leptos);
        let plan = template_plan(&spec, &name).expect("plan");
        assert_eq!(plan, vec![TemplateStep::root("web-leptos-actix", &name)]);
    }

    #[test]
    fn yew_fullstack_plans_a_workspace() {
        let name = project_name("app");
        let spec = fullstack_spec(Backend::Axum, Frontend::Yew);
        let plan = template_plan(&spec, &name).expect("plan");
        assert_eq!(
            plan,
            vec![
                TemplateStep::root("web-wasm-workspace", &name),
                TemplateStep::member("web-axum", "backend", &name, "backend"),
                TemplateStep::member("frontend-yew", "frontend", &name, "frontend"),
            ]
        );
    }

    #[test]
    fn other_fullstack_wasm_frontends_are_template_not_found() {
        let name = project_name("app");
        let spec = fullstack_spec(Backend::Axum, Frontend::Seed);
        let err = template_plan(&spec, &name).expect_err("no seed fullstack template yet");
        assert!(matches!(err, Error::TemplateNotFound(id) if id == "web-fullstack-seed"));

        // Leptos integrates with axum and actix only.
        let spec = fullstack_spec(Backend::Rocket, Frontend::Leptos);
        let err = template_plan(&spec, &name).expect_err("no rocket leptos template yet");
        assert!(matches!(err, Error::TemplateNotFound(id) if id == "web-leptos-rocket"));
    }

    #[test]
    fn yew_fullstack_renders_a_workspace() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let name = project_name("app");
        let spec = fullstack_spec(Backend::Axum, Frontend::Yew);
        let plan = template_plan(&spec, &name).expect("plan");

        render_plan(&plan, &spec, &name, tmp.path()).expect("render");

        let root = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read workspace");
        assert!(root.contains("[workspace]"));
        assert!(root.contains("\"backend\""));
        let backend = fs::read_to_string(tmp.path().join("backend/Cargo.toml")).expect("read");
        assert!(backend.contains("name = \"app-backend\""));
        assert!(backend.contains("tower-http"));
        let main = fs::read_to_string(tmp.path().join("backend/src/main.rs")).expect("read");
        assert!(main.contains("/api/hello"));
        assert!(main.contains("ServeDir"));
        let frontend = fs::read_to_string(tmp.path().join("frontend/Cargo.toml")).expect("read");
        assert!(frontend.contains("name = \"app-frontend\""));
        assert!(frontend.contains("gloo-net"));
        assert!(tmp.path().join("frontend/Trunk.toml").exists());
        // The workspace root ships the Dockerfile and .gitignore.
        assert!(tmp.path().join("Dockerfile").exists());
        assert!(!tmp.path().join("backend/Dockerfile").exists());
        assert!(!tmp.path().join("backend/.gitignore").exists());
    }

    #[test]
    fn multi_step_plan_renders_into_subdirectories() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let name = project_name("app");
        let plan = vec![
            TemplateStep {
                id: "cli",
                subdir: "tool",
                crate_name: "app-tool".to_owned(),
            },
            TemplateStep {
                id: "library",
                subdir: "core",
                crate_name: "app-core".to_owned(),
            },
        ];

        render_plan(&plan, &ProjectSpec::CliApp, &name, tmp.path()).expect("render plan");

        let tool = fs::read_to_string(tmp.path().join("tool/Cargo.toml")).expect("read");
        assert!(tool.contains("name = \"app-tool\""));
        let core = fs::read_to_string(tmp.path().join("core/Cargo.toml")).expect("read");
        assert!(core.contains("name = \"app-core\""));
        assert!(tmp.path().join("tool/src/main.rs").exists());
        assert!(tmp.path().join("core/src/lib.rs").exists());
    }

    #[test]
    fn docker_deployment_keeps_dockerfile() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-axum").expect("template");

        render_test(
            &template,
            &backend_spec(Backend::Axum, Deployment::Docker),
            "app",
            tmp.path(),
        )
        .expect("render");

        assert!(tmp.path().join("Dockerfile").exists());
        assert!(tmp.path().join("src/main.rs").exists());
        assert!(!tmp.path().join("src/main.rs.j2").exists());
    }

    #[test]
    fn non_docker_deployment_excludes_dockerfile() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-axum").expect("template");

        render_test(
            &template,
            &backend_spec(Backend::Axum, Deployment::Kubernetes),
            "app",
            tmp.path(),
        )
        .expect("render");

        assert!(!tmp.path().join("Dockerfile").exists());
        assert!(tmp.path().join("Cargo.toml").exists());
        assert!(tmp.path().join(".gitignore").exists());
    }

    #[test]
    fn rendered_files_contain_project_name() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-axum").expect("template");

        render_test(
            &template,
            &backend_spec(Backend::Axum, Deployment::Docker),
            "super-app",
            tmp.path(),
        )
        .expect("render");

        let cargo_toml = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read");
        assert!(cargo_toml.contains("name = \"super-app\""));
    }

    #[test]
    fn actix_template_renders_project_name() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-actix").expect("template");

        render_test(
            &template,
            &backend_spec(Backend::Actix, Deployment::Docker),
            "super-app",
            tmp.path(),
        )
        .expect("render");

        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(main_rs.contains("Hello from super-app!"));
        let cargo_toml = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read");
        assert!(cargo_toml.contains("actix-web"));
    }

    #[test]
    fn actix_non_docker_deployment_excludes_dockerfile() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-actix").expect("template");

        render_test(
            &template,
            &backend_spec(Backend::Actix, Deployment::Kubernetes),
            "app",
            tmp.path(),
        )
        .expect("render");

        assert!(!tmp.path().join("Dockerfile").exists());
        assert!(tmp.path().join("src/main.rs").exists());
    }

    #[test]
    fn cli_template_renders_clap_scaffold() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("cli").expect("template");

        render_test(&template, &ProjectSpec::CliApp, "greeter", tmp.path()).expect("render");

        let cargo_toml = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read");
        assert!(cargo_toml.contains("name = \"greeter\""));
        assert!(cargo_toml.contains("clap"));
        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(main_rs.contains("greeter"));
        assert!(!tmp.path().join("src/lib.rs").exists());
    }

    #[test]
    fn library_template_renders_lib_with_test_stub() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("library").expect("template");

        render_test(&template, &ProjectSpec::Library, "mathlib", tmp.path()).expect("render");

        let lib_rs = fs::read_to_string(tmp.path().join("src/lib.rs")).expect("read");
        assert!(lib_rs.contains("mathlib"));
        assert!(lib_rs.contains("#[cfg(test)]"));
        assert!(!tmp.path().join("src/main.rs").exists());
    }

    #[test]
    fn dir_source_renders_the_custom_template() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let template_dir = tmp.path().join("my-template");
        fs::create_dir(&template_dir).expect("mkdir");
        fs::write(
            template_dir.join("template.toml"),
            "name = \"custom\"\ndescription = \"test template\"",
        )
        .expect("write manifest");
        fs::write(template_dir.join("README.md.j2"), "# {{ name }}").expect("write file");

        let target_dir = tmp.path().join("out");
        fs::create_dir(&target_dir).expect("mkdir");
        let spec = ProjectSpec::CliApp;
        let template = Template::from_dir(&template_dir).expect("load custom template");
        render_test(&template, &spec, "custom-app", &target_dir).expect("render");

        let readme = fs::read_to_string(target_dir.join("README.md")).expect("read output");
        assert_eq!(readme, "# custom-app");
    }

    #[test]
    fn non_empty_target_is_rejected() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let target = tmp.path().join("occupied");
        fs::create_dir(&target).expect("mkdir");
        fs::write(target.join("file"), "x").expect("write");

        let err = prepare_target(&target).expect_err("must reject non-empty dir");
        assert!(matches!(err, Error::TargetNotEmpty(_)));
    }

    #[test]
    fn sqlx_database_adds_pool_module_and_env_example() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-axum").expect("template");

        render_test(
            &template,
            &backend_spec(Backend::Axum, Deployment::Docker),
            "app",
            tmp.path(),
        )
        .expect("render");

        assert!(tmp.path().join("src/db.rs").exists());
        assert!(tmp.path().join(".env.example").exists());
        let cargo_toml = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read");
        assert!(cargo_toml.contains("sqlx"));
        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(main_rs.contains("mod db;"));
        assert!(main_rs.contains("with_state(db)"));
        let gitignore = fs::read_to_string(tmp.path().join(".gitignore")).expect("read");
        assert!(gitignore.contains(".env"));
    }

    #[test]
    fn diesel_wires_a_connection_pool() {
        let spec = ProjectSpec::WebApp(WebAppSpec::Backend(BackendSpec {
            backend: Backend::Axum,
            database: Database::Diesel,
            deployment: Deployment::Docker,
            testing: vec![],
            features: vec![],
        }));
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-axum").expect("template");

        render_test(&template, &spec, "app", tmp.path()).expect("render");

        assert!(tmp.path().join("src/db.rs").exists());
        assert!(tmp.path().join(".env.example").exists());
        let cargo_toml = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read");
        assert!(cargo_toml.contains("diesel"));
        assert!(cargo_toml.contains("r2d2"));
        assert!(!cargo_toml.contains("sqlx"));
        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(main_rs.contains("mod db"));
        assert!(main_rs.contains("db::create_pool()"));
        let dockerfile = fs::read_to_string(tmp.path().join("Dockerfile")).expect("read");
        assert!(dockerfile.contains("libpq"));
    }

    #[test]
    fn sled_opens_an_embedded_store() {
        let spec = ProjectSpec::WebApp(WebAppSpec::Backend(BackendSpec {
            backend: Backend::Axum,
            database: Database::Sled,
            deployment: Deployment::SelfHosted,
            testing: vec![],
            features: vec![],
        }));
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-axum").expect("template");

        render_test(&template, &spec, "app", tmp.path()).expect("render");

        assert!(tmp.path().join("src/db.rs").exists());
        let env_example = fs::read_to_string(tmp.path().join(".env.example")).expect("read");
        assert!(env_example.contains("DB_PATH"));
        let gitignore = fs::read_to_string(tmp.path().join(".gitignore")).expect("read");
        assert!(gitignore.contains("/data"));
        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(main_rs.contains("db::open()"));
    }

    #[test]
    fn leptos_fullstack_renders_crate_identifiers() {
        // A hyphenated crate name needs its underscored form in Rust code.
        let spec = fullstack_spec(Backend::Axum, Frontend::Leptos);
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-leptos-axum").expect("template");

        render_test(&template, &spec, "my-app", tmp.path()).expect("render");

        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(main_rs.contains("use my_app::app::*;"));
        let cargo_toml = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read");
        assert!(cargo_toml.contains("name = \"my-app\""));
        let app_rs = fs::read_to_string(tmp.path().join("src/app.rs")).expect("read");
        assert!(app_rs.contains("/pkg/my-app.css"));
        // Database wiring ships with the leptos templates.
        assert!(tmp.path().join("src/db.rs").exists());
        assert!(tmp.path().join(".env.example").exists());
    }

    #[test]
    fn actix_sqlx_shares_the_pool_via_app_data() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-actix").expect("template");

        render_test(
            &template,
            &backend_spec(Backend::Actix, Deployment::Docker),
            "app",
            tmp.path(),
        )
        .expect("render");

        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(main_rs.contains("mod db;"));
        assert!(main_rs.contains("app_data(actix_web::web::Data::new(pool.clone()))"));
    }

    #[test]
    fn api_testing_adds_test_module_and_dev_dependency() {
        let spec = backend_spec_full(
            Backend::Axum,
            Database::SQLx,
            Deployment::Docker,
            vec![Testing::Api, Testing::Mock],
            vec![],
        );
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-axum").expect("template");

        render_test(&template, &spec, "app", tmp.path()).expect("render");

        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(main_rs.contains("#[cfg(test)]"));
        assert!(main_rs.contains("oneshot"));
        let cargo_toml = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read");
        assert!(cargo_toml.contains("[dev-dependencies]"));
        assert!(cargo_toml.contains("tower"));
        assert!(cargo_toml.contains("mockall"));
    }

    #[test]
    fn sqlx_testing_adds_pool_test_only_with_sqlx_database() {
        let with_sqlx = backend_spec_full(
            Backend::Axum,
            Database::SQLx,
            Deployment::Docker,
            vec![Testing::SQLx],
            vec![],
        );
        let with_diesel = backend_spec_full(
            Backend::Axum,
            Database::Diesel,
            Deployment::Docker,
            vec![Testing::SQLx],
            vec![],
        );
        let template = Template::embedded("web-axum").expect("template");

        let tmp = tempfile::tempdir().expect("tempdir");
        render_test(&template, &with_sqlx, "app", tmp.path()).expect("render");
        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(main_rs.contains("mod db_tests"));

        let tmp = tempfile::tempdir().expect("tempdir");
        render_test(&template, &with_diesel, "app", tmp.path()).expect("render");
        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(
            !main_rs.contains("mod db_tests"),
            "no sqlx pool to test without sqlx"
        );
    }

    #[test]
    fn actix_api_testing_needs_no_dev_dependencies() {
        let spec = backend_spec_full(
            Backend::Actix,
            Database::SQLx,
            Deployment::Docker,
            vec![Testing::Api],
            vec![],
        );
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-actix").expect("template");

        render_test(&template, &spec, "app", tmp.path()).expect("render");

        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(main_rs.contains("actix_web::test"));
        let cargo_toml = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read");
        assert!(!cargo_toml.contains("[dev-dependencies]"));
    }

    #[test]
    fn websockets_add_ws_route_and_axum_feature() {
        let spec = backend_spec_full(
            Backend::Axum,
            Database::SQLx,
            Deployment::Docker,
            vec![],
            vec![Feature::WebSockets],
        );
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-axum").expect("template");

        render_test(&template, &spec, "app", tmp.path()).expect("render");

        let cargo_toml = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read");
        assert!(cargo_toml.contains("features = [\"ws\"]"));
        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(main_rs.contains("route(\"/ws\", get(ws_handler))"));
        assert!(main_rs.contains("WebSocketUpgrade"));
    }

    #[test]
    fn actix_websockets_add_actix_ws_endpoint() {
        let spec = backend_spec_full(
            Backend::Actix,
            Database::SQLx,
            Deployment::Docker,
            vec![],
            vec![Feature::WebSockets],
        );
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-actix").expect("template");

        render_test(&template, &spec, "app", tmp.path()).expect("render");

        let cargo_toml = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read");
        assert!(cargo_toml.contains("actix-ws"));
        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(main_rs.contains("service(ws)"));
    }

    #[test]
    fn without_websockets_no_ws_content() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-axum").expect("template");

        render_test(
            &template,
            &backend_spec(Backend::Axum, Deployment::Docker),
            "app",
            tmp.path(),
        )
        .expect("render");

        let cargo_toml = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read");
        assert!(!cargo_toml.contains("features = [\"ws\"]"));
        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(!main_rs.contains("/ws"));
    }

    // --- Fullstack with a server-side template engine ---

    #[test]
    fn axum_askama_fullstack_renders_template_at_compile_time() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let spec = fullstack_spec(Backend::Axum, Frontend::Askama);
        let template = Template::embedded("web-axum").expect("template");

        render_test(&template, &spec, "app", tmp.path()).expect("render");

        let cargo_toml = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read");
        assert!(cargo_toml.contains("askama"));
        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(main_rs.contains("IndexTemplate"));
        assert!(main_rs.contains("mod db;"));
        assert!(tmp.path().join("templates/index.html").exists());
        assert!(!tmp.path().join("templates/index.hbs").exists());
    }

    #[test]
    fn axum_handlebars_fullstack_uses_hbs_templates() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let spec = fullstack_spec(Backend::Axum, Frontend::Handlebars);
        let template = Template::embedded("web-axum").expect("template");

        render_test(&template, &spec, "app", tmp.path()).expect("render");

        let cargo_toml = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read");
        assert!(cargo_toml.contains("handlebars"));
        assert!(cargo_toml.contains("serde_json"));
        assert!(tmp.path().join("templates/index.hbs").exists());
        assert!(!tmp.path().join("templates/index.html").exists());
    }

    #[test]
    fn actix_tera_fullstack_embeds_templates() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let spec = fullstack_spec(Backend::Actix, Frontend::Tera);
        let template = Template::embedded("web-actix").expect("template");

        render_test(&template, &spec, "app", tmp.path()).expect("render");

        let cargo_toml = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read");
        assert!(cargo_toml.contains("tera"));
        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(main_rs.contains("TEMPLATES"));
        assert!(main_rs.contains("add_raw_template"));
        assert!(tmp.path().join("templates/index.html").exists());
    }

    #[test]
    fn actix_minijinja_fullstack_renders_with_context_macro() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let spec = fullstack_spec(Backend::Actix, Frontend::Minijinja);
        let template = Template::embedded("web-actix").expect("template");

        render_test(&template, &spec, "app", tmp.path()).expect("render");

        let cargo_toml = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read");
        assert!(cargo_toml.contains("minijinja"));
        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(main_rs.contains("context!"));
    }

    // --- Output snapshots ---
    //
    // The whole generated tree as one deterministic string, snapshotted with
    // insta. Baseline for template content changes: any diff in generated
    // output shows up here. Update with `INSTA_UPDATE=always cargo test`
    // (or `cargo insta accept`) after a deliberate template change.

    #[test]
    fn snapshot_web_axum_docker() {
        insta::assert_snapshot!(snapshot_output(&backend_spec(
            Backend::Axum,
            Deployment::Docker
        )));
    }

    #[test]
    fn snapshot_web_axum_kubernetes() {
        insta::assert_snapshot!(snapshot_output(&backend_spec(
            Backend::Axum,
            Deployment::Kubernetes
        )));
    }

    #[test]
    fn snapshot_web_actix_docker() {
        insta::assert_snapshot!(snapshot_output(&backend_spec(
            Backend::Actix,
            Deployment::Docker
        )));
    }

    #[test]
    fn snapshot_web_actix_kubernetes() {
        insta::assert_snapshot!(snapshot_output(&backend_spec(
            Backend::Actix,
            Deployment::Kubernetes
        )));
    }

    #[test]
    fn snapshot_web_axum_all_options() {
        insta::assert_snapshot!(snapshot_output(&backend_spec_full(
            Backend::Axum,
            Database::SQLx,
            Deployment::Docker,
            vec![Testing::Api, Testing::SQLx, Testing::Mock],
            vec![Feature::WebSockets],
        )));
    }

    #[test]
    fn snapshot_web_actix_all_options() {
        insta::assert_snapshot!(snapshot_output(&backend_spec_full(
            Backend::Actix,
            Database::SQLx,
            Deployment::Docker,
            vec![Testing::Api, Testing::SQLx, Testing::Mock],
            vec![Feature::WebSockets],
        )));
    }

    #[test]
    fn snapshot_web_axum_diesel() {
        insta::assert_snapshot!(snapshot_output(&backend_spec_full(
            Backend::Axum,
            Database::Diesel,
            Deployment::Kubernetes,
            vec![],
            vec![],
        )));
    }

    #[test]
    fn snapshot_web_actix_diesel() {
        insta::assert_snapshot!(snapshot_output(&backend_spec_full(
            Backend::Actix,
            Database::Diesel,
            Deployment::Docker,
            vec![],
            vec![],
        )));
    }

    #[test]
    fn snapshot_web_axum_sled() {
        insta::assert_snapshot!(snapshot_output(&backend_spec_full(
            Backend::Axum,
            Database::Sled,
            Deployment::SelfHosted,
            vec![],
            vec![],
        )));
    }

    #[test]
    fn snapshot_web_actix_sled() {
        insta::assert_snapshot!(snapshot_output(&backend_spec_full(
            Backend::Actix,
            Database::Sled,
            Deployment::SelfHosted,
            vec![],
            vec![],
        )));
    }

    #[test]
    fn snapshot_web_axum_rate_limiting() {
        insta::assert_snapshot!(snapshot_output(&backend_spec_full(
            Backend::Axum,
            Database::SQLx,
            Deployment::Docker,
            vec![],
            vec![Feature::RateLimiting],
        )));
    }

    #[test]
    fn snapshot_web_actix_rate_limiting() {
        insta::assert_snapshot!(snapshot_output(&backend_spec_full(
            Backend::Actix,
            Database::SQLx,
            Deployment::Docker,
            vec![],
            vec![Feature::RateLimiting],
        )));
    }

    #[test]
    fn snapshot_web_axum_graphql() {
        insta::assert_snapshot!(snapshot_output(&backend_spec_full(
            Backend::Axum,
            Database::SQLx,
            Deployment::Docker,
            vec![],
            vec![Feature::GraphQL],
        )));
    }

    #[test]
    fn snapshot_web_axum_grpc() {
        insta::assert_snapshot!(snapshot_output(&backend_spec_full(
            Backend::Axum,
            Database::SQLx,
            Deployment::Docker,
            vec![],
            vec![Feature::gRPC],
        )));
    }

    #[test]
    fn snapshot_web_actix_graphql() {
        insta::assert_snapshot!(snapshot_output(&backend_spec_full(
            Backend::Actix,
            Database::SQLx,
            Deployment::Docker,
            vec![],
            vec![Feature::GraphQL],
        )));
    }

    #[test]
    fn snapshot_web_actix_grpc() {
        insta::assert_snapshot!(snapshot_output(&backend_spec_full(
            Backend::Actix,
            Database::SQLx,
            Deployment::Docker,
            vec![],
            vec![Feature::gRPC],
        )));
    }

    #[test]
    fn snapshot_web_axum_sea_orm() {
        insta::assert_snapshot!(snapshot_output(&backend_spec_full(
            Backend::Axum,
            Database::SeaORM,
            Deployment::Docker,
            vec![],
            vec![],
        )));
    }

    #[test]
    fn snapshot_web_actix_sea_orm() {
        insta::assert_snapshot!(snapshot_output(&backend_spec_full(
            Backend::Actix,
            Database::SeaORM,
            Deployment::Docker,
            vec![],
            vec![],
        )));
    }

    #[test]
    fn snapshot_cli() {
        insta::assert_snapshot!(snapshot_output(&ProjectSpec::CliApp));
    }

    #[test]
    fn snapshot_frontend_yew_docker() {
        insta::assert_snapshot!(snapshot_output(&frontend_spec(
            Frontend::Yew,
            Deployment::Docker
        )));
    }

    #[test]
    fn snapshot_frontend_yew_self_hosted() {
        insta::assert_snapshot!(snapshot_output(&frontend_spec(
            Frontend::Yew,
            Deployment::SelfHosted
        )));
    }

    #[test]
    fn snapshot_frontend_leptos_docker() {
        insta::assert_snapshot!(snapshot_output(&frontend_spec(
            Frontend::Leptos,
            Deployment::Docker
        )));
    }

    #[test]
    fn snapshot_frontend_leptos_self_hosted() {
        insta::assert_snapshot!(snapshot_output(&frontend_spec(
            Frontend::Leptos,
            Deployment::SelfHosted
        )));
    }

    #[test]
    fn snapshot_library() {
        insta::assert_snapshot!(snapshot_output(&ProjectSpec::Library));
    }

    #[test]
    fn snapshot_web_axum_fullstack_askama() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec(
            Backend::Axum,
            Frontend::Askama
        )));
    }

    #[test]
    fn snapshot_web_axum_fullstack_tera() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec(
            Backend::Axum,
            Frontend::Tera
        )));
    }

    #[test]
    fn snapshot_web_axum_fullstack_minijinja() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec(
            Backend::Axum,
            Frontend::Minijinja
        )));
    }

    #[test]
    fn snapshot_web_axum_fullstack_handlebars() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec(
            Backend::Axum,
            Frontend::Handlebars
        )));
    }

    #[test]
    fn snapshot_web_actix_fullstack_askama() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec(
            Backend::Actix,
            Frontend::Askama
        )));
    }

    #[test]
    fn snapshot_web_actix_fullstack_tera() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec(
            Backend::Actix,
            Frontend::Tera
        )));
    }

    #[test]
    fn snapshot_web_actix_fullstack_minijinja() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec(
            Backend::Actix,
            Frontend::Minijinja
        )));
    }

    #[test]
    fn snapshot_web_actix_fullstack_handlebars() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec(
            Backend::Actix,
            Frontend::Handlebars
        )));
    }

    #[test]
    fn snapshot_web_axum_fullstack_yew() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec(
            Backend::Axum,
            Frontend::Yew
        )));
    }

    #[test]
    fn snapshot_web_actix_fullstack_yew() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec(
            Backend::Actix,
            Frontend::Yew
        )));
    }

    #[test]
    fn snapshot_web_axum_fullstack_yew_grpc() {
        // gRPC in a WASM workspace: the root Dockerfile builds the backend,
        // so it is the file that must install protobuf-compiler.
        insta::assert_snapshot!(snapshot_output(&ProjectSpec::WebApp(
            WebAppSpec::Fullstack(FullstackSpec {
                frontend: Frontend::Yew,
                backend: BackendSpec {
                    backend: Backend::Axum,
                    database: Database::SQLx,
                    deployment: Deployment::Docker,
                    testing: vec![],
                    features: vec![Feature::gRPC],
                },
            })
        )));
    }

    #[test]
    fn snapshot_web_axum_fullstack_leptos() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec(
            Backend::Axum,
            Frontend::Leptos
        )));
    }

    #[test]
    fn snapshot_web_actix_fullstack_leptos() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec(
            Backend::Actix,
            Frontend::Leptos
        )));
    }

    // Diesel covers the blocking wrapper (spawn_blocking / web::block) and
    // the libpq lines in the Dockerfile on both backends.
    #[test]
    fn snapshot_web_axum_fullstack_leptos_diesel() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec_with_db(
            Backend::Axum,
            Frontend::Leptos,
            Database::Diesel
        )));
    }

    #[test]
    fn snapshot_web_actix_fullstack_leptos_diesel() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec_with_db(
            Backend::Actix,
            Frontend::Leptos,
            Database::Diesel
        )));
    }

    #[test]
    fn snapshot_web_axum_fullstack_leptos_sea_orm() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec_with_db(
            Backend::Axum,
            Frontend::Leptos,
            Database::SeaORM
        )));
    }

    // sled covers DB_PATH in .env.example and /data in .gitignore.
    #[test]
    fn snapshot_web_actix_fullstack_leptos_sled() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec_with_db(
            Backend::Actix,
            Frontend::Leptos,
            Database::Sled
        )));
    }

    // Testing options in a Leptos app: the server-function API test and
    // the sqlx pool test, both gated on the `ssr` feature.
    #[test]
    fn snapshot_web_axum_fullstack_leptos_testing() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec_full(
            Backend::Axum,
            Frontend::Leptos,
            Database::SQLx,
            Deployment::Docker,
            vec![Testing::Api, Testing::SQLx],
        )));
    }

    #[test]
    fn snapshot_web_actix_fullstack_leptos_testing_sled() {
        // sled runs the API test live (embedded store, no Postgres).
        insta::assert_snapshot!(snapshot_output(&fullstack_spec_full(
            Backend::Actix,
            Frontend::Leptos,
            Database::Sled,
            Deployment::Docker,
            vec![Testing::Api],
        )));
    }

    // The WebSocket echo endpoint sits next to the leptos routes on both
    // backends (axum `ws` feature / actix-ws), gated on `ssr`.
    #[test]
    fn snapshot_web_axum_fullstack_leptos_web_sockets() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec_featured(
            Backend::Axum,
            Frontend::Leptos,
            vec![Feature::WebSockets],
        )));
    }

    #[test]
    fn snapshot_web_actix_fullstack_leptos_web_sockets() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec_featured(
            Backend::Actix,
            Frontend::Leptos,
            vec![Feature::WebSockets],
        )));
    }

    // Per-IP rate limiting wraps the whole leptos router (axum serves
    // through into_make_service_with_connect_info; actix wraps Governor).
    #[test]
    fn snapshot_web_axum_fullstack_leptos_rate_limiting() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec_featured(
            Backend::Axum,
            Frontend::Leptos,
            vec![Feature::RateLimiting],
        )));
    }

    #[test]
    fn snapshot_web_actix_fullstack_leptos_rate_limiting() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec_featured(
            Backend::Actix,
            Frontend::Leptos,
            vec![Feature::RateLimiting],
        )));
    }

    // The api test module switches routes under a WASM frontend.
    #[test]
    fn snapshot_web_axum_fullstack_yew_api_testing() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec_tested(
            Backend::Axum,
            Frontend::Yew
        )));
    }

    #[test]
    fn snapshot_web_actix_fullstack_yew_api_testing() {
        insta::assert_snapshot!(snapshot_output(&fullstack_spec_tested(
            Backend::Actix,
            Frontend::Yew
        )));
    }

    /// Render `spec` into a tempdir and return every generated file as one
    /// sorted `=== path ===` listing. Path separators are normalized so
    /// snapshots match across platforms.
    fn snapshot_output(spec: &ProjectSpec) -> String {
        let tmp = tempfile::tempdir().expect("tempdir");
        let name = project_name("sample-app");
        let plan = template_plan(spec, &name).expect("plan");
        render_plan(&plan, spec, &name, tmp.path()).expect("render");

        let mut files = Vec::new();
        collect_files(tmp.path(), tmp.path(), &mut files);
        files.sort();

        let mut out = String::new();
        for rel in files {
            let contents = fs::read_to_string(tmp.path().join(&rel)).expect("utf8 file");
            out.push_str(&format!("=== {rel} ===\n{contents}\n"));
        }
        out
    }

    fn collect_files(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                collect_files(root, &path, out);
            } else {
                let rel = path.strip_prefix(root).expect("under root");
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
}
