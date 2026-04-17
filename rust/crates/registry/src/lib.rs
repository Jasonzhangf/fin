use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerHeartbeat {
    pub worker_id: String,
    pub node_id: String,
    pub healthy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RegistrySnapshot {
    pub workers: Vec<WorkerHeartbeat>,
}

impl RegistrySnapshot {
    pub fn register(&mut self, heartbeat: WorkerHeartbeat) {
        self.workers.push(heartbeat);
    }
}