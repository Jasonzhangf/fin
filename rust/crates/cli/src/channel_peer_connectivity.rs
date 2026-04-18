use crate::{CliError, channel_peer::GatewayPeerState, time::local_timestamp_now};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{env, fs, path::Path, time::Duration};

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
struct QqbotCredentials {
    app_id: String,
    client_secret: String,
    source: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    expires_in: Option<u64>,
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
}

pub(crate) fn probe_qqbot_upstream() -> Result<ConnectivitySuccess, ConnectivityFailure> {
    let credentials = resolve_qqbot_credentials().ok_or_else(|| ConnectivityFailure {
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
    let body = response
        .text()
        .map_err(|err| ConnectivityFailure {
            connectivity_state: "auth_failed",
            credential_source: Some(credentials.source.clone()),
            error: format!("failed reading qqbot upstream response: {err}"),
        })?;
    let parsed: TokenResponse = serde_json::from_str(&body).map_err(|err| ConnectivityFailure {
        connectivity_state: "auth_failed",
        credential_source: Some(credentials.source.clone()),
        error: format!("invalid qqbot upstream json (status={}): {}", status.as_u16(), err),
    })?;
    if !status.is_success() || parsed.access_token.as_deref().unwrap_or_default().is_empty() {
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
            error: format!("qqbot upstream rejected credentials (status={}): {}", status.as_u16(), detail),
        });
    }
    let expires_at = parsed
        .expires_in
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

fn resolve_qqbot_credentials() -> Option<QqbotCredentials> {
    resolve_from_env()
        .or_else(resolve_from_finger_plugin_config)
        .or_else(resolve_from_finger_channels_config)
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

fn resolve_from_finger_plugin_config() -> Option<QqbotCredentials> {
    let path = expand_home("~/.finger/runtime/plugins/openclaw-qqbot.json");
    let content = fs::read_to_string(path).ok()?;
    let parsed: Value = serde_json::from_str(&content).ok()?;
    let config = parsed.get("config")?.as_object()?;
    let app_id = config.get("appId")?.as_str()?.trim().to_string();
    let client_secret = config.get("clientSecret")?.as_str()?.trim().to_string();
    if app_id.is_empty() || client_secret.is_empty() {
        return None;
    }
    Some(QqbotCredentials {
        app_id,
        client_secret,
        source: "legacy_finger_plugin".into(),
    })
}

fn resolve_from_finger_channels_config() -> Option<QqbotCredentials> {
    let path = expand_home("~/.finger/config/channels.json");
    let content = fs::read_to_string(path).ok()?;
    let parsed: Value = serde_json::from_str(&content).ok()?;
    let channels = parsed.get("channels")?.as_array()?;
    let entry = channels.iter().find(|channel| {
        matches!(channel.get("id").and_then(Value::as_str), Some("qqbot"))
            || matches!(channel.get("channelId").and_then(Value::as_str), Some("qqbot"))
    })?;
    let credentials = entry.get("credentials")?.as_object()?;
    let app_id = credentials.get("appId")?.as_str()?.trim().to_string();
    let client_secret = credentials.get("clientSecret")?.as_str()?.trim().to_string();
    if app_id.is_empty() || client_secret.is_empty() {
        return None;
    }
    Some(QqbotCredentials {
        app_id,
        client_secret,
        source: "legacy_finger_channels".into(),
    })
}

fn token_endpoint() -> String {
    env::var("FIN_QQBOT_TOKEN_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| QQBOT_TOKEN_URL.into())
}

fn expand_home(input: &str) -> String {
    if let Some(rest) = input.strip_prefix("~/") {
        return env::var("HOME")
            .map(|home| Path::new(&home).join(rest).display().to_string())
            .unwrap_or_else(|_| input.to_string());
    }
    input.to_string()
}

fn mask_app_id(app_id: &str) -> String {
    if app_id.len() <= 4 {
        return format!("****(len={})", app_id.len());
    }
    format!("{}…(len={})", &app_id[..4], app_id.len())
}

fn shorten(input: String, max_chars: usize) -> String {
    let trimmed = input.trim().replace('\n', " ");
    if trimmed.chars().count() <= max_chars {
        return trimmed;
    }
    trimmed.chars().take(max_chars).collect::<String>() + "…"
}

fn add_seconds(ts: &str, seconds: u64) -> Result<String, CliError> {
    let parsed = chrono::DateTime::parse_from_rfc3339(ts)
        .map_err(|err| CliError::ChannelConnectivity(format!("invalid timestamp '{}': {}", ts, err)))?;
    Ok((parsed + chrono::Duration::seconds(seconds as i64))
        .format("%Y-%m-%dT%H:%M:%S%:z")
        .to_string())
}
