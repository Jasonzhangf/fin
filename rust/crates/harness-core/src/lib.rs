use fin_contracts::TaskStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayScenario {
    pub name: String,
    pub expected_final_status: TaskStatus,
    pub injected_faults: Vec<String>,
}
