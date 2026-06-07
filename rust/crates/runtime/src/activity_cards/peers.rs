use super::helpers::{peer_activity, peer_state, peer_summary, peer_title, should_promote};
use super::records::PeerRegistryEntry;
use fin_contracts::SourceActivityCardView;

pub fn build_peer_cards_from_registry(
    peers: &[PeerRegistryEntry],
    session_id: Option<&str>,
    task_id: Option<&str>,
) -> Vec<SourceActivityCardView> {
    peers
        .iter()
        .map(|peer| {
            let state = peer_state(peer);
            let failure_detail = peer
                .connectivity_state
                .as_deref()
                .filter(|value| matches!(*value, "degraded" | "failed" | "disconnected"))
                .map(|value| format!("connectivity {value}"));
            let waiting_detail = peer
                .binding_state
                .as_deref()
                .filter(|value| matches!(*value, "pairing_required" | "invalidated"))
                .map(|value| format!("binding {value}"));
            SourceActivityCardView {
                source_id: peer.peer_id.clone(),
                source_kind: peer.peer_kind.clone(),
                title: peer_title(peer),
                visibility: if failure_detail.is_some() || waiting_detail.is_some() {
                    "detailed".into()
                } else {
                    "compact".into()
                },
                state: state.clone(),
                summary: peer_summary(peer),
                focus_label: peer
                    .session_id
                    .as_deref()
                    .map(|value| format!("session {value}")),
                auto_promoted: should_promote(
                    state.as_str(),
                    failure_detail.as_deref(),
                    waiting_detail.as_deref(),
                ),
                current_activity: Some(peer_activity(peer)),
                recent_actions: Vec::new(),
                waiting_detail,
                failure_detail,
                session_id: session_id
                    .filter(|_| peer.session_id.as_deref() == session_id)
                    .map(str::to_string),
                task_id: task_id.map(str::to_string),
                updated_at: peer.updated_at.clone(),
            }
        })
        .collect()
}
