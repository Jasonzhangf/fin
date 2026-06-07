use super::*;
use crate::channel_peer::complete_builtin_qqbot_pairing;
use crate::channel_peer_activity_delivery::channel_peer_activity_delivery_render::render_recent_action;
use fin_contracts::{
    ActivitySourceSummary, SourceActivityCardView, ToolSemanticView, UserActivityCardView,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

pub(super) fn temp_runtime_home() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-qqbot-activity-delivery-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos(),
        TEMP_SEQ.fetch_add(1, Ordering::Relaxed),
    ))
}

pub(super) fn write_json(path: &Path, value: &impl serde::Serialize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, serde_json::to_vec_pretty(value).expect("json")).expect("write");
}

mod periodic_basic;
mod periodic_repeat;
mod periodic_task_list;
mod periodic_worker;
mod render_basic;
mod render_identifiers;
