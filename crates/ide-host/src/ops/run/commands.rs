#[allow(unused_imports)]
use super::*;
use crate::Host;
use crate::rpc::Dispatcher;
use ide_core::{IdeError, IdeResult};
use ide_run::RunConfig;

pub async fn run_dotnet_resolve(host: &Host, window: &str, config: RunConfig, debug: bool) -> IdeResult<RunConfig> {
    host.require_trust(window)?;
    if !matches!(config.kind.as_str(), "dotnet" | "dotnet-test")
        || config.extras.target.as_ref().is_some_and(|s| !s.is_empty())
    {
        return Err(IdeError::invalid(".NET Project requires the local machine"));
    }
    let project = host.project(window)?;
    let mut config = host.blocking(move || ide_run::dotnet::resolve(&project.root, config)).await?;
    if debug {
        let root = host.project(window)?.root.clone();
        let target = ide_run::dotnet::target_path(&root, &config).await?;
        config.extras.params.insert("targetPath".into(), target);
    }
    Ok(config)
}

fn application_cache(project: &crate::project::Project, id: &str) -> std::path::PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hash);
    project.cache_dir.join("application-update").join(format!("{:x}", hash.finish()))
}

pub async fn run_application_build(
    host: &Host,
    window: &str,
    config: RunConfig,
    module: std::path::PathBuf,
) -> IdeResult<RunConfig> {
    host.require_trust(window)?;
    if config.kind != "spring-boot" || config.extras.target.as_ref().is_some_and(|t| !t.is_empty()) {
        return Err(IdeError::invalid("Application resource updates require a local Spring Boot configuration"));
    }
    let project = host.project(window)?;
    tokio::task::spawn_blocking(move || {
        let model = crate::ops::build::load(&project.root, &project.build_cache_dir(), &project.settings.effective());
        let plan = ide_build::application_update::plan(
            &project.root,
            &model,
            &module,
            &application_cache(&project, &config.id),
        )?;
        let mut env = plan.env;
        env.extend(config.env);
        Ok(RunConfig {
            id: format!("application-build:{}", config.id),
            name: format!("Update {}", config.name),
            kind: plan.kind.into(),
            command: plan.command,
            args: plan.args,
            cwd: Some(project.root.join(plan.cwd).into()),
            env,
            detected: false,
            is_test: false,
            debug_adapter: None,
            allow_parallel: false,
            extras: ide_run::RunExtras {
                include_system_env: config.extras.include_system_env,
                env_files: config.extras.env_files,
                ..Default::default()
            },
        })
    })
    .await
    .map_err(|e| IdeError::internal(e.to_string()))?
}

pub async fn run_application_publish(
    host: &Host,
    window: &str,
    config: RunConfig,
    class_paths: Vec<String>,
    main_class: String,
) -> IdeResult<()> {
    host.require_trust(window)?;
    let project = host.project(window)?;
    tokio::task::spawn_blocking(move || {
        ide_build::application_update::publish(
            &project.root,
            &application_cache(&project, &config.id),
            &class_paths,
            &main_class,
        )
    })
    .await
    .map_err(|e| IdeError::internal(e.to_string()))?
}

/// Configuration templates: per-type defaults applied to new configurations.
pub fn run_templates(host: &Host, window: &str) -> IdeResult<Vec<RunConfig>> {
    let root = host.project(window)?.root.clone();
    ide_run::load_templates(&root)
}

pub fn run_save_templates(host: &Host, window: &str, templates: Vec<RunConfig>) -> IdeResult<()> {
    let root = host.project(window)?.root.clone();
    ide_run::save_templates(&root, &templates)
}

/// The environment runs inherit when "Include system environment variables" is on.
pub fn run_system_env(_host: &Host, _window: &str) -> std::collections::BTreeMap<String, String> {
    ide_run::system_env()
}

/// Results from one JUnit XML file (CTest's `--output-junit`) written since
/// `since_ms`; `path` is relative to the project root.
pub async fn run_test_report_file(
    host: &Host,
    window: &str,
    path: std::path::PathBuf,
    since_ms: f64,
) -> IdeResult<Vec<ide_run::junit::TestCaseReport>> {
    host.require_trust(window)?;
    let root = host.project(window)?.root.clone();
    let file = root.join(path);
    let since = std::time::UNIX_EPOCH + std::time::Duration::from_millis(since_ms.max(0.0) as u64);
    tokio::task::spawn_blocking(move || ide_run::junit::read_file(&file, since))
        .await
        .map_err(|e| IdeError::internal(e.to_string()))
}

/// Commands of this module the remote server answers.
pub(crate) fn register(d: &mut Dispatcher) {
    d.add("run_dotnet_resolve", |c| async move {
        run_dotnet_resolve(&c.host, &c.window, c.arg("config")?, c.arg("debug")?).await
    });
    d.add("run_application_build", |c| async move {
        run_application_build(&c.host, &c.window, c.arg("config")?, c.arg("module")?).await
    });
    d.add("run_application_publish", |c| async move {
        run_application_publish(&c.host, &c.window, c.arg("config")?, c.arg("classPaths")?, c.arg("mainClass")?).await
    });
    d.add("run_templates", |c| async move { run_templates(&c.host, &c.window) });
    d.add("run_save_templates", |c| async move { run_save_templates(&c.host, &c.window, c.arg("templates")?) });
    d.add("run_system_env", |c| async move { Ok(run_system_env(&c.host, &c.window)) });
    d.add("run_test_report_file", |c| async move {
        run_test_report_file(&c.host, &c.window, c.arg("path")?, c.arg("sinceMs")?).await
    });
}
