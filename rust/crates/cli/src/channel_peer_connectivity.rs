use crate::{CliError, channel_peer::GatewayPeerState, time::local_timestamp_now};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};
use toml::Value as TomlValue;

const QQBOT_TOKEN_URL: &str = "https://bots.qq.com/app/getAppAccessToken";
const QQBOT_CONNECTIVITY_TIMEOUT_SECS: u64 = 12;
const QQBOT_USER_AGENT: &str = "fin-qqbot-connectivity-probe/0.1";

#[derive(Debug, Clone)]
pub(crate) struct ConnectivitySuccess {
    pub(crate) credential_source: String,
    pub(crate) app_id_masked: String,
    pub(crate) expires_at: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ConnectivityFailure {
    pub(crate) connectivity_state: &'static str,
    pub(crate) credential_source: Option<String>,
    pub(crate) error: String,
}

#[derive(Debug, Clone)]
pub(crate) struct QqbotCredentials {
    pub(crate) app_id: String,
    pub(crate) client_secret: String,
    pub(crate) source: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    expires_in: Option<Value>,
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
}

pub(crate) fn probe_qqbot_upstream(
    user_toml_path: Option<&Path>,
) -> Result<ConnectivitySuccess, ConnectivityFailure> {
    let credentials =
        resolve_qqbot_credentials(user_toml_path).ok_or_else(|| ConnectivityFailure {
            connectivity_state: "auth_required",
            credential_source: None,
            error: "missing qqbot credentials".into(),
        })?;
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(QQBOT_CONNECTIVITY_TIMEOUT_SECS))
        .timeout(Duration::from_secs(QQBOT_CONNECTIVITY_TIMEOUT_SECS))
        .build()
        .map_err(|err| ConnectivityFailure {
            connectivity_state: "auth_failed",
            credential_source: Some(credentials.source.clone()),
            error: format!("failed to build http client: {err}"),
        })?;
    let response = client
        .post(token_endpoint())
        .header("Content-Type", "application/json")
        .header("User-Agent", QQBOT_USER_AGENT)
        .body(
            json!({
                "appId": credentials.app_id,
                "clientSecret": credentials.client_secret,
            })
            .to_string(),
        )
        .send()
        .map_err(|err| ConnectivityFailure {
            connectivity_state: "unreachable",
            credential_source: Some(credentials.source.clone()),
            error: format!("network error probing qqbot upstream: {err}"),
        })?;
    let status = response.status();
    let body = response.text().map_err(|err| ConnectivityFailure {
        connectivity_state: "auth_failed",
        credential_source: Some(credentials.source.clone()),
        error: format!("failed reading qqbot upstream response: {err}"),
    })?;
    let parsed: TokenResponse = serde_json::from_str(&body).map_err(|err| ConnectivityFailure {
        connectivity_state: "auth_failed",
        credential_source: Some(credentials.source.clone()),
        error: format!(
            "invalid qqbot upstream json (status={}): {}",
            status.as_u16(),
            err
        ),
    })?;
    if !status.is_success()
        || parsed
            .access_token
            .as_deref()
            .unwrap_or_default()
            .is_empty()
    {
        let detail = parsed
            .extra
            .get("message")
            .or_else(|| parsed.extra.get("msg"))
            .or_else(|| parsed.extra.get("error"))
            .map(|value| shorten(value.to_string(), 240))
            .unwrap_or_else(|| shorten(body, 240));
        return Err(ConnectivityFailure {
            connectivity_state: "auth_failed",
            credential_source: Some(credentials.source.clone()),
            error: format!(
                "qqbot upstream rejected credentials (status={}): {}",
                status.as_u16(),
                detail
            ),
        });
    }
    let expires_at = parsed
        .expires_in
        .and_then(|value| parse_u64_field(&value))
        .map(|seconds| add_seconds(local_timestamp_now().as_str(), seconds))
        .transpose()
        .map_err(|err| ConnectivityFailure {
            connectivity_state: "auth_failed",
            credential_source: Some(credentials.source.clone()),
            error: err.to_string(),
        })?;
    Ok(ConnectivitySuccess {
        credential_source: credentials.source,
        app_id_masked: mask_app_id(&credentials.app_id),
        expires_at,
    })
}

pub(crate) fn apply_probe_success(
    state: &mut GatewayPeerState,
    result: ConnectivitySuccess,
) -> Value {
    let now = local_timestamp_now();
    state.connectivity_state = "connected".into();
    state.updated_at = now.clone();
    state.connectivity_checked_at = Some(now.clone());
    state.upstream_authenticated_at = Some(now);
    state.upstream_expires_at = result.expires_at.clone();
    state.credential_source = Some(result.credential_source.clone());
    state.last_connectivity_error = None;
    json!({
        "runtime_state": state.runtime_state.clone(),
        "connectivity_state": state.connectivity_state.clone(),
        "binding_state": state.binding_state.clone(),
        "credential_source": result.credential_source,
        "app_id_masked": result.app_id_masked,
        "upstream_expires_at": result.expires_at,
    })
}

pub(crate) fn apply_probe_failure(
    state: &mut GatewayPeerState,
    failure: ConnectivityFailure,
) -> Value {
    let now = local_timestamp_now();
    state.connectivity_state = failure.connectivity_state.into();
    state.updated_at = now.clone();
    state.connectivity_checked_at = Some(now);
    state.credential_source = failure.credential_source.clone();
    state.last_connectivity_error = Some(failure.error.clone());
    state.upstream_authenticated_at = None;
    state.upstream_expires_at = None;
    json!({
        "runtime_state": state.runtime_state.clone(),
        "connectivity_state": state.connectivity_state.clone(),
        "binding_state": state.binding_state.clone(),
        "credential_source": failure.credential_source,
        "error": failure.error,
    })
}

pub(crate) fn resolve_qqbot_credentials(user_toml_path: Option<&Path>) -> Option<QqbotCredentials> {
    resolve_from_env().or_else(|| resolve_from_user_toml(user_toml_path))
}

fn resolve_from_env() -> Option<QqbotCredentials> {
    let app_id = env::var("FIN_QQBOT_APP_ID")
        .ok()
        .or_else(|| env::var("QQBOT_APP_ID").ok())?;
    let client_secret = env::var("FIN_QQBOT_CLIENT_SECRET")
        .ok()
        .or_else(|| env::var("QQBOT_CLIENT_SECRET").ok())?;
    Some(QqbotCredentials {
        app_id: app_id.trim().to_string(),
        client_secret: client_secret.trim().to_string(),
        source: "env".into(),
    })
}

fn resolve_from_user_toml(user_toml_path: Option<&Path>) -> Option<QqbotCredentials> {
    let path = explicit_user_toml_path(user_toml_path)?;
    let content = fs::read_to_string(&path).ok()?;
    let parsed: TomlValue = toml::from_str(&content).ok()?;
    let qqbot = parsed.get("channels")?.get("qqbot")?;
    if matches!(
        qqbot.get("enabled").and_then(TomlValue::as_bool),
        Some(false)
    ) {
        return None;
    }

    let app_id = string_field(qqbot, &["app_id", "appId"])
        .or_else(|| env_field(qqbot, &["app_id_env", "appIdEnv"]))?;
    let client_secret = string_field(qqbot, &["client_secret", "clientSecret"])
        .or_else(|| env_field(qqbot, &["client_secret_env", "clientSecretEnv"]))?;

    if app_id.is_empty() || client_secret.is_empty() {
        return None;
    }

    Some(QqbotCredentials {
        app_id,
        client_secret,
        source: format!("user_toml:{}", path.display()),
    })
}

fn explicit_user_toml_path(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = explicit.filter(|path| path.exists()) {
        return Some(path.to_path_buf());
    }
    if let Ok(path) = env::var("FIN_QQBOT_USER_TOML") {
        let path = PathBuf::from(path.trim());
        if path.exists() {
            return Some(path);
        }
    }
    let default = expand_home("~/.fin/config/user.toml");
    default.exists().then_some(default)
}

fn string_field(root: &TomlValue, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| root.get(*key).and_then(TomlValue::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn env_field(root: &TomlValue, keys: &[&str]) -> Option<String> {
    let env_name = string_field(root, keys)?;
    env::var(env_name.trim())
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_u64_field(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.trim().parse::<u64>().ok(),
        _ => None,
    }
}

fn token_endpoint() -> String {
    env::var("FIN_QQBOT_TOKEN_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| QQBOT_TOKEN_URL.into())
}

fn expand_home(input: &str) -> PathBuf {
    if let Some(rest) = input.strip_prefix("~/") {
        if let Ok(home) = env::var("HOME") {
            return Path::new(&home).join(rest);
        }
    }
    PathBuf::from(input)
}

fn mask_app_id(app_id: &str) -> String {
    if app_id.len() <= 4 {
        return format!("****(len={})", app_id.len());
    }
    format!("{}…(len={})", &app_id[..4], app_id.len())
}

fn shorten(input: String, max_chars: usize) -> String {
    let trimmed = input.trim().replace('\n', " ");
    let mut chars = trimmed.chars();
    let shortened = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{shortened}…")
    } else {
        shortened
    }
}

fn add_seconds(base: &str, seconds: u64) -> Result<String, CliError> {
    let base = chrono::DateTime::parse_from_rfc3339(base).map_err(|err| {
        CliError::ChannelConnectivity(format!("invalid timestamp '{base}': {err}"))
    })?;
    Ok((base + chrono::Duration::seconds(seconds as i64)).to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

    fn temp_file(name: &str) -> PathBuf {
        let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "fin-qqbot-config-{}-{}-{}",
            name,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos(),
            seq,
        ))
    }

    #[test]
    fn resolves_credentials_from_user_toml() {
        unsafe { std::env::remove_var("FIN_QQBOT_APP_ID") };
        unsafe { std::env::remove_var("FIN_QQBOT_CLIENT_SECRET") };
        unsafe { std::env::remove_var("QQBOT_APP_ID") };
        unsafe { std::env::remove_var("QQBOT_CLIENT_SECRET") };
        let path = temp_file("direct.toml");
        fs::write(
            &path,
            r#"
[channels.qqbot]
app_id = "1903323793"
client_secret = "secret-1"
"#,
        )
        .expect("write toml");
        let creds = resolve_qqbot_credentials(Some(&path)).expect("credentials");
        assert_eq!(creds.app_id, "1903323793");
        assert_eq!(creds.client_secret, "secret-1");
        assert!(creds.source.starts_with("user_toml:"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn resolves_credentials_from_user_toml_env_reference() {
        unsafe { std::env::remove_var("FIN_QQBOT_APP_ID") };
        unsafe { std::env::remove_var("FIN_QQBOT_CLIENT_SECRET") };
        unsafe { std::env::remove_var("QQBOT_APP_ID") };
        unsafe { std::env::remove_var("QQBOT_CLIENT_SECRET") };
        unsafe { std::env::set_var("QQBOT_SECRET_FOR_TEST", "secret-2") };
        let path = temp_file("env.toml");
        fs::write(
            &path,
            r#"
[channels.qqbot]
app_id = "1903323793"
client_secret_env = "QQBOT_SECRET_FOR_TEST"
"#,
        )
        .expect("write toml");
        let creds = resolve_qqbot_credentials(Some(&path)).expect("credentials");
        assert_eq!(creds.client_secret, "secret-2");
        let _ = fs::remove_file(path);
        unsafe { std::env::remove_var("QQBOT_SECRET_FOR_TEST") };
    }
}
