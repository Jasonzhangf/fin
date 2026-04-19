import { StructuredTreeRenderer } from './tree.js';
import type { EventLedgerView } from './types.js';

export function renderEventLedgerLinks(
  tree: StructuredTreeRenderer,
  ledger: EventLedgerView,
): string {
  const selectedOperationId = ledger.selectedOperationId;
  if (!selectedOperationId) {
    return renderEmpty(tree, 'No archive operation selected for live/materialized links');
  }
  const matchedTools = ledger.matchedToolRecords ?? [];
  const matchedClosures = ledger.matchedClosures ?? [];
  return `
    <section class="event-ledger-links">
      <div class="detail-subtitle">Linked live / materialized records</div>
      <div class="event-ledger-summary-grid">
        ${summaryChip(tree, 'live turn', ledger.liveTurn ? 'available' : 'recent window miss')}
        ${summaryChip(tree, 'digest', ledger.matchedDigest?.digest_id ?? 'recent window miss')}
        ${summaryChip(tree, 'reasoning', ledger.matchedReasoningView?.reasoning_id ?? 'recent window miss')}
        ${summaryChip(tree, 'closure', matchedClosures.length ? matchedClosures[0]?.closure_id ?? 'available' : 'recent window miss')}
        ${summaryChip(tree, 'tool records', matchedTools.length ? String(matchedTools.length) : '0')}
        ${summaryChip(tree, 'operation', selectedOperationId)}
      </div>
      ${textPanel(tree, 'Live Turn Snippet', liveTurnSnippet(ledger))}
      ${textPanel(tree, 'Digest Summary', scalar(ledger.matchedDigest?.summary))}
      ${textPanel(tree, 'Reasoning Summary', scalar(ledger.matchedReasoningView?.summary))}
      ${textPanel(tree, 'Closure Summary', closureSummary(ledger))}
      ${textPanel(tree, 'Tool Record Summary', toolSummary(matchedTools))}
    </section>
  `;
}

function liveTurnSnippet(ledger: EventLedgerView): string {
  const turn = ledger.liveTurn;
  if (!turn) return '-';
  const user = scalar(turn.userMessage?.content);
  const assistant = scalar(turn.assistantMessage?.content);
  return `user=${shortText(user, 100)} | assistant=${shortText(assistant, 120)}`;
}

function closureSummary(ledger: EventLedgerView): string {
  const first = (ledger.matchedClosures ?? [])[0];
  if (!first) return '-';
  return `${scalar(first.closure_id)} · ${scalar(first.provider_name)} / ${scalar(first.provider_model)} · ${shortText(scalar(first.assistant_response), 140)}`;
}

function toolSummary(items: NonNullable<EventLedgerView['matchedToolRecords']>): string {
  if (!items.length) return '-';
  return items
    .slice(0, 8)
    .map((item) => `${scalar(item.tool_name)}:${scalar(item.status)}`)
    .join(' | ');
}

function summaryChip(tree: StructuredTreeRenderer, label: string, value: string): string {
  return `
    <article class="summary-chip">
      <span class="summary-chip-label">${tree.escapeHtml(label)}</span>
      <span class="summary-chip-value">${tree.escapeHtml(value)}</span>
    </article>
  `;
}

function textPanel(tree: StructuredTreeRenderer, title: string, text: string): string {
  if (text === '-') return '';
  return `
    <section class="semantic-panel">
      <div class="semantic-panel-title">${tree.escapeHtml(title)}</div>
      <div class="text-block compact">${tree.escapeHtml(text)}</div>
    </section>
  `;
}

function renderEmpty(tree: StructuredTreeRenderer, text: string): string {
  return `<div class="empty-state compact"><div class="empty-text">${tree.escapeHtml(text)}</div></div>`;
}

function scalar(value: unknown): string {
  if (value === null || value === undefined) return '-';
  if (typeof value === 'string') return value.trim() || '-';
  return JSON.stringify(value);
}

function shortText(value: string, limit: number): string {
  const text = value.trim();
  if (!text || text === '-') return '-';
  return text.length <= limit ? text : `${text.slice(0, limit)}…`;
}
