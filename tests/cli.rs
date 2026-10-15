//! End-to-end CLI tests: the real binary, real arguments, real filesystem.

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn list_prints_embedded_templates() {
    Command::cargo_bin("rise")
        .expect("binary")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("web-axum"));
}

#[test]
fn without_tty_missing_name_is_an_actionable_error() {
    // assert_cmd closes stdin by default, so this run is non-interactive
    // even when the test suite itself runs in a terminal.
    Command::cargo_bin("rise")
        .expect("binary")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("missing required option `NAME`"));
}

#[test]
fn new_generates_backend_project_without_prompts() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "sample-app",
            "--backend",
            "axum",
            "--db",
            "sqlx",
            "--deployment",
            "docker",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("created ./sample-app"))
        .stdout(predicate::str::contains(".env.example"));

    let project = tmp.path().join("sample-app");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("name = \"sample-app\""));
    assert!(project.join("src/main.rs").exists());
    assert!(project.join("Dockerfile").exists());
    assert!(project.join(".git").exists(), "git init ran");

    // --db sqlx wires the full database stack.
    assert!(cargo_toml.contains("sqlx"));
    assert!(project.join("src/db.rs").exists());
    assert!(project.join(".env.example").exists());
    let gitignore = std::fs::read_to_string(project.join(".gitignore")).expect("read .gitignore");
    assert!(gitignore.contains(".env"));
}

#[test]
fn new_with_diesel_wires_a_connection_pool() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "diesel-app",
            "--backend",
            "axum",
            "--db",
            "diesel",
            "--deployment",
            "self-hosted",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let project = tmp.path().join("diesel-app");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("diesel"));
    assert!(cargo_toml.contains("r2d2"));
    assert!(!cargo_toml.contains("sqlx"));
    assert!(project.join("src/db.rs").exists(), "pool module");
    assert!(
        project.join(".env.example").exists(),
        "DATABASE_URL example"
    );
    let main_rs = std::fs::read_to_string(project.join("src/main.rs")).expect("read main.rs");
    assert!(main_rs.contains("db::create_pool()"));
}

#[test]
fn new_with_testing_adds_test_module_and_dev_dependencies() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "tested-app",
            "--backend",
            "axum",
            "--db",
            "sqlx",
            "--deployment",
            "docker",
            "--testing",
            "api,sqlx,mock",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let project = tmp.path().join("tested-app");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("[dev-dependencies]"));
    assert!(cargo_toml.contains("tower"));
    assert!(cargo_toml.contains("mockall"));
    let main_rs = std::fs::read_to_string(project.join("src/main.rs")).expect("read main.rs");
    assert!(main_rs.contains("mod tests"));
    assert!(main_rs.contains("mod db_tests"));
}

#[test]
fn new_with_websockets_adds_ws_endpoint() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "ws-app",
            "--backend",
            "axum",
            "--db",
            "sqlx",
            "--deployment",
            "docker",
            "--features",
            "web-sockets",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let project = tmp.path().join("ws-app");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("features = [\"ws\"]"));
    let main_rs = std::fs::read_to_string(project.join("src/main.rs")).expect("read main.rs");
    assert!(main_rs.contains("route(\"/ws\", get(ws_handler))"));
}

#[test]
fn new_with_sea_orm_wires_a_connection() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "seaorm-app",
            "--backend",
            "axum",
            "--db",
            "sea-orm",
            "--deployment",
            "docker",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let project = tmp.path().join("seaorm-app");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("sea-orm"));
    let db_rs = std::fs::read_to_string(project.join("src/db.rs")).expect("read db.rs");
    assert!(db_rs.contains("Database::connect"));
    let main_rs = std::fs::read_to_string(project.join("src/main.rs")).expect("read main.rs");
    assert!(main_rs.contains("db::connect().await"));
}

#[test]
fn new_with_sled_wires_an_embedded_store() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "sled-app",
            "--backend",
            "actix",
            "--db",
            "sled",
            "--deployment",
            "self-hosted",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let project = tmp.path().join("sled-app");
    let db_rs = std::fs::read_to_string(project.join("src/db.rs")).expect("read db.rs");
    assert!(db_rs.contains("sled::open"));
    let env_example =
        std::fs::read_to_string(project.join(".env.example")).expect("read .env.example");
    assert!(env_example.contains("DB_PATH"));
    let gitignore = std::fs::read_to_string(project.join(".gitignore")).expect("read .gitignore");
    assert!(gitignore.contains("/data"));
    let main_rs = std::fs::read_to_string(project.join("src/main.rs")).expect("read main.rs");
    assert!(main_rs.contains("db::open()"));
}

#[test]
fn new_with_rate_limiting_adds_governor() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "limited-app",
            "--backend",
            "axum",
            "--db",
            "sqlx",
            "--deployment",
            "docker",
            "--features",
            "rate-limiting",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let project = tmp.path().join("limited-app");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("tower_governor"));
    let main_rs = std::fs::read_to_string(project.join("src/main.rs")).expect("read main.rs");
    assert!(main_rs.contains("GovernorLayer::new(governor_conf)"));
    assert!(main_rs.contains("into_make_service_with_connect_info"));
}

#[test]
fn new_with_graphql_adds_schema_and_graphiql() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "gql-app",
            "--backend",
            "actix",
            "--db",
            "sqlx",
            "--deployment",
            "docker",
            "--features",
            "graph-ql",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let project = tmp.path().join("gql-app");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("async-graphql"));
    let main_rs = std::fs::read_to_string(project.join("src/main.rs")).expect("read main.rs");
    assert!(main_rs.contains("QueryRoot"));
    assert!(main_rs.contains("GraphiQLSource"));
    assert!(main_rs.contains("Data::new(schema.clone())"));
}

#[test]
fn new_with_grpc_generates_a_tonic_service() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "grpc-app",
            "--backend",
            "axum",
            "--db",
            "sqlx",
            "--deployment",
            "docker",
            "--features",
            "grpc",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("protoc"));

    let project = tmp.path().join("grpc-app");
    let proto = std::fs::read_to_string(project.join("proto/hello.proto")).expect("read proto");
    assert!(proto.contains("service Greeter"));
    let build_rs = std::fs::read_to_string(project.join("build.rs")).expect("read build.rs");
    assert!(build_rs.contains("tonic_prost_build::compile_protos"));
    let grpc_rs = std::fs::read_to_string(project.join("src/grpc.rs")).expect("read grpc.rs");
    assert!(grpc_rs.contains("GreeterImpl"));
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("tonic"));
    assert!(cargo_toml.contains("[build-dependencies]"));
    let dockerfile = std::fs::read_to_string(project.join("Dockerfile")).expect("read Dockerfile");
    assert!(dockerfile.contains("protobuf-compiler"));
    assert!(dockerfile.contains("EXPOSE 50051"));
}

#[test]
fn sqlx_testing_without_sqlx_database_is_rejected() {
    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "sample-app",
            "--backend",
            "axum",
            "--db",
            "diesel",
            "--testing",
            "sqlx",
            "--no-input",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "`--testing sqlx` requires `--db sqlx`",
        ));
}

#[test]
fn new_generates_actix_project_without_prompts() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "sample-actix",
            "--backend",
            "actix",
            "--db",
            "sqlx",
            "--deployment",
            "kubernetes",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("created ./sample-actix"));

    let project = tmp.path().join("sample-actix");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("actix-web"));
    let main_rs = std::fs::read_to_string(project.join("src/main.rs")).expect("read main.rs");
    assert!(main_rs.contains("Hello from sample-actix!"));
    assert!(
        !project.join("Dockerfile").exists(),
        "kubernetes excludes it"
    );
}

#[test]
fn new_generates_cli_project_without_prompts() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args(["new", "greeter", "--kind", "cli", "--no-input"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("created ./greeter"))
        .stdout(predicate::str::contains("cargo run"));

    let project = tmp.path().join("greeter");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("name = \"greeter\""));
    assert!(cargo_toml.contains("clap"));
    assert!(project.join("src/main.rs").exists());
    assert!(project.join(".git").exists(), "git init ran");
}

#[test]
fn new_generates_library_project_without_prompts() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args(["new", "mathlib", "--kind", "library", "--no-input"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("created ./mathlib"))
        .stdout(predicate::str::contains("cargo test"));

    let project = tmp.path().join("mathlib");
    let lib_rs = std::fs::read_to_string(project.join("src/lib.rs")).expect("read lib.rs");
    assert!(lib_rs.contains("mathlib"));
    assert!(lib_rs.contains("#[cfg(test)]"));
    assert!(!project.join("src/main.rs").exists());
}

#[test]
fn new_generates_yew_frontend_without_prompts() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "spa-app",
            "--frontend",
            "yew",
            "--deployment",
            "self-hosted",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("created ./spa-app"))
        .stdout(predicate::str::contains("trunk serve"))
        .stdout(predicate::str::contains("wasm32-unknown-unknown"));

    let project = tmp.path().join("spa-app");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("name = \"spa-app\""));
    assert!(cargo_toml.contains("yew"));
    assert!(!cargo_toml.contains("gloo-net"), "no backend to call");
    assert!(project.join("src/main.rs").exists());
    assert!(project.join("index.html").exists());
    assert!(
        !project.join("Dockerfile").exists(),
        "self-hosted excludes it"
    );
    assert!(
        !project.join("Trunk.toml").exists(),
        "no backend to proxy to"
    );
    assert!(project.join(".git").exists(), "git init ran");
}

#[test]
fn yew_frontend_with_docker_gets_a_static_serving_dockerfile() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "spa-app",
            "--frontend",
            "yew",
            "--deployment",
            "docker",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let dockerfile =
        std::fs::read_to_string(tmp.path().join("spa-app/Dockerfile")).expect("read Dockerfile");
    assert!(dockerfile.contains("trunk build --release"));
    assert!(dockerfile.contains("nginx"));
}

#[test]
fn new_generates_leptos_frontend_without_prompts() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "spa-app",
            "--frontend",
            "leptos",
            "--deployment",
            "self-hosted",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("created ./spa-app"))
        .stdout(predicate::str::contains("trunk serve"))
        .stdout(predicate::str::contains("wasm32-unknown-unknown"));

    let project = tmp.path().join("spa-app");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("name = \"spa-app\""));
    assert!(cargo_toml.contains("leptos"));
    assert!(cargo_toml.contains("csr"), "client-side rendered SPA");
    assert!(project.join("src/main.rs").exists());
    assert!(project.join("index.html").exists());
    assert!(
        !project.join("Dockerfile").exists(),
        "self-hosted excludes it"
    );
    assert!(project.join(".git").exists(), "git init ran");
}

#[test]
fn leptos_frontend_with_docker_gets_a_static_serving_dockerfile() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "spa-app",
            "--frontend",
            "leptos",
            "--deployment",
            "docker",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let project = tmp.path().join("spa-app");
    let dockerfile = std::fs::read_to_string(project.join("Dockerfile")).expect("read Dockerfile");
    assert!(dockerfile.contains("trunk build --release"));
    assert!(dockerfile.contains("nginx"));
    assert!(project.join(".dockerignore").exists());
}

#[test]
fn new_reports_the_missing_flag_when_non_interactive() {
    Command::cargo_bin("rise")
        .expect("binary")
        .args(["new", "sample-app", "--backend", "axum", "--no-input"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing required option `--db`"));
}

#[test]
fn new_generates_fullstack_project_without_prompts() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "full-app",
            "--backend",
            "axum",
            "--frontend",
            "askama",
            "--db",
            "sqlx",
            "--deployment",
            "docker",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("created ./full-app"))
        .stdout(predicate::str::contains(".env.example"));

    let project = tmp.path().join("full-app");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read Cargo.toml");
    assert!(cargo_toml.contains("axum"));
    assert!(cargo_toml.contains("askama"));
    assert!(cargo_toml.contains("sqlx"));
    assert!(project.join("templates/index.html").exists());
    let main_rs = std::fs::read_to_string(project.join("src/main.rs")).expect("read main.rs");
    assert!(main_rs.contains("IndexTemplate"));
}

#[test]
fn new_generates_yew_fullstack_workspace_without_prompts() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "wasm-app",
            "--backend",
            "axum",
            "--frontend",
            "yew",
            "--db",
            "sqlx",
            "--deployment",
            "docker",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("created ./wasm-app"))
        .stdout(predicate::str::contains("cargo run -p wasm-app-backend"))
        .stdout(predicate::str::contains("cd frontend && trunk serve"));

    let project = tmp.path().join("wasm-app");
    let root = std::fs::read_to_string(project.join("Cargo.toml")).expect("read workspace");
    assert!(root.contains("[workspace]"));
    assert!(root.contains("\"backend\""));
    assert!(root.contains("\"frontend\""));

    let backend =
        std::fs::read_to_string(project.join("backend/Cargo.toml")).expect("read backend");
    assert!(backend.contains("name = \"wasm-app-backend\""));
    assert!(backend.contains("tower-http"));
    let main_rs =
        std::fs::read_to_string(project.join("backend/src/main.rs")).expect("read main.rs");
    assert!(main_rs.contains("/api/hello"));
    assert!(main_rs.contains("ServeDir"));

    let frontend =
        std::fs::read_to_string(project.join("frontend/Cargo.toml")).expect("read frontend");
    assert!(frontend.contains("name = \"wasm-app-frontend\""));
    assert!(frontend.contains("yew"));
    assert!(frontend.contains("gloo-net"));
    let trunk_toml =
        std::fs::read_to_string(project.join("frontend/Trunk.toml")).expect("read Trunk.toml");
    assert!(trunk_toml.contains("http://localhost:3000/api/"));

    // The workspace root ships project-level files; members don't repeat them.
    assert!(project.join("Dockerfile").exists());
    assert!(project.join("README.md").exists());
    assert!(!project.join("backend/Dockerfile").exists());
    assert!(!project.join("frontend/Dockerfile").exists());
    assert!(project.join(".git").exists(), "git init ran");
}

#[test]
fn new_generates_actix_yew_workspace_without_prompts() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "actix-wasm",
            "--backend",
            "actix",
            "--frontend",
            "yew",
            "--db",
            "sled",
            "--deployment",
            "self-hosted",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let project = tmp.path().join("actix-wasm");
    let backend =
        std::fs::read_to_string(project.join("backend/Cargo.toml")).expect("read backend");
    assert!(backend.contains("actix-files"));
    let main_rs =
        std::fs::read_to_string(project.join("backend/src/main.rs")).expect("read main.rs");
    assert!(main_rs.contains("/api/hello"));
    assert!(main_rs.contains("Files::new"));
    assert!(
        !project.join("Dockerfile").exists(),
        "self-hosted excludes it"
    );
}

#[test]
fn fullstack_with_other_wasm_frontends_is_an_actionable_error() {
    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "seed-app",
            "--backend",
            "axum",
            "--frontend",
            "seed",
            "--db",
            "sqlx",
            "--deployment",
            "docker",
            "--no-input",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("web-fullstack-seed"));
}

#[test]
fn new_generates_leptos_axum_fullstack_without_prompts() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "leptos-app",
            "--backend",
            "axum",
            "--frontend",
            "leptos",
            "--db",
            "sqlx",
            "--deployment",
            "docker",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("created ./leptos-app"))
        .stdout(predicate::str::contains("cargo-leptos"))
        .stdout(predicate::str::contains("cargo leptos watch"))
        // Database wiring ships with the leptos templates, so the
        // .env hint is printed like for any other backend.
        .stdout(predicate::str::contains(".env"));

    let project = tmp.path().join("leptos-app");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read");
    assert!(cargo_toml.contains("name = \"leptos-app\""));
    assert!(cargo_toml.contains("leptos_axum"));
    assert!(cargo_toml.contains("[package.metadata.leptos]"));
    assert!(project.join("src/app.rs").exists());
    assert!(project.join("src/lib.rs").exists());
    assert!(project.join("style/main.css").exists());
    let dockerfile = std::fs::read_to_string(project.join("Dockerfile")).expect("read");
    assert!(dockerfile.contains("cargo-leptos"));
    assert!(project.join(".env.example").exists());
    assert!(project.join("src/db.rs").exists());
}

#[test]
fn new_generates_leptos_actix_fullstack_without_prompts() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "leptos-app",
            "--backend",
            "actix",
            "--frontend",
            "leptos",
            "--db",
            "sqlx",
            "--deployment",
            "self-hosted",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let project = tmp.path().join("leptos-app");
    let cargo_toml = std::fs::read_to_string(project.join("Cargo.toml")).expect("read");
    assert!(cargo_toml.contains("leptos_actix"));
    let main_rs = std::fs::read_to_string(project.join("src/main.rs")).expect("read");
    assert!(main_rs.contains("actix_web"));
    assert!(
        !project.join("Dockerfile").exists(),
        "self-hosted excludes it"
    );
}

#[test]
fn leptos_fullstack_wires_the_database() {
    let tmp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "leptos-app",
            "--backend",
            "axum",
            "--frontend",
            "leptos",
            "--db",
            "sled",
            "--deployment",
            "self-hosted",
            "--no-input",
        ])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("DB_PATH"));

    let project = tmp.path().join("leptos-app");
    let env_example =
        std::fs::read_to_string(project.join(".env.example")).expect("read .env.example");
    assert!(env_example.contains("DB_PATH"));
    let gitignore = std::fs::read_to_string(project.join(".gitignore")).expect("read .gitignore");
    assert!(gitignore.contains("/data"));
    let main_rs = std::fs::read_to_string(project.join("src/main.rs")).expect("read main.rs");
    assert!(main_rs.contains("db::open()"));
    let app_rs = std::fs::read_to_string(project.join("src/app.rs")).expect("read app.rs");
    assert!(app_rs.contains("use_context::<sled::Db>()"));
}

#[test]
fn leptos_rejects_backend_stack_options() {
    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "leptos-app",
            "--backend",
            "axum",
            "--frontend",
            "leptos",
            "--testing",
            "api",
            "--deployment",
            "docker",
            "--no-input",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "not supported with a leptos frontend",
        ));
}

#[test]
fn new_with_template_dir_uses_the_custom_template() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let template_dir = tmp.path().join("custom-template");
    std::fs::create_dir(&template_dir).expect("mkdir");
    std::fs::write(
        template_dir.join("template.toml"),
        "name = \"custom\"\ndescription = \"test template\"",
    )
    .expect("write manifest");
    std::fs::write(template_dir.join("NOTE.md.j2"), "project: {{ name }}").expect("write file");

    let work = tmp.path().join("work");
    std::fs::create_dir(&work).expect("mkdir");
    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "custom-app",
            "--kind",
            "cli",
            "--template",
            template_dir.to_str().expect("utf8 path"),
            "--no-input",
        ])
        .current_dir(&work)
        .assert()
        .success()
        .stdout(predicate::str::contains("created ./custom-app"));

    let note =
        std::fs::read_to_string(work.join("custom-app/NOTE.md")).expect("read rendered file");
    assert_eq!(note, "project: custom-app");
}

#[test]
fn new_with_missing_template_dir_names_the_path() {
    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "custom-app",
            "--kind",
            "cli",
            "--template",
            "/nonexistent/rise-template",
            "--no-input",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "cannot read template directory `/nonexistent/rise-template`",
        ));
}

#[test]
fn completions_register_for_the_binary_name() {
    // The package is `rise-cli`, but the installed binary is `rise`;
    // completion scripts must register for the binary name.
    Command::cargo_bin("rise")
        .expect("binary")
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete -F _rise"))
        .stdout(predicate::str::contains("rise-cli").not());
}

#[test]
fn fish_completions_cover_the_shell_argument() {
    // clap_complete's fish generator skips positional arguments; we append
    // the shell names for `rise completions <shell>` ourselves.
    Command::cargo_bin("rise")
        .expect("binary")
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "complete -c rise -n \"__fish_rise_using_subcommand completions\" -f -a \"bash elvish fish powershell zsh\"",
        ));
}

#[test]
fn failed_generation_removes_the_half_written_project() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let template_dir = tmp.path().join("broken-template");
    std::fs::create_dir(&template_dir).expect("mkdir");
    std::fs::write(
        template_dir.join("template.toml"),
        "name = \"broken\"\ndescription = \"fails mid-render\"",
    )
    .expect("write manifest");
    // Disk templates render in sorted path order: this file lands first...
    std::fs::write(template_dir.join("01-note.txt.j2"), "ok").expect("write file");
    // ...and this one fails under strict undefined behavior.
    std::fs::write(template_dir.join("zz-boom.txt.j2"), "{{ nope }}").expect("write file");

    let work = tmp.path().join("work");
    std::fs::create_dir(&work).expect("mkdir");
    Command::cargo_bin("rise")
        .expect("binary")
        .args([
            "new",
            "doomed-app",
            "--kind",
            "cli",
            "--template",
            template_dir.to_str().expect("utf8 path"),
            "--no-input",
        ])
        .current_dir(&work)
        .assert()
        .failure()
        .stderr(predicate::str::contains("render failed"));

    assert!(
        !work.join("doomed-app").exists(),
        "no half-written project left behind"
    );
}

#[test]
fn new_into_an_existing_empty_directory_works() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let work = tmp.path().join("work");
    std::fs::create_dir(&work).expect("mkdir");
    std::fs::create_dir(work.join("empty-app")).expect("mkdir empty target");

    Command::cargo_bin("rise")
        .expect("binary")
        .args(["new", "empty-app", "--kind", "cli", "--no-input"])
        .current_dir(&work)
        .assert()
        .success();

    assert!(work.join("empty-app/src/main.rs").exists());
}

#[test]
fn conflicting_flags_are_rejected_before_prompts() {
    Command::cargo_bin("rise")
        .expect("binary")
        .args(["new", "sample-app", "--kind", "cli", "--backend", "axum"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("conflicting options"));
}
