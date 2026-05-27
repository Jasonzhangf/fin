use serde_json::{Value, json};

pub(crate) fn unknown_mobile_message(kind: &str, handshake_ok: bool) -> Value {
    if handshake_ok {
        json!({"type":"protocol.error","reason":"unknown_message_type","message_type":kind})
    } else {
        json!({"type":"handshake.auth_failed","reason":"handshake_required","message_type":kind})
    }
}
