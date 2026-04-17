use crate::{
    CliError,
    fs_utils::write_file,
    install_smoke::{run_installed_smoke, run_post_install_smoke},
    process_utils::{
        append_log, file_checksum_hex, now_unix_seconds, repo_root, run_process_and_log,
        short_git_sha,
    },
    runtime_home::{ensure_runtime_home_layout, init_runtime_home, resolved_runtime_home},
    versioning::{parse_build_version, resolve_build_version},
};
use fin_config::SystemConfig;
use serde_json::json;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
};

#[cfg(unix)]
use std::os::unix::fs as unix_fs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstallArtifacts {
    pub(crate) build_version: String,
    pub(crate) runtime_home: PathBuf,
    pub(crate) staged_dir: PathBuf,
    pub(crate) version_dir: PathBuf,
    pub(crate) bin_link: PathBuf,
}

pub(crate) fn build_dev(
    user_toml: &str,
    system: &SystemConfig,
    build_version_override: Option<String>,
    runtime_home_override: Option<&Path>,
    source_executable_override: Option<&Path>,
    smoke_home_override: Option<&Path>,
) -> Result<InstallArtifacts, CliError> {
    let runtime_home = init_runtime_home(user_toml, system, runtime_home_override)?;
    let build_version = resolve_build_version(&runtime_home, build_version_override)?;
    let install_log = runtime_home
        .join("logs/install")
        .join(format!("{build_version}.log"));
    let regression_log = runtime_home
        .join("logs/regression")
        .join(format!("{build_version}.log"));
    let smoke_config_path = runtime_home.join("config/user.toml");
    let smoke_home = resolve_smoke_home(&runtime_home, smoke_home_override);
    let source_executable =
        source_executable_override
            .map(Path::to_path_buf)
            .unwrap_or(env::current_exe().map_err(|source| CliError::ReadFile {
                path: "current executable".into(),
                source,
            })?);

    run_source_validation(&repo_root()?, &install_log)?;
    let staged_dir = stage_build(
        &runtime_home,
        &build_version,
        source_executable,
        &install_log,
    )?;
    run_installed_smoke(
        &staged_dir.join("bin/fin"),
        &smoke_config_path,
        &smoke_home,
        &runtime_home,
        &build_version,
        &regression_log,
    )?;
    let version_dir = promote_build(&runtime_home, &build_version, &install_log)?;
    run_post_install_smoke(
        &runtime_home,
        &smoke_config_path,
        &smoke_home,
        &build_version,
        &install_log,
    )?;

    Ok(InstallArtifacts {
        build_version,
        runtime_home: runtime_home.clone(),
        staged_dir,
        version_dir,
        bin_link: runtime_home.join("bin/fin"),
    })
}

pub(crate) fn promote_existing_build(
    user_toml: &str,
    system: &SystemConfig,
    build_version: &str,
    runtime_home_override: Option<&Path>,
    smoke_home_override: Option<&Path>,
) -> Result<PathBuf, CliError> {
    let runtime_home = init_runtime_home(user_toml, system, runtime_home_override)?;
    parse_build_version(build_version)?;
    let install_log = runtime_home
        .join("logs/install")
        .join(format!("{build_version}.log"));
    let smoke_home = resolve_smoke_home(&runtime_home, smoke_home_override);
    let smoke_config_path = runtime_home.join("config/user.toml");

    let version_dir = promote_build(&runtime_home, build_version, &install_log)?;
    run_post_install_smoke(
        &runtime_home,
        &smoke_config_path,
        &smoke_home,
        build_version,
        &install_log,
    )?;
    append_log(
        &install_log,
        &format!(
            "[promote-existing] build_version={build_version} version_dir={}\n",
            version_dir.display()
        ),
    )?;
    Ok(runtime_home)
}

pub(crate) fn rollback_install(
    system: &SystemConfig,
    runtime_home_override: Option<&Path>,
    smoke_home_override: Option<&Path>,
) -> Result<PathBuf, CliError> {
    let runtime_home = resolved_runtime_home(system, runtime_home_override);
    ensure_runtime_home_layout(&runtime_home)?;
    let current_link = runtime_home.join("install/current");
    let previous_link = runtime_home.join("install/previous");
    let current_target = fs::read_link(&current_link).map_err(|source| CliError::ReadFile {
        path: current_link.display().to_string(),
        source,
    })?;
    let previous_target = fs::read_link(&previous_link).map_err(|source| CliError::ReadFile {
        path: previous_link.display().to_string(),
        source,
    })?;
    let current_build = build_id_from_link(&current_target)?;
    let previous_build = build_id_from_link(&previous_target)?;
    let install_log = runtime_home
        .join("logs/install")
        .join(format!("rollback-{previous_build}.log"));

    replace_symlink(&current_link, &previous_target)?;
    replace_symlink(&previous_link, &current_target)?;
    replace_symlink(
        &runtime_home.join("bin/fin"),
        &runtime_home.join("install/current/bin/fin"),
    )?;
    write_receipt(
        &runtime_home,
        &format!("rollback-{previous_build}"),
        &json!({
            "action": "rollback",
            "rolled_back_from": current_build,
            "current_version": previous_build,
            "previous_version": current_build,
            "timestamp": now_unix_seconds(),
        }),
    )?;
    let smoke_home = resolve_smoke_home(&runtime_home, smoke_home_override);
    run_post_install_smoke(
        &runtime_home,
        &runtime_home.join("config/user.toml"),
        &smoke_home,
        &format!("rollback-{previous_build}"),
        &install_log,
    )?;
    Ok(runtime_home)
}

fn resolve_smoke_home(runtime_home: &Path, override_path: Option<&Path>) -> PathBuf {
    override_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| runtime_home.parent().unwrap_or(runtime_home).to_path_buf())
}

fn run_source_validation(repo_root: &Path, log_path: &Path) -> Result<(), CliError> {
    run_process_and_log(
        ProcessCommand::new(repo_root.join("scripts/verify-governance.sh")),
        "verify-governance",
        log_path,
    )?;
    run_process_and_log(
        ProcessCommand::new(repo_root.join("scripts/check-code-line-limit.py")),
        "check-code-line-limit",
        log_path,
    )?;
    let mut fmt = ProcessCommand::new("cargo");
    fmt.args(["fmt", "--check"])
        .current_dir(repo_root.join("rust"));
    run_process_and_log(fmt, "cargo fmt --check", log_path)?;
    let mut test = ProcessCommand::new("cargo");
    test.args(["test"]).current_dir(repo_root.join("rust"));
    run_process_and_log(test, "cargo test", log_path)
}

fn stage_build(
    runtime_home: &Path,
    build_version: &str,
    source_executable: PathBuf,
    log_path: &Path,
) -> Result<PathBuf, CliError> {
    let staged_dir = runtime_home.join("install/staged").join(build_version);
    if staged_dir.exists() {
        return Err(CliError::InvalidInstallState(format!(
            "staged build already exists: {}",
            staged_dir.display()
        )));
    }
    fs::create_dir_all(staged_dir.join("bin")).map_err(|source| CliError::WriteFile {
        path: staged_dir.join("bin").display().to_string(),
        source,
    })?;
    let staged_binary = staged_dir.join("bin/fin");
    fs::copy(&source_executable, &staged_binary).map_err(|source| CliError::WriteFile {
        path: staged_binary.display().to_string(),
        source,
    })?;

    let checksum = file_checksum_hex(&source_executable)?;
    let git_sha = short_git_sha().unwrap_or_else(|| "nogit".into());
    let manifest = format!(
        "build_version = \"{build_version}\"\ngit_sha = \"{git_sha}\"\nsource_executable = \"{}\"\ncreated_at = {}\n",
        source_executable.display(),
        now_unix_seconds()
    );
    write_file(&staged_dir.join("manifest.toml"), manifest.as_bytes())?;
    write_file(
        &staged_dir.join("checksums.txt"),
        format!("{checksum}  bin/fin\n").as_bytes(),
    )?;
    append_log(
        log_path,
        &format!(
            "[stage] build_version={build_version} source={} staged={}\n",
            source_executable.display(),
            staged_binary.display()
        ),
    )?;
    Ok(staged_dir)
}

fn promote_build(
    runtime_home: &Path,
    build_version: &str,
    log_path: &Path,
) -> Result<PathBuf, CliError> {
    let staged_dir = runtime_home.join("install/staged").join(build_version);
    if !staged_dir.exists() {
        return Err(CliError::MissingInstallTarget(
            staged_dir.display().to_string(),
        ));
    }
    let version_dir = runtime_home.join("install/versions").join(build_version);
    if version_dir.exists() {
        return Err(CliError::InvalidInstallState(format!(
            "version already exists: {}",
            version_dir.display()
        )));
    }
    fs::rename(&staged_dir, &version_dir).map_err(|source| CliError::WriteFile {
        path: format!("{} -> {}", staged_dir.display(), version_dir.display()),
        source,
    })?;

    let current_link = runtime_home.join("install/current");
    let previous_link = runtime_home.join("install/previous");
    let old_current = fs::read_link(&current_link).ok();
    if let Some(old) = &old_current {
        replace_symlink(&previous_link, old)?;
    }
    replace_symlink(&current_link, &version_dir)?;
    replace_symlink(
        &runtime_home.join("bin/fin"),
        &runtime_home.join("install/current/bin/fin"),
    )?;
    write_receipt(
        runtime_home,
        build_version,
        &json!({
            "action": "promote",
            "build_version": build_version,
            "current_version": build_version,
            "previous_version": old_current
                .as_ref()
                .and_then(|path| path.file_name())
                .and_then(|name| name.to_str()),
            "git_sha": short_git_sha().unwrap_or_else(|| "nogit".into()),
            "timestamp": now_unix_seconds(),
        }),
    )?;
    append_log(
        log_path,
        &format!(
            "[promote] build_version={build_version} current={} previous={}
",
            version_dir.display(),
            old_current
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "-".into())
        ),
    )?;
    Ok(version_dir)
}

fn write_receipt(
    runtime_home: &Path,
    name: &str,
    payload: &serde_json::Value,
) -> Result<(), CliError> {
    write_file(
        &runtime_home
            .join("install/receipts")
            .join(format!("{name}.json")),
        serde_json::to_vec_pretty(payload)?.as_slice(),
    )
}

fn build_id_from_link(link_target: &Path) -> Result<String, CliError> {
    link_target
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| CliError::InvalidInstallState(link_target.display().to_string()))
}

#[cfg(unix)]
fn replace_symlink(link_path: &Path, target: &Path) -> Result<(), CliError> {
    if link_path.exists() || fs::symlink_metadata(link_path).is_ok() {
        let meta = fs::symlink_metadata(link_path).map_err(|source| CliError::ReadFile {
            path: link_path.display().to_string(),
            source,
        })?;
        if meta.is_dir() && !meta.file_type().is_symlink() {
            fs::remove_dir_all(link_path).map_err(|source| CliError::WriteFile {
                path: link_path.display().to_string(),
                source,
            })?;
        } else {
            fs::remove_file(link_path).map_err(|source| CliError::WriteFile {
                path: link_path.display().to_string(),
                source,
            })?;
        }
    }
    if let Some(parent) = link_path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    unix_fs::symlink(target, link_path).map_err(|source| CliError::WriteFile {
        path: format!("{} -> {}", link_path.display(), target.display()),
        source,
    })
}
