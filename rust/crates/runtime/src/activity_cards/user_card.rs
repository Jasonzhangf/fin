use fin_contracts::{ActivitySourceSummary, SourceActivityCardView, UserActivityCardView};

use super::SYSTEM_SOURCE_ID;
use super::helpers::{frontstage_recent_item, frontstage_source_recent_item, shorten};
use super::records::OwnerLoopActionRecord;

pub fn build_user_card(
    cards: &[SourceActivityCardView],
    generated_at: &str,
) -> UserActivityCardView {
    let total_sources = cards.len();
    let running_sources = cards
        .iter()
        .filter(|card| matches!(card.state.as_str(), "running"))
        .count();
    let waiting_sources = cards
        .iter()
        .filter(|card| matches!(card.state.as_str(), "waiting" | "paused"))
        .count();
    let failed_sources = cards
        .iter()
        .filter(|card| matches!(card.state.as_str(), "failed"))
        .count();
    let idle_sources = total_sources
        .saturating_sub(running_sources)
        .saturating_sub(waiting_sources)
        .saturating_sub(failed_sources);
    let focus = cards
        .iter()
        .find(|card| card.source_id == SYSTEM_SOURCE_ID)
        .or_else(|| {
            cards.iter().find(|card| {
                card.auto_promoted
                    && (card.source_id == SYSTEM_SOURCE_ID || card.source_kind == "system_agent")
            })
        })
        .or_else(|| {
            cards.iter().find(|card| {
                card.auto_promoted && !card.source_kind.starts_with("channel_gateway.")
            })
        })
        .or_else(|| {
            cards
                .iter()
                .find(|card| !card.source_kind.starts_with("channel_gateway."))
        })
        .or_else(|| cards.first());
    let header = if let Some(focus_card) = focus {
        format!("system frontstage · {}", shorten(&focus_card.summary, 96))
    } else {
        "system frontstage · idle".into()
    };
    UserActivityCardView {
        owner_source_id: SYSTEM_SOURCE_ID.into(),
        header,
        state: focus
            .map(|card| card.state.clone())
            .unwrap_or_else(|| "idle".into()),
        focus_source_id: focus.map(|card| card.source_id.clone()),
        focus_summary: focus.map(|card| card.summary.clone()),
        stage: focus.and_then(|card| card.current_activity.clone()),
        recent_items: frontstage_recent_items(cards, focus),
        active_sources: cards
            .iter()
            .map(|card| ActivitySourceSummary {
                source_id: card.source_id.clone(),
                title: card.title.clone(),
                state: card.state.clone(),
                summary: card.summary.clone(),
                visibility: card.visibility.clone(),
                auto_promoted: card.auto_promoted,
            })
            .collect(),
        total_sources,
        running_sources,
        waiting_sources,
        failed_sources,
        idle_sources,
        waiting_detail: focus.and_then(|card| card.waiting_detail.clone()),
        failure_detail: focus.and_then(|card| card.failure_detail.clone()),
        updated_at: focus
            .map(|card| card.updated_at.clone())
            .unwrap_or_else(|| generated_at.to_string()),
    }
}

fn frontstage_recent_items(
    cards: &[SourceActivityCardView],
    focus: Option<&SourceActivityCardView>,
) -> Vec<String> {
    let mut items = focus
        .map(|card| {
            card.recent_actions
                .iter()
                .take(3)
                .map(frontstage_recent_item)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let focus_id = focus.map(|card| card.source_id.as_str());
    for item in cards
        .iter()
        .filter(|card| Some(card.source_id.as_str()) != focus_id)
        .filter(|card| !card.source_kind.starts_with("channel_gateway."))
        .filter(|card| {
            matches!(
                card.state.as_str(),
                "running" | "waiting" | "paused" | "failed"
            )
        })
        .take(2)
        .map(|card| {
            if let Some(action) = card.recent_actions.first() {
                let label = card
                    .source_id
                    .rsplit('.')
                    .next()
                    .unwrap_or(card.source_id.as_str());
                return format!(
                    "{label} · {} · {}",
                    card.state.as_str(),
                    shorten(frontstage_recent_item(action).as_str(), 72)
                );
            }
            frontstage_source_recent_item(
                card.source_id.as_str(),
                card.state.as_str(),
                card.summary.as_str(),
                card.current_activity.as_deref(),
                card.failure_detail.as_deref(),
            )
        })
    {
        if !items.iter().any(|existing| existing == &item) {
            items.push(item);
        }
    }
    items
}

pub fn apply_dispatch_frontstage_overlay(
    system_card: &mut SourceActivityCardView,
    agent_cards: &[SourceActivityCardView],
    owner_loop_action: Option<&OwnerLoopActionRecord>,
) {
    let active_workers = agent_cards
        .iter()
        .filter(|card| {
            matches!(
                card.source_kind.as_str(),
                "system_worker" | "project_agent" | "project_worker"
            ) && matches!(
                card.state.as_str(),
                "running" | "waiting" | "paused" | "failed"
            )
        })
        .take(2)
        .map(|card| {
            let label = card
                .source_id
                .rsplit('.')
                .next()
                .unwrap_or(card.source_id.as_str());
            let detail = card
                .current_activity
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(card.summary.as_str());
            format!("{label}: {}", shorten(detail, 120))
        })
        .collect::<Vec<_>>();
    if !active_workers.is_empty() && matches!(system_card.state.as_str(), "idle" | "ready") {
        let detail = active_workers.join(" | ");
        system_card.state = "waiting".into();
        system_card.summary = "已派发任务，等待 worker 持续回报".into();
        system_card.current_activity = Some(format!("并行执行中：{detail}"));
        system_card.waiting_detail = Some(detail);
        system_card.failure_detail = None;
        system_card.recent_actions.clear();
        system_card.auto_promoted = true;
        return;
    }
    let Some(owner_loop_action) = owner_loop_action else {
        return;
    };
    if owner_loop_action.action_kind != "ready_tasks_present"
        || !matches!(system_card.state.as_str(), "idle" | "ready")
    {
        return;
    }
    let task_count = owner_loop_action.target_task_ids.len().max(1);
    let active_task = owner_loop_action.active_task_id.as_deref().unwrap_or("-");
    let waiting_detail =
        format!("最近已派发 {task_count} 个任务；当前关注 {active_task}；等待 worker 启动或回报");
    system_card.state = "waiting".into();
    system_card.summary = "已派发任务，等待 worker 回报".into();
    system_card.current_activity = Some(owner_loop_action.reason.clone());
    system_card.waiting_detail = Some(waiting_detail);
    system_card.failure_detail = None;
    system_card.recent_actions.clear();
    system_card.auto_promoted = true;
}
