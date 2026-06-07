use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub(super) struct PeerRegistry {
    #[serde(default)]
    pub(super) peers: Vec<PeerRegistryEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub(super) struct PeerRegistryEntry {
    pub(super) peer_id: String,
    pub(super) peer_kind: String,
    #[serde(default)]
    pub(super) alias: Option<String>,
    #[serde(default)]
    pub(super) label: Option<String>,
    pub(super) presence_state: String,
    #[serde(default)]
    pub(super) lifecycle_state: String,
    #[serde(default)]
    pub(super) runtime_state: Option<String>,
    #[serde(default)]
    pub(super) connectivity_state: Option<String>,
    #[serde(default)]
    pub(super) binding_state: Option<String>,
    pub(super) updated_at: String,
    #[serde(default)]
    pub(super) pairing_required: Option<bool>,
    #[serde(default)]
    pub(super) session_valid: Option<bool>,
    #[serde(default)]
    pub(super) session_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub(super) struct ChannelConversationRegistry {
    #[serde(default)]
    pub(super) conversations: Vec<ChannelConversationRecord>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub(super) struct ChannelConversationRecord {
    #[serde(default)]
    pub(super) session_id: Option<String>,
    #[serde(default)]
    pub(super) last_inbound_message_id: Option<String>,
    #[serde(default)]
    pub(super) last_inbound_at: Option<String>,
    #[serde(default)]
    pub(super) last_delivered_message_id: Option<String>,
    #[serde(default)]
    pub(super) last_delivery_at: Option<String>,
    #[serde(default)]
    pub(super) status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub(super) struct StartupControlSummaryRecord {
    #[serde(default)]
    pub(super) startup_config_summary: String,
    #[serde(default)]
    pub(super) startup_state_summary: String,
    #[serde(default)]
    pub(super) started_resource_count: usize,
    #[serde(default)]
    pub(super) busy_resource_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub(super) struct OwnerLoopActionRecord {
    #[serde(default)]
    pub(super) action_kind: String,
    #[serde(default)]
    pub(super) reason: String,
    #[serde(default)]
    pub(super) active_task_id: Option<String>,
    #[serde(default)]
    pub(super) target_task_ids: Vec<String>,
}
