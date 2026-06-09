use crate::{
    API_UPDATE_DIR, API_UPDATE_LATEST_PATH, API_UPGRADE_MANIFEST_JS_PATH,
    API_UPGRADE_MANIFEST_JSON_PATH, HttpRequest, NoopDebugActionHandler, routes,
};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn runtime_home(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "{}-{}",
        prefix,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should work")
            .as_nanos()
    ))
}

fn write_update_file(runtime_home: &PathBuf, name: &str, bytes: &[u8]) {
    let update_dir = runtime_home.join(API_UPDATE_DIR);
    fs::create_dir_all(&update_dir).expect("update dir");
    fs::write(update_dir.join(name), bytes).expect("update file");
}

#[test]
fn updates_latest_serves_runtime_update_manifest() {
    let home = runtime_home("fin-debug-updates-latest");
    write_update_file(
        &home,
        "latest.json",
        br#"{"versionName":"test","apkUrl":"fin-latest-debug.apk"}"#,
    );

    let response = response("GET", API_UPDATE_LATEST_PATH, &home);

    assert_eq!(response.status_code, 200);
    assert_eq!(response.content_type, "application/json; charset=utf-8");
    let body = String::from_utf8(response.body).expect("manifest utf8");
    assert!(body.contains("fin-latest-debug.apk"));
}

#[test]
fn legacy_upgrade_manifest_paths_serve_same_manifest() {
    let home = runtime_home("fin-debug-upgrade-manifest");
    write_update_file(
        &home,
        "latest.json",
        br#"{"versionName":"alias","apkUrl":"fin-latest-debug.apk"}"#,
    );

    for path in [API_UPGRADE_MANIFEST_JSON_PATH, API_UPGRADE_MANIFEST_JS_PATH] {
        let response = response("GET", path, &home);

        assert_eq!(response.status_code, 200);
        assert_eq!(response.content_type, "application/json; charset=utf-8");
        let body = String::from_utf8(response.body).expect("manifest utf8");
        assert!(body.contains("alias"));
    }
}

#[test]
fn updates_apk_supports_get_and_head() {
    let home = runtime_home("fin-debug-updates-apk");
    write_update_file(&home, "fin-latest-debug.apk", b"apk-bytes");

    let get_response = response("GET", "/updates/fin-latest-debug.apk", &home);
    assert_eq!(get_response.status_code, 200);
    assert_eq!(
        get_response.content_type,
        "application/vnd.android.package-archive"
    );
    assert_eq!(get_response.body, b"apk-bytes");

    let head_response = response("HEAD", "/updates/fin-latest-debug.apk", &home);
    assert_eq!(head_response.status_code, 200);
    assert_eq!(
        head_response.content_type,
        "application/vnd.android.package-archive"
    );
    assert_eq!(head_response.content_length, b"apk-bytes".len());
    assert!(head_response.body.is_empty());
}

fn response(method: &str, path: &str, runtime_home: &PathBuf) -> crate::HttpResponse {
    routes::response_for_request(
        &HttpRequest {
            method: method.into(),
            path: path.into(),
            headers: Vec::new(),
            body: Vec::new(),
        },
        runtime_home,
        &NoopDebugActionHandler,
    )
}
