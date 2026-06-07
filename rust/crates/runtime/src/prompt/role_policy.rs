use super::*;

#[test]
fn system_and_project_roles_share_runtime_but_receive_different_tool_policies() {
    let system_tools = crate::tools::catalog_dynamic::build_dynamic_catalog_block(
        &crate::tools::catalog_dynamic::DynamicToolCatalogInput {
            role_id: Some("system"),
            context: &MinimalContextView::default(),
            recent_tool_records: &[],
            round_index: 1,
        },
    );
    let project_tools = crate::tools::catalog_dynamic::build_dynamic_catalog_block(
        &crate::tools::catalog_dynamic::DynamicToolCatalogInput {
            role_id: Some("project"),
            context: &MinimalContextView::default(),
            recent_tool_records: &[],
            round_index: 1,
        },
    );

    assert!(system_tools.tool_selection_policy.iter().any(|item| {
        item.contains("framework-owned backlog/task-board state")
            && item.contains("agent presence")
            && item.contains("supervision")
    }));
    assert!(
        system_tools
            .tool_selection_policy
            .iter()
            .any(|item| item.contains("system role uses the same runtime/tooling foundation"))
    );
    assert!(
        project_tools
            .tool_selection_policy
            .iter()
            .any(|item| item.contains("prefer project-scoped closure, task-board execution management, worker dispatch, review, and delivery progress"))
    );

    for (tool_name, expected) in [
        ("peer.list", "system role"),
        ("agent.presence.list", "system role"),
        ("project.task.create", "system role"),
    ] {
        let tool = system_tools
            .model_tools
            .iter()
            .find(|tool| tool.tool_name == tool_name)
            .unwrap_or_else(|| panic!("missing system tool {tool_name}"));
        assert!(tool.when_to_use.iter().any(|item| item.contains(expected)));
    }

    for (tool_name, expected) in [
        ("apply_patch", "project role"),
        ("project.task.submit", "project role"),
        ("project.task.create", "project role"),
        ("project.task.review", "project role"),
        ("agent.assign", "project role"),
    ] {
        let tool = project_tools
            .model_tools
            .iter()
            .find(|tool| tool.tool_name == tool_name)
            .unwrap_or_else(|| panic!("missing project tool {tool_name}"));
        assert!(tool.when_to_use.iter().any(|item| item.contains(expected)));
    }

    let default_tools = crate::tools::catalog_dynamic::build_dynamic_catalog_block(
        &crate::tools::catalog_dynamic::DynamicToolCatalogInput {
            role_id: Some("default"),
            context: &MinimalContextView::default(),
            recent_tool_records: &[],
            round_index: 1,
        },
    );
    assert!(
        default_tools
            .tool_selection_policy
            .iter()
            .any(|item| item.contains("same project role"))
    );
}

#[test]
fn owner_loop_truth_biases_review_before_dispatch_and_ready_before_new_work() {
    let system_tools = crate::tools::catalog_dynamic::build_dynamic_catalog_block(
        &crate::tools::catalog_dynamic::DynamicToolCatalogInput {
            role_id: Some("system"),
            context: &MinimalContextView {
                project: Some(fin_contracts::ProjectContextBlock {
                    submitted_task_ids: vec!["task-review-a".into()],
                    ..fin_contracts::ProjectContextBlock::default()
                }),
                ..MinimalContextView::default()
            },
            recent_tool_records: &[],
            round_index: 1,
        },
    );
    assert!(system_tools.tool_selection_policy.iter().any(|item| {
        item.contains("submitted managed tasks exist [task-review-a]")
            && item.contains("review these before dispatching more work")
    }));
    assert!(
        system_tools
            .model_tools
            .iter()
            .find(|tool| tool.tool_name == "project.task.review")
            .is_some_and(|tool| tool
                .when_to_use
                .iter()
                .any(|item| item.contains("waiting for review owner decision")))
    );

    let project_tools = crate::tools::catalog_dynamic::build_dynamic_catalog_block(
        &crate::tools::catalog_dynamic::DynamicToolCatalogInput {
            role_id: Some("project"),
            context: &MinimalContextView {
                project: Some(fin_contracts::ProjectContextBlock {
                    ready_task_ids: vec!["task-ready-a".into()],
                    ..fin_contracts::ProjectContextBlock::default()
                }),
                ..MinimalContextView::default()
            },
            recent_tool_records: &[],
            round_index: 1,
        },
    );
    assert!(project_tools.tool_selection_policy.iter().any(|item| {
        item.contains("ready unclaimed managed tasks exist [task-ready-a]")
            && item.contains("dispatch or claim them before inventing new managed work")
    }));
    assert!(
        project_tools
            .model_tools
            .iter()
            .find(|tool| tool.tool_name == "project.task.claim")
            .is_some_and(|tool| tool
                .when_to_use
                .iter()
                .any(|item| item.contains("ready unclaimed managed task is available")))
    );
}
