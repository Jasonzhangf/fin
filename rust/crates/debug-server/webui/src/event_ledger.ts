import { renderEventLedgerLinks } from './event_ledger_links.js';
import { renderEventLedgerOperationSummary } from './event_ledger_summary.js';
import { StructuredTreeRenderer } from './tree.js';
import type { EventArchiveSegmentRef, EventLedgerScope, EventLedgerView, RuntimeEvent } from './types.js';

export function renderEventLedger(
  tree: StructuredTreeRenderer,
  ledger: EventLedgerView,
): string {
  if (!ledger.index) return '';
  return `
    <section class="event-ledger-panel">
      <div class="event-ledger-header">
        <div class="detail-subtitle">Event Ledger Scope</div>
        <div class="event-ledger-scope-row">
          ${renderLedgerScopeButton(tree, 'live', 'Live', ledger.scope === 'live')}
          ${renderLedgerScopeButton(tree, 'local_archive', `Local (${segmentCount(ledger.index.local_segments)})`, ledger.scope === 'local_archive')}
          ${renderLedgerScopeButton(tree, 'cold_archive', `Cold (${segmentCount(ledger.index.cold_segments)})`, ledger.scope === 'cold_archive')}
        </div>
      </div>
      <div class="event-ledger-grid">
        ${renderLedgerSegmentColumn(tree, 'local_archive', 'Local archive segments', ledger.index.local_segments, ledger.segment)}
        ${renderLedgerSegmentColumn(tree, 'cold_archive', 'Cold archive segments', ledger.index.cold_segments, ledger.segment)}
      </div>
      ${renderLedgerOperations(tree, ledger)}
      ${renderEventLedgerOperationSummary(tree, ledger.events, ledger.selectedOperationId)}
      ${renderEventLedgerLinks(tree, ledger)}
    </section>
  `;
}

function renderLedgerScopeButton(
  tree: StructuredTreeRenderer,
  scope: EventLedgerScope,
  title: string,
  active: boolean,
): string {
  return `<button class="event-ledger-btn ${active ? 'active' : ''}" type="button" data-event-ledger-scope="${tree.escapeHtml(scope)}">${tree.escapeHtml(title)}</button>`;
}

function renderLedgerSegmentColumn(
  tree: StructuredTreeRenderer,
  scope: Extract<EventLedgerScope, 'local_archive' | 'cold_archive'>,
  title: string,
  entries: EventArchiveSegmentRef[] | undefined,
  activeSegment: string | null,
): string {
  const items = Array.isArray(entries) ? entries : [];
  return `
    <section class="event-ledger-column">
      <div class="detail-subtitle">${tree.escapeHtml(title)}</div>
      <div class="event-ledger-segment-list">
        ${items.length ? items.map((entry) => `
          <button class="event-ledger-segment ${entry.segment === activeSegment ? 'active' : ''}" type="button" data-event-ledger-tier="${tree.escapeHtml(scope)}" data-event-ledger-segment="${tree.escapeHtml(String(entry.segment ?? ''))}">
            <span>${tree.escapeHtml(String(entry.segment ?? '-'))}</span>
            <span class="muted">${tree.escapeHtml(`${scalar(entry.event_count)} events`)}</span>
          </button>
        `).join('') : `<div class="empty-state compact"><div class="empty-text">No segments</div></div>`}
      </div>
    </section>
  `;
}

function scalar(value: unknown): string {
  if (value === null || value === undefined) return '-';
  if (typeof value === 'string') return value || '-';
  return JSON.stringify(value);
}

function segmentCount(value: unknown): number {
  return Array.isArray(value) ? value.length : 0;
}

function renderLedgerOperations(
  tree: StructuredTreeRenderer,
  ledger: EventLedgerView,
): string {
  const operations = groupLedgerOperations(ledger.events);
  return `
    <section class="event-ledger-operations">
      <div class="detail-subtitle">Operations in current ledger scope</div>
      <div class="event-ledger-operation-list">
        ${operations.length ? operations.map((operation) => `
          <button class="event-ledger-operation ${ledger.selectedOperationId === operation.operationId ? 'active' : ''}" type="button" data-event-ledger-operation="${tree.escapeHtml(operation.operationId)}">
            <span>${tree.escapeHtml(operation.operationId)}</span>
            <span class="muted">${tree.escapeHtml(`${operation.eventCount} events · ${operation.lastEvent}`)}</span>
          </button>
        `).join('') : `<div class="empty-state compact"><div class="empty-text">No operation ids in this scope</div></div>`}
      </div>
    </section>
  `;
}

function groupLedgerOperations(events: RuntimeEvent[]): Array<{
  operationId: string;
  eventCount: number;
  lastEvent: string;
}> {
  const groups = new Map<string, { eventCount: number; lastSequence: number; lastEvent: string }>();
  for (const event of events) {
    const operationId = event.operation_id?.trim();
    if (!operationId) continue;
    const existing = groups.get(operationId) ?? { eventCount: 0, lastSequence: -1, lastEvent: '-' };
    existing.eventCount += 1;
    const sequence = typeof event.sequence === 'number' ? event.sequence : -1;
    if (sequence >= existing.lastSequence) {
      existing.lastSequence = sequence;
      existing.lastEvent = String(event.event_type ?? '-');
    }
    groups.set(operationId, existing);
  }
  return Array.from(groups.entries())
    .map(([operationId, value]) => ({
      operationId,
      eventCount: value.eventCount,
      lastEvent: value.lastEvent,
      lastSequence: value.lastSequence,
    }))
    .sort((left, right) => right.lastSequence - left.lastSequence)
    .map(({ operationId, eventCount, lastEvent }) => ({ operationId, eventCount, lastEvent }));
}
