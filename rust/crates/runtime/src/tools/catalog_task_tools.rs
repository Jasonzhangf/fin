use super::catalog::model_tool;
use fin_contracts::ToolCatalogEntry;

pub(super) fn build_project_task_model_tools() -> Vec<ToolCatalogEntry> {
    vec![
        model_tool(
            "project.task.status",
            "inspect one task status from runtime/session truth",
            "read the current or specified task execution state, routing state, and plan summary before deciding next action",
            vec![
                "you need grounded task status before reporting progress or deciding routing".into(),
                "you need current task state without guessing from memory".into(),
            ],
            vec![
                "the turn does not relate to any known task".into(),
                "you already have fresh task status from a just-produced artifact in this same step".into(),
            ],
            "optional task_id".into(),
            "task status summary + artifact refs".into(),
            vec!["reads execution_state / routing / plan artifacts".into()],
            vec!["inspect the current task before deciding whether to resume or switch".into()],
        ),
        model_tool(
            "project.task.list",
            "list known task ids from runtime/session truth",
            "discover reusable task threads before deciding whether to continue, switch, or report project progress",
            vec![
                "you need a grounded list of known tasks for the current runtime".into(),
                "the user asks which tasks exist or whether there is a prior task thread".into(),
            ],
            vec![
                "the exact target task id is already known".into(),
                "task discovery is irrelevant to the current turn".into(),
            ],
            "optional limit".into(),
            "known task ids with session linkage summary".into(),
            vec!["reads session task artifacts under runtime_home".into()],
            vec!["list recent task ids before choosing a resume path".into()],
        ),
        model_tool(
            "project.task.create",
            "create one managed task under session truth",
            "enter complex work into the managed task system so owner-loop dispatch and review can track it durably",
            vec![
                "the work is complex enough to require managed epic/task tracking instead of only update_plan".into(),
                "you need an explicit task record before assigning, claiming, or reviewing execution".into(),
            ],
            vec![
                "the work is a one-shot simple direct response with no managed follow-up".into(),
                "a suitable managed task record already exists and only needs status mutation".into(),
            ],
            "title + optional summary/status/task_id/epic_id/review_owner_worker_id".into(),
            "task creation receipt + registry/board artifact refs".into(),
            vec!["writes task registry + board artifacts".into()],
            vec!["create a managed task before dispatching the work to a project worker".into()],
        ),
        model_tool(
            "project.task.claim",
            "claim one managed task for execution",
            "mark a ready task as owned by one worker before execution starts",
            vec![
                "a worker is starting execution on a managed task".into(),
                "framework task truth should record who currently owns the execution slice".into(),
            ],
            vec![
                "the task is already terminal".into(),
                "another worker already owns the task and no takeover flow exists".into(),
            ],
            "task_id + optional worker_id/status".into(),
            "claim receipt + updated task registry/board refs".into(),
            vec!["writes task registry + board artifacts".into()],
            vec!["claim a ready task before running commands or patches for it".into()],
        ),
        model_tool(
            "project.task.submit",
            "submit one managed task result for review",
            "record completion candidate, evidence summary, and artifacts so the review owner can decide done/reopen/block",
            vec![
                "a worker completed or partially completed a claimed task and needs owner review".into(),
                "you need durable result summary instead of only free-form chat".into(),
            ],
            vec![
                "no concrete execution result exists yet".into(),
                "the task is claimed by another worker and you are not that worker".into(),
            ],
            "task_id + result_summary + optional artifact_refs[]".into(),
            "submission receipt + updated task registry/board refs".into(),
            vec!["writes task registry + board artifacts".into()],
            vec!["submit the build/test result summary for owner review".into()],
        ),
        model_tool(
            "project.task.review",
            "review one submitted managed task",
            "let the review owner approve, reopen, block, or cancel a task as part of owner-loop scheduling",
            vec![
                "the owner must turn a submitted task into done/reopen/block/cancel truth".into(),
                "completion should trigger unblock analysis and board refresh".into(),
            ],
            vec![
                "the current worker is not the task review owner".into(),
                "there is no review decision yet".into(),
            ],
            "task_id + decision(approve|reopen|block|cancel) + optional review_summary".into(),
            "review receipt + updated task registry/board refs".into(),
            vec!["writes task registry + board artifacts".into()],
            vec!["approve a submitted task after reviewing its evidence and unblock impact".into()],
        ),
        model_tool(
            "agent.presence.list",
            "list current framework-owned agent presence state",
            "inspect which agents are busy, idle, or waiting before routing, waking, or recovering work",
            vec![
                "you need grounded agent busy/idle/waiting state before dispatch or recovery".into(),
                "system role wants a presence registry snapshot instead of inferring worker state from chat".into(),
            ],
            vec![
                "the turn can be resolved without any agent coordination or recovery".into(),
                "the current context has no runtime_home, so presence truth is unavailable".into(),
            ],
            "optional limit + optional status filter".into(),
            "agent presence summary + listed agent states".into(),
            vec!["reads current_agent_presence_registry artifact".into()],
            vec!["list busy/idle agents before assigning a new slice".into()],
        ),
        model_tool(
            "project.supervision.list",
            "list current framework-owned project supervision state",
            "inspect project wake/resume/recover intent before deciding control-plane actions",
            vec![
                "you need grounded supervision intent before waking, recovering, or reprioritizing projects".into(),
                "system role wants explicit project control state instead of guessing from stale progress".into(),
            ],
            vec![
                "the turn does not involve project lifecycle, wakeup, recovery, or backlog coordination".into(),
                "the current context has no runtime_home, so supervision truth is unavailable".into(),
            ],
            "optional limit + optional desired_action filter".into(),
            "supervision summary counts + listed project desired actions".into(),
            vec!["reads current_project_supervision artifact".into()],
            vec!["list resume/recover actions before deciding which project to wake".into()],
        ),
    ]
}
