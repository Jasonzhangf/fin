import type { EventArchiveIndex, EventLedgerScope, RuntimeEvent } from './types.js';

export interface ResolvedEventLedgerState {
  scope: EventLedgerScope;
  segment: string | null;
  selectedOperationId: string | null;
}

export function resolveEventLedgerState(
  index: EventArchiveIndex | null,
  scope: EventLedgerScope,
  segment: string | null,
  events: RuntimeEvent[],
  requestedSelectedOperationId: string | null,
  selectedOperationId: string | null,
): ResolvedEventLedgerState {
  const nextScope = hasLedgerSegment(index, scope, segment) ? scope : 'live';
  const nextSegment = nextScope === scope ? segment : null;
  const nextSelectedOperationId = hasLedgerOperation(events, requestedSelectedOperationId)
    ? requestedSelectedOperationId
    : defaultLedgerOperationId(nextScope, events, selectedOperationId);
  return {
    scope: nextScope,
    segment: nextSegment,
    selectedOperationId: nextSelectedOperationId,
  };
}

function hasLedgerSegment(
  index: EventArchiveIndex | null,
  scope: EventLedgerScope,
  segment: string | null,
): boolean {
  if (scope === 'live') return true;
  if (!index || !segment) return false;
  const entries = scope === 'local_archive' ? index.local_segments : index.cold_segments;
  return Array.isArray(entries) && entries.some((item) => item.segment === segment);
}

function hasLedgerOperation(events: RuntimeEvent[], operationId: string | null): boolean {
  if (!operationId) return false;
  return events.some((event) => event.operation_id === operationId);
}

function defaultLedgerOperationId(
  scope: EventLedgerScope,
  events: RuntimeEvent[],
  selectedOperationId: string | null,
): string | null {
  if (scope === 'live') return selectedOperationId;
  for (const event of events) {
    const operationId = event.operation_id?.trim();
    if (operationId) return operationId;
  }
  return null;
}
