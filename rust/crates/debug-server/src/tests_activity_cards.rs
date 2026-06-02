use crate::{API_ACTIVITY_CARDS_PATH, HttpRequest, NoopDebugActionHandler, routes};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_runtime_home() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-activity-cards-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ))
}

fn write_json(path: &Path, value: &impl serde::Serialize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, serde_json::to_vec_pretty(value).expect("json")).expect("write");
}

#[test]
fn activity_cards_route_returns_snapshot_json() {
    let runtime_home = temp_runtime_home();
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "session-empty",
            "submitted_at": "2026-04-19T12:00:00+08:00"
        }),
    );
    let handler = NoopDebugActionHandler;
    let response = routes::response_for_request(
        &HttpRequest {
            method: "GET".into(),
            path: API_ACTIVITY_CARDS_PATH.into(),
            body: Vec::new(),
        },
        &runtime_home,
        &handler,
    );
    let body = String::from_utf8(response.body).expect("utf8");
    assert_eq!(response.status_code, 200);
    assert!(body.contains("\"source_cards\""));
    assert!(body.contains("\"user_card\""));
}
