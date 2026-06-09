use super::*;
use crate::{DebugBinding, NoopDebugActionHandler};

#[test]
fn websocket_accept_matches_rfc_example() {
    assert_eq!(
        websocket_accept_value("dGhlIHNhbXBsZSBub25jZQ=="),
        "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    );
}

#[test]
fn websocket_accept_requires_upgrade_headers() {
    let request = HttpRequest {
        method: "GET".into(),
        path: "/ws".into(),
        headers: vec![
            ("Connection".into(), "Upgrade".into()),
            ("Upgrade".into(), "websocket".into()),
            ("Sec-WebSocket-Key".into(), "abc".into()),
        ],
        body: vec![],
    };
    assert!(websocket_accept_key(&request).is_some());
}

#[test]
fn session_snapshot_contains_mobile_boot_events() {
    struct Handler;
    impl DebugActionHandler for Handler {
        fn read_binding(&self, _: &Path) -> Result<DebugBinding, String> {
            Ok(DebugBinding {
                project_id: "fin".into(),
                project_label: "fin".into(),
                runtime_home: "/tmp".into(),
                session_id: Some("session-live".into()),
                task_id: None,
                session_messages_path: None,
                recent_contexts_path: None,
                recent_digests_path: None,
            })
        }

        fn send_chat_message(
            &self,
            _: &Path,
            _: ChatSendRequest,
        ) -> Result<crate::ChatSendResponse, String> {
            unreachable!("snapshot should not send chat")
        }
    }
    let snapshot = session_snapshot(Path::new("/tmp/missing-fin-ws"), &Handler);
    assert!(
        snapshot
            .iter()
            .any(|event| event.contains("\"type\":\"session.list\""))
    );
    assert!(
        snapshot
            .iter()
            .any(|event| event.contains("\"type\":\"runtime.health\""))
    );
    assert!(
        snapshot
            .iter()
            .any(|event| event.contains("\"type\":\"provider.health\""))
    );

    let no_session = session_snapshot(Path::new("/tmp/missing-fin-ws"), &NoopDebugActionHandler);
    assert!(
        no_session
            .iter()
            .any(|event| event.contains("\"sessions\":[]"))
    );
}
