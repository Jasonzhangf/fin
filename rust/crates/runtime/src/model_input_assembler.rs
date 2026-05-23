use crate::context_assembly_plan::{ContextAssemblyPlan, ContextAssemblyPlanner};
use fin_contracts::MinimalContextView;

#[derive(Debug, Clone, Default)]
pub struct ModelInputAssembler;

impl ModelInputAssembler {
    pub fn assemble(&self, input: &str, context: &MinimalContextView) -> String {
        let plan = self.assembly_plan(input, context);
        render_plan(&plan)
    }

    pub fn assembly_plan(&self, input: &str, context: &MinimalContextView) -> ContextAssemblyPlan {
        ContextAssemblyPlanner::default().build_plan(input, context)
    }
}

fn render_plan(plan: &ContextAssemblyPlan) -> String {
    plan.sections
        .iter()
        .map(|section| {
            format!(
                "{}:
{}",
                section.title, section.body
            )
        })
        .collect::<Vec<_>>()
        .join(
            "

",
        )
}
