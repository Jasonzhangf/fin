use crate::EntityRefs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonStateRecord {
    pub daemon_id: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub service_kind: String,
    pub lifecycle_state: String,
    pub supervision_state: String,
    pub mode: String,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub last_heartbeat_id: Option<String>,
    #[serde(default)]
    pub last_cycle_id: Option<String>,
    #[serde(default)]
    pub health_state: Option<String>,
    pub recovery_needed: bool,
    #[serde(default)]
    pub recovery_action_kind: Option<String>,
    #[serde(default)]
    pub active_binding: Option<String>,
    pub status_summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonRecoveryActionRecord {
    pub action_id: String,
    pub created_at: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub source: String,
    pub action_kind: String,
    pub apply_immediately: bool,
    #[serde(default)]
    pub target_heartbeat_id: Option<String>,
    #[serde(default)]
    pub target_cycle_id: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonEnsurePeerRequestRecord {
    pub request_id: String,
    pub peer_id: String,
    pub peer_kind: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub agent_name: Option<String>,
    #[serde(default)]
    pub mode_hint: Option<String>,
    #[serde(default)]
    pub project_root: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
    pub lease_ttl_ms: u64,
    pub requested_at: String,
    #[serde(default)]
    pub requested_by_worker_id: Option<String>,
    pub status: String,
    #[serde(default)]
    pub consumed_at: Option<String>,
}
