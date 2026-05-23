use crate::context_assembly_plan::{ContextAssemblyPlan, ContextStabilityClass};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextBaselineRecord {
    pub session_id: String,
    pub baseline_id: String,
    pub stable_prefix_hash: String,
    pub role_id: String,
    pub tool_schema_hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextBaselineDiff {
    pub requires_full_reinject: bool,
    pub changed_fields: Vec<String>,
    pub stable_prefix_hash: String,
    pub tool_schema_hash: String,
}

#[derive(Debug, Clone, Default)]
pub struct ContextBaselineManager;

impl ContextBaselineManager {
    pub fn create(
        &self,
        session_id: &str,
        plan: &ContextAssemblyPlan,
        role_id: &str,
        created_at: &str,
    ) -> ContextBaselineRecord {
        ContextBaselineRecord {
            session_id: session_id.into(),
            baseline_id: format!("ctx-baseline-{}", stable_prefix_hash(plan)),
            stable_prefix_hash: stable_prefix_hash(plan),
            role_id: role_id.into(),
            tool_schema_hash: tool_schema_hash(plan),
            created_at: created_at.into(),
        }
    }

    pub fn diff(
        &self,
        previous: Option<&ContextBaselineRecord>,
        plan: &ContextAssemblyPlan,
        role_id: &str,
    ) -> ContextBaselineDiff {
        let stable_prefix_hash = stable_prefix_hash(plan);
        let tool_schema_hash = tool_schema_hash(plan);
        let Some(previous) = previous else {
            return ContextBaselineDiff {
                requires_full_reinject: true,
                changed_fields: vec!["missing_baseline".into()],
                stable_prefix_hash,
                tool_schema_hash,
            };
        };
        let mut changed_fields = Vec::new();
        if previous.stable_prefix_hash != stable_prefix_hash {
            changed_fields.push("stable_prefix_hash".into());
        }
        if previous.role_id != role_id {
            changed_fields.push("role_id".into());
        }
        if previous.tool_schema_hash != tool_schema_hash {
            changed_fields.push("tool_schema_hash".into());
        }
        ContextBaselineDiff {
            requires_full_reinject: !changed_fields.is_empty(),
            changed_fields,
            stable_prefix_hash,
            tool_schema_hash,
        }
    }
}

fn stable_prefix_hash(plan: &ContextAssemblyPlan) -> String {
    hash_sections(plan, |stability| {
        matches!(
            stability,
            ContextStabilityClass::Immutable | ContextStabilityClass::RarelyChanging
        )
    })
}

fn tool_schema_hash(plan: &ContextAssemblyPlan) -> String {
    let mut hasher = DefaultHasher::new();
    for section in &plan.sections {
        if section.section_id.contains("tool") || section.section_id.contains("capabilities") {
            section.section_id.hash(&mut hasher);
            section.body.hash(&mut hasher);
        }
    }
    format!("{:016x}", hasher.finish())
}

fn hash_sections(
    plan: &ContextAssemblyPlan,
    include: impl Fn(ContextStabilityClass) -> bool,
) -> String {
    let mut hasher = DefaultHasher::new();
    for section in &plan.sections {
        if include(section.stability) {
            section.section_id.hash(&mut hasher);
            section.body.hash(&mut hasher);
        }
    }
    format!("{:016x}", hasher.finish())
}
