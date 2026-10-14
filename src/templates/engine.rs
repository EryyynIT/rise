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
use crate::spec::{Backend, BackendSpec, Frontend, ProjectSpec, WebAppSpec};
use crate::templates::template::{RENDER_SUFFIX, Template, TemplateFile};

/// Where to load the template from.
#[derive(Debug, Default)]
pub(crate) enum TemplateSource {
    /// The embedded template implementing the spec's stack.
    #[default]
    Embedded,
    /// A template directory on disk, used instead of the embedded one
    /// (`--template`). The spec is still collected as usual and forms the
    /// render context.
    Dir(PathBuf),
}

impl TemplateSource {
    fn load(&self, spec: &ProjectSpec) -> Result<Template> {
        match self {
            Self::Embedded => Template::embedded(template_id(spec)?),
            Self::Dir(path) => Template::from_dir(path),
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
    let template = source.load(spec)?;
    let target = PathBuf::from(name.as_str());
    let target_existed = prepare_target(&target)?;

    let result = render_into(&template, spec, name.as_str(), &target);
    if result.is_err() && !target_existed {
        // Never leave a half-written project behind.
        let _ = fs::remove_dir_all(&target);
    }
    result?;
    Ok(target)
}

/// Map a spec to the name of the template implementing its stack.
fn template_id(spec: &ProjectSpec) -> Result<&'static str> {
    match spec {
        ProjectSpec::WebApp(WebAppSpec::Backend(backend)) => match backend.backend {
            Backend::Axum => Ok("web-axum"),
            Backend::Actix => Ok("web-actix"),
            other => Err(Error::TemplateNotFound(format!(
                "web-{}",
                other.to_string().to_lowercase()
            ))),
        },
        ProjectSpec::WebApp(WebAppSpec::Frontend(_)) => {
            Err(Error::TemplateNotFound("web-frontend".into()))
        }
        // A fullstack app with a template engine is the backend template
        // plus server-side rendering; the `frontend` context field drives
        // the engine-specific blocks.
        ProjectSpec::WebApp(WebAppSpec::Fullstack(fullstack))
            if fullstack.frontend.is_template_engine() =>
        {
            match fullstack.backend.backend {
                Backend::Axum => Ok("web-axum"),
                Backend::Actix => Ok("web-actix"),
                other => Err(Error::TemplateNotFound(format!(
                    "web-{}",
                    other.to_string().to_lowercase()
                ))),
            }
        }
        ProjectSpec::WebApp(WebAppSpec::Fullstack(_)) => {
            Err(Error::TemplateNotFound("web-fullstack".into()))
        }
        ProjectSpec::CliApp => Ok("cli"),
        ProjectSpec::Library => Ok("library"),
        ProjectSpec::Embedded => Err(Error::TemplateNotFound("embedded".into())),
        ProjectSpec::Game => Err(Error::TemplateNotFound("game".into())),
    }
}

/// Variables available to every template: the project name, the full spec,
/// and the web stacks flattened out of the spec's enum nesting.
///
/// Templates use `backend`/`frontend` rather than digging into
/// `spec.WebApp.Backend` vs `spec.WebApp.Fullstack.backend`: strict
/// undefined behavior makes spec access shape-dependent, the flattened
/// fields are not. `None` serializes as a minijinja `none`, so
/// `{% if backend %}` guards every access.
#[derive(Serialize)]
struct RenderContext<'a> {
    name: &'a str,
    spec: &'a ProjectSpec,
    backend: Option<&'a BackendSpec>,
    frontend: Option<Frontend>,
}

impl<'a> RenderContext<'a> {
    fn new(name: &'a str, spec: &'a ProjectSpec) -> Self {
        let (backend, frontend) = match spec {
            ProjectSpec::WebApp(WebAppSpec::Backend(backend)) => (Some(backend), None),
            ProjectSpec::WebApp(WebAppSpec::Frontend(frontend)) => (None, Some(frontend.frontend)),
            ProjectSpec::WebApp(WebAppSpec::Fullstack(fullstack)) => {
                (Some(&fullstack.backend), Some(fullstack.frontend))
            }
            _ => (None, None),
        };
        Self {
            name,
            spec,
            backend,
            frontend,
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

/// Render every template file into `target`.
fn render_into(template: &Template, spec: &ProjectSpec, name: &str, target: &Path) -> Result<()> {
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

    let ctx = Value::from(Serde(RenderContext::new(name, spec)));
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
    use crate::spec::{BackendSpec, Database, Deployment, Feature, FullstackSpec, Testing};

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

    #[test]
    fn axum_backend_maps_to_web_axum() {
        let spec = backend_spec(Backend::Axum, Deployment::Docker);
        assert_eq!(template_id(&spec).expect("template id"), "web-axum");
    }

    #[test]
    fn actix_backend_maps_to_web_actix() {
        let spec = backend_spec(Backend::Actix, Deployment::Docker);
        assert_eq!(template_id(&spec).expect("template id"), "web-actix");
    }

    #[test]
    fn cli_app_maps_to_cli() {
        assert_eq!(
            template_id(&ProjectSpec::CliApp).expect("template id"),
            "cli"
        );
    }

    #[test]
    fn library_maps_to_library() {
        assert_eq!(
            template_id(&ProjectSpec::Library).expect("template id"),
            "library"
        );
    }

    #[test]
    fn unsupported_stack_is_template_not_found() {
        let err = template_id(&backend_spec(Backend::Rocket, Deployment::Docker))
            .expect_err("no rocket template yet");
        assert!(matches!(err, Error::TemplateNotFound(id) if id == "web-rocket"));

        let err = template_id(&ProjectSpec::Embedded).expect_err("no embedded template yet");
        assert!(matches!(err, Error::TemplateNotFound(id) if id == "embedded"));
    }

    #[test]
    fn fullstack_with_template_engine_maps_to_the_backend_template() {
        let spec = fullstack_spec(Backend::Axum, Frontend::Askama);
        assert_eq!(template_id(&spec).expect("template id"), "web-axum");

        let spec = fullstack_spec(Backend::Actix, Frontend::Tera);
        assert_eq!(template_id(&spec).expect("template id"), "web-actix");
    }

    #[test]
    fn fullstack_with_wasm_frontend_is_template_not_found() {
        let spec = fullstack_spec(Backend::Axum, Frontend::Yew);
        let err = template_id(&spec).expect_err("no yew fullstack template yet");
        assert!(matches!(err, Error::TemplateNotFound(id) if id == "web-fullstack"));
    }

    #[test]
    fn docker_deployment_keeps_dockerfile() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-axum").expect("template");

        render_into(
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

        render_into(
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

        render_into(
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

        render_into(
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

        render_into(
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

        render_into(&template, &ProjectSpec::CliApp, "greeter", tmp.path()).expect("render");

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

        render_into(&template, &ProjectSpec::Library, "mathlib", tmp.path()).expect("render");

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
        let source = TemplateSource::Dir(template_dir);
        let template = source.load(&spec).expect("load custom template");
        render_into(&template, &spec, "custom-app", &target_dir).expect("render");

        let readme = fs::read_to_string(target_dir.join("README.md")).expect("read output");
        assert_eq!(readme, "# custom-app");
    }

    #[test]
    fn embedded_source_maps_the_spec() {
        let spec = ProjectSpec::Library;
        let template = TemplateSource::Embedded
            .load(&spec)
            .expect("embedded template");
        assert_eq!(template.manifest().name, "library");
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

        render_into(
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
        assert!(main_rs.contains("with_state(pool)"));
        let gitignore = fs::read_to_string(tmp.path().join(".gitignore")).expect("read");
        assert!(gitignore.contains(".env"));
    }

    #[test]
    fn non_sqlx_database_adds_only_the_dependency() {
        let spec = ProjectSpec::WebApp(WebAppSpec::Backend(BackendSpec {
            backend: Backend::Axum,
            database: Database::Diesel,
            deployment: Deployment::Docker,
            testing: vec![],
            features: vec![],
        }));
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-axum").expect("template");

        render_into(&template, &spec, "app", tmp.path()).expect("render");

        assert!(!tmp.path().join("src/db.rs").exists());
        assert!(!tmp.path().join(".env.example").exists());
        let cargo_toml = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read");
        assert!(cargo_toml.contains("diesel"));
        assert!(!cargo_toml.contains("sqlx"));
        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(!main_rs.contains("mod db"));
    }

    #[test]
    fn actix_sqlx_shares_the_pool_via_app_data() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-actix").expect("template");

        render_into(
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

        render_into(&template, &spec, "app", tmp.path()).expect("render");

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
        render_into(&template, &with_sqlx, "app", tmp.path()).expect("render");
        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(main_rs.contains("mod db_tests"));

        let tmp = tempfile::tempdir().expect("tempdir");
        render_into(&template, &with_diesel, "app", tmp.path()).expect("render");
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

        render_into(&template, &spec, "app", tmp.path()).expect("render");

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

        render_into(&template, &spec, "app", tmp.path()).expect("render");

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

        render_into(&template, &spec, "app", tmp.path()).expect("render");

        let cargo_toml = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("read");
        assert!(cargo_toml.contains("actix-ws"));
        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).expect("read");
        assert!(main_rs.contains("service(ws)"));
    }

    #[test]
    fn without_websockets_no_ws_content() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let template = Template::embedded("web-axum").expect("template");

        render_into(
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

        render_into(&template, &spec, "app", tmp.path()).expect("render");

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

        render_into(&template, &spec, "app", tmp.path()).expect("render");

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

        render_into(&template, &spec, "app", tmp.path()).expect("render");

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

        render_into(&template, &spec, "app", tmp.path()).expect("render");

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
    fn snapshot_web_axum_diesel_deps_only() {
        insta::assert_snapshot!(snapshot_output(&backend_spec_full(
            Backend::Axum,
            Database::Diesel,
            Deployment::Kubernetes,
            vec![],
            vec![],
        )));
    }

    #[test]
    fn snapshot_cli() {
        insta::assert_snapshot!(snapshot_output(&ProjectSpec::CliApp));
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

    /// Render `spec` into a tempdir and return every generated file as one
    /// sorted `=== path ===` listing. Path separators are normalized so
    /// snapshots match across platforms.
    fn snapshot_output(spec: &ProjectSpec) -> String {
        let tmp = tempfile::tempdir().expect("tempdir");
        let template =
            Template::embedded(template_id(spec).expect("template id")).expect("embedded template");
        render_into(&template, spec, "sample-app", tmp.path()).expect("render");

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
