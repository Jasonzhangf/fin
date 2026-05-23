use crate::context_assembly_plan::ContextAssemblyPlan;
use fin_provider::TokenUsage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextCompactionDecisionKind {
    NoCompact,
    PreTurnCompact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextBudgetDecision {
    pub decision: ContextCompactionDecisionKind,
    pub observed_prompt_tokens: usize,
    pub threshold_tokens: usize,
    pub evidence_source: String,
    pub evidence_strength: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextBudgetManager {
    pub compact_threshold_tokens: usize,
}

impl ContextBudgetManager {
    pub fn new(compact_threshold_tokens: usize) -> Self {
        Self {
            compact_threshold_tokens,
        }
    }

    pub fn decide(
        &self,
        plan: &ContextAssemblyPlan,
        provider_usage: Option<&TokenUsage>,
    ) -> ContextBudgetDecision {
        let threshold_tokens = self.compact_threshold_tokens.max(1);
        let (observed_prompt_tokens, evidence_source, evidence_strength) = provider_usage
            .and_then(|usage| usage.prompt_tokens.map(|tokens| (tokens as usize, usage)))
            .map(|(tokens, usage)| {
                (
                    tokens,
                    usage.usage_source.clone(),
                    if usage.usage_source.starts_with("provider_") {
                        "strong".to_string()
                    } else {
                        "weak".to_string()
                    },
                )
            })
            .unwrap_or_else(|| {
                (
                    plan.budget.prompt_token_estimate,
                    "runtime_estimate".to_string(),
                    "weak".to_string(),
                )
            });
        let reached_threshold = observed_prompt_tokens >= threshold_tokens;
        ContextBudgetDecision {
            decision: if reached_threshold {
                ContextCompactionDecisionKind::PreTurnCompact
            } else {
                ContextCompactionDecisionKind::NoCompact
            },
            observed_prompt_tokens,
            threshold_tokens,
            evidence_source,
            evidence_strength,
            reason: if reached_threshold {
                format!("prompt_tokens>={threshold_tokens}")
            } else {
                "below_threshold".into()
            },
        }
    }
}

impl Default for ContextBudgetManager {
    fn default() -> Self {
        Self::new(120_000)
    }
}
