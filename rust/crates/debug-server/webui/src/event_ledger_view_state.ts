import type { EventLedgerView, JsonRecord, RefreshState } from './types.js';

export function buildEventLedgerView(state: RefreshState): EventLedgerView {
  const operationId = state.eventLedgerSelectedOperationId;
  return {
    index: state.eventArchiveIndex,
    scope: state.eventLedgerScope,
    segment: state.eventLedgerSegment,
    events: state.eventLedgerEvents,
    selectedOperationId: operationId,
    liveTurn: state.focusTurns.find((turn) => turn.operationId === operationId) ?? null,
    matchedDigest: state.recentDigests.find((item) => digestOperationId(item) === operationId) ?? null,
    matchedReasoningView: state.recentReasoningViews.find((item) => item.operation_id === operationId) ?? null,
    matchedClosures: state.recentClosures.filter((item) => item.operation_id === operationId),
    matchedToolRecords: state.recentToolRecords.filter((item) => item.operation_id === operationId),
  };
}

function digestOperationId(digest: JsonRecord): string | null {
  const digestId = scalar(digest.digest_id);
  return digestId.startsWith('digest-') ? digestId.slice('digest-'.length) : null;
}

function scalar(value: unknown): string {
  if (value === null || value === undefined) return '-';
  if (typeof value === 'string') return value || '-';
  return JSON.stringify(value);
}
