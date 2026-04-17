use crate::{
    CliError,
    fs_utils::write_file,
    process_utils::{append_log, now_unix_seconds, run_process_and_log},
    runtime_home::read_last_run_value,
};
use serde_json::json;
use std::{fs, path::Path, process::Command as ProcessCommand};

pub(crate) fn run_installed_smoke(
    binary_path: &Path,
    config_path: &Path,
    smoke_home: &Path,
    runtime_home: &Path,
    build_version: &str,
    log_path: &Path,
) -> Result<(), CliError> {
    let smoke_namespace = format!("test-install-{}", build_version.replace('.', "-"));
    let commands: [(&str, Vec<String>); 3] = [
        (
            "config-check",
            vec![
                binary_path.display().to_string(),
                "config-check".into(),
                config_path.display().to_string(),
            ],
        ),
        (
            "runtime-demo",
            vec![
                binary_path.display().to_string(),
                "runtime-demo".into(),
                config_path.display().to_string(),
                format!("installed smoke {build_version}"),
            ],
        ),
        (
            "debug-projection",
            vec![
                binary_path.display().to_string(),
                "debug-projection".into(),
                config_path.display().to_string(),
                format!("observable smoke {build_version}"),
            ],
        ),
    ];

    for (label, argv) in commands {
        run_smoke_command(
            label,
            argv,
            smoke_home,
            runtime_home,
            &smoke_namespace,
            log_path,
        )?;
    }

    verify_smoke_artifacts(runtime_home, build_version, binary_path, smoke_home)
}

pub(crate) fn run_post_install_smoke(
    runtime_home: &Path,
    config_path: &Path,
    smoke_home: &Path,
    build_version: &str,
    log_path: &Path,
) -> Result<(), CliError> {
    let bin_link = runtime_home.join("bin/fin");
    if !bin_link.exists() {
        return Err(CliError::MissingInstallTarget(
            bin_link.display().to_string(),
        ));
    }
    let mut command = ProcessCommand::new(&bin_link);
    command
        .arg("config-check")
        .arg(config_path)
        .env("HOME", smoke_home)
        .env("FIN_RUNTIME_HOME_OVERRIDE", runtime_home);
    run_process_and_log(command, "post-install config-check", log_path)?;
    write_file(
        &runtime_home.join("runtime/current/install_state.json"),
        serde_json::to_vec_pretty(&json!({
            "build_version": build_version,
            "bin": bin_link.display().to_string(),
            "timestamp": now_unix_seconds(),
        }))?
        .as_slice(),
    )
}

fn run_smoke_command(
    label: &str,
    argv: Vec<String>,
    smoke_home: &Path,
    runtime_home: &Path,
    smoke_namespace: &str,
    log_path: &Path,
) -> Result<(), CliError> {
    let attempts = if matches!(label, "runtime-demo" | "debug-projection") {
        3
    } else {
        1
    };
    let mut last_err = None;
    for attempt in 1..=attempts {
        let mut command = ProcessCommand::new(&argv[0]);
        command
            .args(&argv[1..])
            .env("HOME", smoke_home)
            .env("FIN_RUNTIME_HOME_OVERRIDE", runtime_home)
            .env("FIN_SESSION_NAMESPACE", smoke_namespace);
        match run_process_and_log(command, &format!("{label}#{attempt}"), log_path) {
            Ok(()) => return Ok(()),
            Err(err) if attempt < attempts => {
                append_log(
                    log_path,
                    &format!("[retry] label={label} attempt={attempt}/{attempts} reason={err}\n"),
                )?;
                last_err = Some(err);
            }
            Err(err) => {
                last_err = Some(err);
                break;
            }
        }
    }
    Err(last_err.expect("smoke failure should be recorded"))
}

fn verify_smoke_artifacts(
    runtime_home: &Path,
    build_version: &str,
    binary_path: &Path,
    smoke_home: &Path,
) -> Result<(), CliError> {
    for path in [
        runtime_home.join("runtime/current/last_run.json"),
        runtime_home.join("runtime/current/current_context.json"),
        runtime_home.join("runtime/projections/current_snapshot.json"),
        runtime_home.join("runtime/projections/current_projection.json"),
    ] {
        if !path.exists() {
            return Err(CliError::MissingInstallTarget(path.display().to_string()));
        }
    }

    let last_run = read_last_run_value(runtime_home)?;
    let session_recent_context_path = runtime_home.join(
        last_run["session_recent_contexts_path"]
            .as_str()
            .ok_or_else(|| CliError::MissingInstallTarget("session_recent_contexts_path".into()))?,
    );
    if !session_recent_context_path.exists() {
        return Err(CliError::MissingInstallTarget(
            session_recent_context_path.display().to_string(),
        ));
    }

    let report_dir = runtime_home.join("harness/reports").join(build_version);
    fs::create_dir_all(&report_dir).map_err(|source| CliError::WriteFile {
        path: report_dir.display().to_string(),
        source,
    })?;
    write_file(
        &report_dir.join("summary.json"),
        serde_json::to_vec_pretty(&json!({
            "build_version": build_version,
            "smoke_home": smoke_home.display().to_string(),
            "binary": binary_path.display().to_string(),
            "verified_paths": [
                "runtime/current/last_run.json",
                "runtime/current/current_context.json",
                "runtime/projections/current_snapshot.json",
                "runtime/projections/current_projection.json",
                last_run["session_recent_contexts_path"]
            ]
        }))?
        .as_slice(),
    )
}