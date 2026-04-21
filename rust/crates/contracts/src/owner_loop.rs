use crate::EntityRefs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerLoopActionRecord {
    pub action_id: String,
    pub created_at: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub source: String,
    pub action_kind: String,
    #[serde(default)]
    pub active_task_id: Option<String>,
    #[serde(default)]
    pub target_task_ids: Vec<String>,
    #[serde(default)]
    pub task_status_counts: Vec<String>,
    pub reason: String,
}
