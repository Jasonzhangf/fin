import { resolveEventLedgerState } from './event_ledger_state.js';
import { buildFocusTurns } from './focus.js';
import type {
  ClosureTraceRecord,
  ContextSnapshotRecord,
  DebugBinding,
  DigestRecord,
  EventArchiveIndex,
  JsonRecord,
  ReasoningViewRecord,
  RefreshState,
  RuntimeEvent,
  SessionMessage,
  TurnRecord,
  ToolExecutionRecord,
} from './types.js';

export type FetchJson = <T>(url: string) => Promise<T>;

export async function loadRefreshState(
  fetchJson: FetchJson,
  previousState: RefreshState,
): Promise<RefreshState> {
  const [
    binding,
    recentContexts,
    recentDigests,
    recentReasoningViews,
    recentToolRecords,
    recentClosures,
    recentTurns,
    currentExecutionState,
    currentPendingInputs,
    currentPauseCheckpoint,
    currentInterruptedSegment,
    currentSegmentMerge,
    currentRoutingDecision,
    messages,
    sessionEvents,
    eventArchiveIndex,
    lastRun,
    currentContext,
  ] = await Promise.all([
    fetchJson<DebugBinding>('/api/binding.json').catch(() => null),
    fetchJson<ContextSnapshotRecord[]>('/api/recent_contexts.json').catch(() => []),
    fetchJson<DigestRecord[]>('/api/recent_digests.json').catch(() => []),
    fetchJson<ReasoningViewRecord[]>('/api/recent_reasoning_views.json').catch(() => []),
    fetchJson<ToolExecutionRecord[]>('/api/recent_tool_records.json').catch(() => []),
    fetchJson<ClosureTraceRecord[]>('/api/recent_closures.json').catch(() => []),
    fetchJson<TurnRecord[]>('/api/recent_turns.json').catch(() => []),
    fetchJson<JsonRecord>('/api/current_execution_state.json').catch(() => null),
    fetchJson<JsonRecord[]>('/api/current_pending_inputs.json').catch(() => []),
    fetchJson<JsonRecord>('/api/current_pause_checkpoint.json').catch(() => null),
    fetchJson<JsonRecord>('/api/current_interrupted_segment.json').catch(() => null),
    fetchJson<JsonRecord>('/api/current_segment_merge.json').catch(() => null),
    fetchJson<JsonRecord>('/api/current_routing_decision.json').catch(() => null),
    fetchJson<SessionMessage[]>('/api/session_messages.json').catch(() => []),
    fetchJson<RuntimeEvent[]>('/api/session_events.json').catch(() => []),
    fetchJson<EventArchiveIndex>('/api/session_event_archive_index.json').catch(() => null),
    fetchJson<JsonRecord>('/api/last_run.json').catch(() => null),
    fetchJson<JsonRecord>('/api/current_context.json').catch(() => null),
  ]);
  const liveEvents = Array.isArray(sessionEvents) ? sessionEvents : [];
  let eventLedgerScope = previousState.eventLedgerScope;
  let eventLedgerSegment = previousState.eventLedgerSegment;
  let eventLedgerEvents = liveEvents;
  if (eventLedgerScope !== 'live' && eventLedgerSegment) {
    const tier = eventLedgerScope === 'local_archive' ? 'local' : 'cold';
    eventLedgerEvents = await fetchJson<RuntimeEvent[]>(
      `/api/session_events_segment.json?tier=${tier}&segment=${encodeURIComponent(eventLedgerSegment)}`,
    ).catch(() => []);
  }
  const resolvedLedgerState = resolveEventLedgerState(
    eventArchiveIndex,
    eventLedgerScope,
    eventLedgerSegment,
    Array.isArray(eventLedgerEvents) ? eventLedgerEvents : [],
    previousState.eventLedgerSelectedOperationId,
    previousState.selectedOperationId,
  );
  eventLedgerScope = resolvedLedgerState.scope;
  eventLedgerSegment = resolvedLedgerState.segment;
  const eventLedgerSelectedOperationId = resolvedLedgerState.selectedOperationId;

  const focusTurns = buildFocusTurns(
    messages,
    recentContexts,
    recentDigests,
    recentReasoningViews,
    recentToolRecords,
    recentClosures,
    liveEvents,
  );
  const selectedOperationId = focusTurns.some((turn) => turn.operationId === previousState.selectedOperationId)
    ? previousState.selectedOperationId
    : (focusTurns.length ? focusTurns[focusTurns.length - 1].operationId : null);

  return {
    binding,
    projection: {},
    events: [],
    sessionEvents: liveEvents,
    eventArchiveIndex,
    eventLedgerScope,
    eventLedgerSegment,
    eventLedgerEvents: Array.isArray(eventLedgerEvents) ? eventLedgerEvents : [],
    eventLedgerSelectedOperationId,
    lastRun,
    currentContext,
    recentContexts: Array.isArray(recentContexts) ? recentContexts : [],
    recentDigests: Array.isArray(recentDigests) ? recentDigests : [],
    recentReasoningViews: Array.isArray(recentReasoningViews) ? recentReasoningViews : [],
    recentToolRecords: Array.isArray(recentToolRecords) ? recentToolRecords : [],
    recentClosures: Array.isArray(recentClosures) ? recentClosures : [],
    recentTurns: Array.isArray(recentTurns) ? recentTurns : [],
    currentExecutionState,
    currentPendingInputs: Array.isArray(currentPendingInputs) ? currentPendingInputs : [],
    currentPauseCheckpoint,
    currentInterruptedSegment,
    currentSegmentMerge,
    currentRoutingDecision,
    messages: Array.isArray(messages) ? messages : [],
    focusTurns,
    selectedOperationId,
    conversationRichness: previousState.conversationRichness,
    openedCard: previousState.openedCard,
    openedSectionKey: previousState.openedSectionKey,
  };
}
