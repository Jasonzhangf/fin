use super::*;

pub(super) fn build_system_card(
    session_id: Option<&str>,
    task_id: Option<&str>,
    execution_state: Option<&ExecutionStateRecord>,
    startup_summary: Option<&StartupControlSummaryRecord>,
    semantics: &[ToolSemanticView],
    turns: &[TurnRecord],
    generated_at: &str,
) -> SourceActivityCardView {
    let recent_actions = most_recent_actions(semantics, 3);
    let latest_turn = turns.last();
    let startup_fallback_only =
        execution_state.is_none() && latest_turn.is_none() && recent_actions.is_empty();
    let state = execution_state
        .map(|value| value.status.clone())
        .or_else(|| latest_turn.map(|turn| turn.status.clone()))
        .or_else(|| {
            startup_summary.map(|summary| {
                if summary.busy_resource_count > 0 {
                    "running".into()
                } else if summary.started_resource_count > 0 {
                    "ready".into()
                } else {
                    "idle".into()
                }
            })
        })
        .unwrap_or_else(|| "idle".into());
    let execution_reason = execution_state
        .and_then(|state| state.reason.as_ref())
        .filter(|reason| !reason.trim().is_empty())
        .cloned();
    let waiting_detail = execution_state
        .and_then(|state| state.reason.as_ref())
        .filter(|_| matches!(state.as_str(), "waiting" | "paused"))
        .cloned();
    let failure_detail = latest_failed_action(semantics)
        .map(|value| value.summary)
        .or_else(|| {
            latest_turn
                .filter(|turn| turn.status == "failed")
                .and_then(|turn| turn.progress_summary.clone())
        });
    let summary = if startup_fallback_only {
        startup_summary
            .and_then(|item| (!item.startup_config_summary.trim().is_empty()).then_some(item))
            .map(|item| item.startup_config_summary.clone())
            .unwrap_or_else(|| state.clone())
    } else if matches!(state.as_str(), "running" | "waiting" | "paused") {
        execution_reason
            .clone()
            .or_else(|| {
                latest_turn.and_then(|turn| {
                    turn.progress_summary
                        .clone()
                        .or_else(|| turn.assistant_visible_output.clone())
                })
            })
            .or_else(|| recent_actions.first().map(|action| action.summary.clone()))
            .unwrap_or_else(|| state.clone())
    } else if let Some(action) = recent_actions.first() {
        action.summary.clone()
    } else if let Some(turn) = latest_turn {
        turn.progress_summary
            .clone()
            .or_else(|| turn.assistant_visible_output.clone())
            .unwrap_or_else(|| "no recent activity".into())
    } else {
        "idle".into()
    };
    let current_activity = if startup_fallback_only {
        startup_summary
            .and_then(|item| (!item.startup_state_summary.trim().is_empty()).then_some(item))
            .map(|item| item.startup_state_summary.clone())
    } else if matches!(state.as_str(), "running" | "waiting" | "paused") {
        execution_reason.clone().or_else(|| {
            latest_turn.and_then(|turn| {
                turn.progress_summary
                    .clone()
                    .or_else(|| turn.assistant_visible_output.clone())
            })
        })
    } else {
        latest_turn
            .and_then(|turn| turn.progress_summary.clone())
            .or_else(|| recent_actions.first().map(|item| item.summary.clone()))
    };

    SourceActivityCardView {
        source_id: SYSTEM_SOURCE_ID.into(),
        source_kind: "system_agent".into(),
        title: "Kobe".into(),
        visibility: visibility_for_state(state.as_str()).into(),
        state: state.clone(),
        summary: shorten(&summary, 120),
        focus_label: latest_turn
            .and_then(|turn| turn.turn_id.split('-').next_back())
            .map(|suffix| format!("turn {suffix}")),
        auto_promoted: should_promote(
            state.as_str(),
            failure_detail.as_deref(),
            waiting_detail.as_deref(),
        ),
        current_activity: current_activity.map(|value| shorten(&value, 120)),
        recent_actions,
        waiting_detail,
        failure_detail,
        session_id: session_id.map(str::to_string),
        task_id: task_id.map(str::to_string),
        updated_at: execution_state
            .map(|value| value.updated_at.clone())
            .or_else(|| latest_turn.and_then(|turn| turn.completed_at.clone()))
            .unwrap_or_else(|| generated_at.to_string()),
    }
}

pub(super) fn build_peer_cards(
    runtime_home: &Path,
    peers: &[PeerRegistryEntry],
    session_id: Option<&str>,
    task_id: Option<&str>,
) -> Vec<SourceActivityCardView> {
    let runs = read_json_vec_if_exists::<AgentRunRecordView>(
        &runtime_home.join("runtime/agents/control/runs.json"),
    )
    .unwrap_or_default();
    let supervision = read_json_if_exists::<ProjectSupervisionSnapshotView>(
        &runtime_home.join("runtime/current/current_project_supervision.json"),
    )
    .unwrap_or(None)
    .unwrap_or_default();
    let runtime_pickups = read_json_if_exists::<ProjectRuntimePickupSnapshotView>(
        &runtime_home.join("runtime/current/current_project_runtime_pickups.json"),
    )
    .unwrap_or(None)
    .unwrap_or_default();
    peers
        .iter()
        .map(|peer| {
            let active_run = runs
                .iter()
                .filter(|run| run.agent_id == peer.peer_id)
                .max_by(|left, right| {
                    left.last_heartbeat_at
                        .cmp(&right.last_heartbeat_at)
                        .then_with(|| left.agent_run_id.cmp(&right.agent_run_id))
                });
            let run_mailbox =
                read_json_vec_if_exists::<AgentMailboxMessageView>(&runtime_home.join(format!(
                    "runtime/agents/control/mailbox/{}/inbox.json",
                    peer.peer_id
                )))
                .unwrap_or_default();
            let mut result_mailbox =
                read_json_vec_if_exists::<AgentMailboxMessageView>(&runtime_home.join(format!(
                    "runtime/agents/control/mailbox/{}/inbox.json",
                    SYSTEM_SOURCE_ID
                )))
                .unwrap_or_default();
            for system_peer_id in peers
                .iter()
                .filter(|candidate| candidate.peer_kind == "system_agent")
                .map(|candidate| candidate.peer_id.as_str())
                .filter(|peer_id| *peer_id != SYSTEM_SOURCE_ID)
            {
                result_mailbox.extend(
                    read_json_vec_if_exists::<AgentMailboxMessageView>(&runtime_home.join(
                        format!("runtime/agents/control/mailbox/{system_peer_id}/inbox.json"),
                    ))
                    .unwrap_or_default(),
                );
            }
            let state = peer_state(peer, active_run);
            let supervision_record = supervision
                .projects
                .iter()
                .find(|project| project.agent_id == peer.peer_id);
            let runtime_pickup_record = runtime_pickups
                .projects
                .iter()
                .find(|project| project.agent_id == peer.peer_id);
            let failure_detail = active_run
                .filter(|run| run.status == "failed")
                .map(|run| format!("delegated run {} failed", run.agent_run_id))
                .or_else(|| {
                    peer.connectivity_state
                        .as_deref()
                        .filter(|value| matches!(*value, "degraded" | "failed" | "disconnected"))
                        .map(|value| format!("connectivity {value}"))
                });
            let waiting_detail = active_run
                .filter(|run| matches!(run.status.as_str(), "running" | "timeout"))
                .map(|run| {
                    let delegated = run_mailbox
                        .iter()
                        .rev()
                        .find(|message| {
                            message.payload.get("agent_run_id").and_then(Value::as_str)
                                == Some(run.agent_run_id.as_str())
                        })
                        .and_then(|message| {
                            message
                                .payload
                                .get("task_summary")
                                .and_then(Value::as_str)
                                .map(str::to_string)
                        })
                        .unwrap_or_else(|| "delegated task running".into());
                    format!("{delegated} · run {}", run.agent_run_id)
                })
                .or_else(|| {
                    peer.binding_state
                        .as_deref()
                        .filter(|value| matches!(*value, "pairing_required" | "invalidated"))
                        .map(|value| format!("binding {value}"))
                });
            let recent_actions =
                project_recent_actions(runtime_home, &peer.peer_id, &peer.peer_kind);
            let summary = peer_summary(
                peer,
                active_run,
                result_mailbox.as_slice(),
                supervision_record,
                runtime_pickup_record,
            );
            let current_activity = peer_activity(
                peer,
                active_run,
                result_mailbox.as_slice(),
                supervision_record,
                runtime_pickup_record,
            );
            SourceActivityCardView {
                source_id: peer.peer_id.clone(),
                source_kind: peer.peer_kind.clone(),
                title: peer_title(peer),
                visibility: if failure_detail.is_some() || waiting_detail.is_some() {
                    "detailed".into()
                } else {
                    "compact".into()
                },
                state: state.clone(),
                summary,
                focus_label: peer
                    .session_id
                    .as_deref()
                    .map(|value| format!("session {value}")),
                auto_promoted: should_promote(
                    state.as_str(),
                    failure_detail.as_deref(),
                    waiting_detail.as_deref(),
                ),
                current_activity: Some(current_activity),
                recent_actions,
                waiting_detail,
                failure_detail,
                session_id: session_id
                    .filter(|_| peer.session_id.as_deref() == session_id)
                    .map(str::to_string),
                task_id: task_id.map(str::to_string),
                updated_at: peer.updated_at.clone(),
            }
        })
        .collect()
}

pub(super) fn build_user_card(
    cards: &[SourceActivityCardView],
    generated_at: &str,
) -> UserActivityCardView {
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
        recent_items: focus
            .map(|card| {
                card.recent_actions
                    .iter()
                    .take(3)
                    .map(|action| action.summary.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
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
        waiting_detail: focus.and_then(|card| card.waiting_detail.clone()),
        failure_detail: focus.and_then(|card| card.failure_detail.clone()),
        updated_at: focus
            .map(|card| card.updated_at.clone())
            .unwrap_or_else(|| generated_at.to_string()),
    }
}
