use crate::CliError;
use fin_config::{ProviderCredential, SystemConfig};
use std::{
    collections::hash_map::DefaultHasher,
    ffi::OsString,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LaunchdAgentInfo {
    pub(crate) label: String,
    pub(crate) plist_path: PathBuf,
}

pub(crate) fn should_use_launchd(override_path: Option<&Path>) -> bool {
    cfg!(target_os = "macos") && override_path.is_none()
}

pub(crate) fn ensure_launch_agent(
    runtime_home: &Path,
    system: &SystemConfig,
) -> Result<Option<LaunchdAgentInfo>, CliError> {
    if !cfg!(target_os = "macos") {
        return Ok(None);
    }
    let info = launch_agent_info(runtime_home)?;
    let current_exe = std::env::current_exe().map_err(|source| CliError::ReadFile {
        path: "current_exe".into(),
        source,
    })?;
    let payload = render_launch_agent_plist(
        info.label.as_str(),
        current_exe.as_path(),
        runtime_home.join("config/user.toml").as_path(),
        runtime_home,
        runtime_home
            .join("logs/runtime/headless-daemon.log")
            .as_path(),
        launch_agent_environment(system),
    );
    if let Some(parent) = info.plist_path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&info.plist_path, payload).map_err(|source| CliError::WriteFile {
        path: info.plist_path.display().to_string(),
        source,
    })?;
    Ok(Some(info))
}

pub(crate) fn bootstrap_launch_agent(info: &LaunchdAgentInfo) -> Result<(), CliError> {
    if !cfg!(target_os = "macos") {
        return Ok(());
    }
    let domain = launchd_domain()?;
    let target = format!("{domain}/{}", info.label);
    let bootstrap_status = ProcessCommand::new("launchctl")
        .args(["bootstrap", &domain, &info.plist_path.display().to_string()])
        .status()
        .map_err(|source| CliError::ReadFile {
            path: "launchctl bootstrap".into(),
            source,
        })?;
    if !bootstrap_status.success() {
        let _ = ProcessCommand::new("launchctl")
            .args(["bootout", &target])
            .status();
        let retry = ProcessCommand::new("launchctl")
            .args(["bootstrap", &domain, &info.plist_path.display().to_string()])
            .status()
            .map_err(|source| CliError::ReadFile {
                path: "launchctl bootstrap retry".into(),
                source,
            })?;
        if !retry.success() {
            return Err(CliError::ProcessFailed {
                command: format!("launchctl bootstrap {domain} {}", info.plist_path.display()),
                exit_code: retry.code(),
            });
        }
    }
    let kickstart = ProcessCommand::new("launchctl")
        .args(["kickstart", "-k", &target])
        .status()
        .map_err(|source| CliError::ReadFile {
            path: "launchctl kickstart".into(),
            source,
        })?;
    if !kickstart.success() {
        return Err(CliError::ProcessFailed {
            command: format!("launchctl kickstart -k {target}"),
            exit_code: kickstart.code(),
        });
    }
    Ok(())
}

pub(crate) fn bootout_launch_agent(runtime_home: &Path) -> Result<bool, CliError> {
    if !cfg!(target_os = "macos") {
        return Ok(false);
    }
    let info = launch_agent_info(runtime_home)?;
    if !info.plist_path.exists() {
        return Ok(false);
    }
    let target = format!("{}/{}", launchd_domain()?, info.label);
    let status = ProcessCommand::new("launchctl")
        .args(["bootout", &target])
        .status()
        .map_err(|source| CliError::ReadFile {
            path: "launchctl bootout".into(),
            source,
        })?;
    if status.success() {
        return Ok(true);
    }
    Ok(false)
}

fn launchd_domain() -> Result<String, CliError> {
    let output = ProcessCommand::new("id")
        .arg("-u")
        .output()
        .map_err(|source| CliError::ReadFile {
            path: "id -u".into(),
            source,
        })?;
    if !output.status.success() {
        return Err(CliError::ProcessFailed {
            command: "id -u".into(),
            exit_code: output.status.code(),
        });
    }
    let uid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(format!("gui/{uid}"))
}

fn launch_agent_info(runtime_home: &Path) -> Result<LaunchdAgentInfo, CliError> {
    let home = std::env::var("HOME").map_err(|_| CliError::Usage)?;
    let label = launch_agent_label(runtime_home);
    Ok(LaunchdAgentInfo {
        plist_path: Path::new(&home)
            .join("Library/LaunchAgents")
            .join(format!("{label}.plist")),
        label,
    })
}

fn launch_agent_label(runtime_home: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    runtime_home.display().to_string().hash(&mut hasher);
    format!("com.fin.headless-daemon.{:016x}", hasher.finish())
}

fn render_launch_agent_plist(
    label: &str,
    current_exe: &Path,
    user_toml_path: &Path,
    runtime_home: &Path,
    log_path: &Path,
    environment: Vec<(String, String)>,
) -> String {
    let environment_block = if environment.is_empty() {
        String::new()
    } else {
        let entries = environment
            .into_iter()
            .map(|(key, value)| {
                format!(
                    "    <key>{}</key>\n    <string>{}</string>",
                    xml_escape(&key),
                    xml_escape(&value)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("  <key>EnvironmentVariables</key>\n  <dict>\n{entries}\n  </dict>\n")
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>daemon-run</string>
    <string>{user_toml}</string>
  </array>
  <key>WorkingDirectory</key>
  <string>{runtime_home}</string>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>{log_path}</string>
  <key>StandardErrorPath</key>
  <string>{log_path}</string>
{environment_block}  <key>ProcessType</key>
  <string>Background</string>
</dict>
</plist>
"#,
        label = xml_escape(label),
        exe = xml_escape(&current_exe.display().to_string()),
        user_toml = xml_escape(&user_toml_path.display().to_string()),
        runtime_home = xml_escape(&runtime_home.display().to_string()),
        log_path = xml_escape(&log_path.display().to_string()),
        environment_block = environment_block,
    )
}

fn launch_agent_environment(system: &SystemConfig) -> Vec<(String, String)> {
    let mut environment = Vec::new();
    for key in ["PATH", "HOME"] {
        if let Ok(value) = std::env::var(key) {
            if let Some(value) = sanitize_launchd_environment_value(key, value) {
                environment.push((key.to_string(), value));
            }
        }
    }
    for provider in system.providers.values() {
        if let ProviderCredential::ApiKeyEnv { env_var } = &provider.credential
            && let Ok(value) = std::env::var(env_var)
        {
            environment.push((env_var.clone(), value));
        }
    }
    environment.sort_by(|left, right| left.0.cmp(&right.0));
    environment.dedup_by(|left, right| left.0 == right.0);
    environment
}

fn sanitize_launchd_environment_value(key: &str, value: String) -> Option<String> {
    if key != "PATH" {
        return Some(value);
    }
    let mut cleaned = Vec::<PathBuf>::new();
    for entry in std::env::split_paths(&OsString::from(&value)) {
        let text = entry.to_string_lossy().trim().to_string();
        if text.is_empty()
            || !text.starts_with('/')
            || text.contains('\n')
            || text.contains("Unknown command")
            || text.contains("npm help")
        {
            continue;
        }
        let path = PathBuf::from(text);
        if !cleaned.iter().any(|existing| existing == &path) {
            cleaned.push(path);
        }
    }
    if cleaned.is_empty() {
        return None;
    }
    std::env::join_paths(cleaned)
        .ok()
        .map(|value| value.to_string_lossy().into_owned())
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_agent_plist_contains_runtime_paths() {
        let runtime_home = Path::new("/tmp/fin-runtime-home");
        let current_exe = Path::new("/tmp/bin/fin");
        let user_toml = runtime_home.join("config/user.toml");
        let log_path = runtime_home.join("logs/runtime/headless-daemon.log");
        let plist = render_launch_agent_plist(
            "com.fin.test",
            current_exe,
            user_toml.as_path(),
            runtime_home,
            log_path.as_path(),
            vec![
                ("PATH".into(), "/usr/bin:/bin".into()),
                ("ALI_CODINGPLAN_KEY".into(), "secret".into()),
            ],
        );
        assert!(plist.contains("<string>com.fin.test</string>"));
        assert!(plist.contains("<string>/tmp/bin/fin</string>"));
        assert!(plist.contains("<string>daemon-run</string>"));
        assert!(plist.contains("<string>/tmp/fin-runtime-home/config/user.toml</string>"));
        assert!(
            plist.contains(
                "<string>/tmp/fin-runtime-home/logs/runtime/headless-daemon.log</string>"
            )
        );
        assert!(plist.contains("<key>ALI_CODINGPLAN_KEY</key>"));
        assert!(plist.contains("<string>secret</string>"));
        assert!(plist.contains("<true/>"));
    }

    #[test]
    fn sanitize_launchd_path_drops_shell_garbage_entries() {
        let path = "/usr/bin:/opt/homebrew/bin:Unknown command: bin To see a list of supported npm commands, run: npm help:/Users/fanzhang/.cargo/bin:bad\nline".to_string();
        let sanitized = sanitize_launchd_environment_value("PATH", path).expect("sanitized path");
        assert!(sanitized.contains("/usr/bin"));
        assert!(sanitized.contains("/opt/homebrew/bin"));
        assert!(sanitized.contains("/Users/fanzhang/.cargo/bin"));
        assert!(!sanitized.contains("Unknown command"));
        assert!(!sanitized.contains("npm help"));
        assert!(!sanitized.contains("bad\nline"));
    }
}
