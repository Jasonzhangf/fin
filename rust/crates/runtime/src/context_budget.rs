use crate::context_assembly_plan::ContextAssemblyPlan;
use fin_provider::TokenUsage;
use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoldLevel {
    NoFold = 0,
    NormalFold = 1,
    AggressiveFold = 2,
    ForceSummary = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextCompactionDecisionKind {
    NoCompact,
    PreTurnCompact,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextBudgetDecision {
    pub decision: ContextCompactionDecisionKind,
    pub observed_prompt_tokens: usize,
    pub threshold_tokens: usize,
    pub evidence_source: String,
    pub evidence_strength: String,
    pub reason: String,
    pub fold_level: FoldLevel,
    pub cached_ratio: f64,
    pub tail_budget: Option<usize>,
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
        let ratio = if threshold_tokens > 0 {
            observed_prompt_tokens as f64 / threshold_tokens as f64
        } else {
            0.0
        };
        let cached_ratio = provider_usage
            .and_then(|usage| {
                let cached = usage.cached_tokens? as f64;
                let prompt = usage.prompt_tokens? as f64;
                if prompt > 0.0 { Some(cached / prompt) } else { None }
            })
            .unwrap_or(0.0);
        let fold_level = if ratio < 0.75 {
            FoldLevel::NoFold
        } else if ratio < 0.78 {
            FoldLevel::NormalFold
        } else if ratio < 0.85 {
            FoldLevel::AggressiveFold
        } else {
            FoldLevel::ForceSummary
        };
        let tail_budget = if !matches!(fold_level, FoldLevel::NoFold) {
            let tail_fraction = match fold_level {
                FoldLevel::NormalFold => 0.2,
                FoldLevel::AggressiveFold => 0.1,
                _ => 0.0,
            };
            Some((observed_prompt_tokens as f64 * tail_fraction) as usize)
        } else {
            None
        };
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
            fold_level,
            cached_ratio,
            tail_budget,
        }
    }
}

impl Default for ContextBudgetManager {
    fn default() -> Self {
        Self::new(120_000)
    }
}
