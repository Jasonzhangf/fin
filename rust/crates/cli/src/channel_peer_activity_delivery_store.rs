use super::{ActivityCardsSnapshot, QqbotActivityDeliveryState};
use crate::{
    CliError,
    channel_peer_activity_signature::{signature_source_card, signature_user_card},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn load_state(runtime_home: &Path) -> Result<QqbotActivityDeliveryState, CliError> {
    let path = state_path(runtime_home);
    match fs::read_to_string(&path) {
        Ok(body) => serde_json::from_str(&body).map_err(CliError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Ok(QqbotActivityDeliveryState::default())
        }
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

pub(super) fn persist_state(
    runtime_home: &Path,
    state: &QqbotActivityDeliveryState,
) -> Result<(), CliError> {
    let path = state_path(runtime_home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(
        &path,
        serde_json::to_vec_pretty(state).map_err(CliError::Serialize)?,
    )
    .map_err(|source| CliError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

fn state_path(runtime_home: &Path) -> PathBuf {
    runtime_home.join("runtime/peers/qqbot/activity_delivery_state.json")
}

pub(super) fn signature_for_snapshot(snapshot: &ActivityCardsSnapshot) -> String {
    serde_json::to_string(&serde_json::json!({
        "session_id": snapshot.session_id,
        "task_id": snapshot.task_id,
        "user_card": snapshot.user_card.as_ref().map(signature_user_card),
        "source_cards": snapshot.source_cards.iter().map(signature_source_card).collect::<Vec<_>>(),
    }))
    .unwrap_or_else(|_| "activity-cards-signature-error".into())
}
