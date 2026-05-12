use crate::{CliError, time::local_timestamp_now};
use chrono::{DateTime, FixedOffset, Local};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct QqbotProgressPolicy {
    #[serde(default = "default_enabled")]
    pub(crate) enabled: bool,
    #[serde(default = "default_detail_level")]
    pub(crate) detail_level: String,
    #[serde(default = "default_show_active_sources")]
    pub(crate) show_active_sources: bool,
    #[serde(default = "default_show_resource_summary")]
    pub(crate) show_resource_summary: bool,
    #[serde(default = "default_show_tool_summary")]
    pub(crate) show_tool_summary: bool,
    #[serde(default = "default_heartbeat_interval_secs")]
    pub(crate) heartbeat_interval_secs: u64,
    #[serde(default)]
    pub(crate) updated_at: Option<String>,
}

impl Default for QqbotProgressPolicy {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            detail_level: default_detail_level(),
            show_active_sources: default_show_active_sources(),
            show_resource_summary: default_show_resource_summary(),
            show_tool_summary: default_show_tool_summary(),
            heartbeat_interval_secs: default_heartbeat_interval_secs(),
            updated_at: None,
        }
    }
}

fn default_enabled() -> bool {
    true
}

fn default_detail_level() -> String {
    "detailed".into()
}

fn default_show_active_sources() -> bool {
    true
}

fn default_show_resource_summary() -> bool {
    true
}

fn default_show_tool_summary() -> bool {
    true
}

fn default_heartbeat_interval_secs() -> u64 {
    120
}

fn policy_path(runtime_home: &Path) -> std::path::PathBuf {
    runtime_home.join("runtime/channels/qqbot/progress_policy.json")
}

pub(crate) fn load_qqbot_progress_policy(
    runtime_home: &Path,
) -> Result<QqbotProgressPolicy, CliError> {
    let path = policy_path(runtime_home);
    match fs::read_to_string(&path) {
        Ok(content) => {
            serde_json::from_str::<QqbotProgressPolicy>(&content).map_err(CliError::Serialize)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Ok(QqbotProgressPolicy::default())
        }
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

pub(crate) fn save_qqbot_progress_policy(
    runtime_home: &Path,
    mut policy: QqbotProgressPolicy,
) -> Result<QqbotProgressPolicy, CliError> {
    let path = policy_path(runtime_home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    policy.updated_at = Some(local_timestamp_now());
    fs::write(
        &path,
        serde_json::to_vec_pretty(&policy).map_err(CliError::Serialize)?,
    )
    .map_err(|source| CliError::WriteFile {
        path: path.display().to_string(),
        source,
    })?;
    Ok(policy)
}

pub(crate) fn render_policy_summary(policy: &QqbotProgressPolicy) -> String {
    format!(
        "enabled={} detail={} active_sources={} resource_summary={} tool_summary={} heartbeat={}s",
        policy.enabled,
        policy.detail_level,
        policy.show_active_sources,
        policy.show_resource_summary,
        policy.show_tool_summary,
        policy.heartbeat_interval_secs
    )
}

pub(crate) fn heartbeat_due(last_delivery_at: Option<&str>, heartbeat_interval_secs: u64) -> bool {
    if heartbeat_interval_secs == 0 {
        return false;
    }
    let Some(last_delivery_at) = last_delivery_at else {
        return false;
    };
    let Some(last) = parse_local_ts(last_delivery_at) else {
        return false;
    };
    let now = Local::now().fixed_offset();
    (now - last).num_seconds() >= heartbeat_interval_secs as i64
}

fn parse_local_ts(ts: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(ts).ok()
}
