import { StructuredTreeRenderer } from './tree.js';
import type { JsonRecord, RuntimeEvent } from './types.js';

export function renderEventLedgerOperationSummary(
  tree: StructuredTreeRenderer,
  events: RuntimeEvent[],
  selectedOperationId: string | null,
): string {
  if (!selectedOperationId) {
    return renderEmpty(tree, 'Select an operation to inspect reconstructed summary');
  }
  const scopedEvents = events.filter((event) => event.operation_id === selectedOperationId);
  if (!scopedEvents.length) {
    return renderEmpty(tree, 'No events for selected operation');
  }
  const request = payload(findFirstEvent(scopedEvents, 'inference.started'));
  const providerAccepted = payload(findLatestEvent(scopedEvents, 'provider.operation_accepted'));
  const providerCompleted = payload(findLatestEvent(scopedEvents, 'provider.completed'))
    || payload(findLatestEvent(scopedEvents, 'provider.gateway_response_received'));
  const control = payload(findLatestEvent(scopedEvents, 'control.feedback_recorded'));
  const note = payload(findLatestEvent(scopedEvents, 'execution_note.appended'));
  const digest = payload(findLatestEvent(scopedEvents, 'digest.finalized'));
  const reasoning = payload(findLatestEvent(scopedEvents, 'reasoning.view_recorded'));
  const completed = payload(findLatestEvent(scopedEvents, 'operation.completed'));
  const tools = payload(findLatestEvent(scopedEvents, 'tool.execution_recorded'));
  return `
    <section class="event-ledger-summary">
      <div class="detail-subtitle">Archive Operation Summary</div>
      <div class="event-ledger-summary-grid">
        ${summaryItem(tree, 'operation', selectedOperationId)}
        ${summaryItem(tree, 'trace', scalar(scopedEvents[0]?.trace_id))}
        ${summaryItem(tree, 'provider', `${scalar(providerCompleted.provider_name || providerAccepted.provider_name || request.provider)} / ${scalar(providerCompleted.model || providerAccepted.model || request.model)}`)}
        ${summaryItem(tree, 'status', `${scalar(completed.status)} · stop=${scalar(completed.stop_source)} · finish=${scalar(providerCompleted.stop_reason)} · http=${scalar(providerCompleted.status)}`)}
        ${summaryItem(tree, 'control', `${scalar(control.origin)} · topic=${scalar(control.topic_shift_confidence)} · simple=${scalar(control.simple_query_confidence)}`)}
        ${summaryItem(tree, 'digest', `${scalar(digest.digest_id)} · ${shortText(scalar(digest.summary), 100)}`)}
      </div>
      ${textPanel(tree, 'Request Input', scalar(request.input))}
      ${textPanel(tree, 'Execution Note', scalar(note.summary))}
      ${textPanel(tree, 'Reasoning Summary', scalar(reasoning.summary))}
      ${textPanel(tree, 'Assistant / Provider Output', shortText(scalar(completed.answer || providerCompleted.output_text), 600))}
      ${toolPanel(tree, tools)}
    </section>
  `;
}

function findFirstEvent(events: RuntimeEvent[], eventType: string): RuntimeEvent | undefined {
  return events.find((event) => event.event_type === eventType);
}

function findLatestEvent(events: RuntimeEvent[], eventType: string): RuntimeEvent | undefined {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    if (events[index]?.event_type === eventType) return events[index];
  }
  return undefined;
}

function payload(event?: RuntimeEvent): JsonRecord {
  const value = event?.payload;
  return value && typeof value === 'object' && !Array.isArray(value) ? value as JsonRecord : {};
}

function summaryItem(tree: StructuredTreeRenderer, label: string, value: string): string {
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

function toolPanel(tree: StructuredTreeRenderer, value: unknown): string {
  const items = Array.isArray(value) ? value : [];
  if (!items.length) return '';
  const summary = items
    .map((item) => {
      const record = (item && typeof item === 'object' && !Array.isArray(item)) ? item as JsonRecord : {};
      return `${scalar(record.tool_name)}:${scalar(record.status)}`;
    })
    .join(' | ');
  return textPanel(tree, 'Tool Summary', summary);
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
