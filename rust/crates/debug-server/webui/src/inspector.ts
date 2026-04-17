import { formatLocalTimestamp } from './time.js';
import { StructuredTreeRenderer } from './tree.js';
import type { DashboardCardId, FocusTurn, JsonRecord, RefreshState, RuntimeEvent } from './types.js';

interface CardSpec {
  id: DashboardCardId;
  title: string;
  subtitle: string;
  digestLines: Array<[string, string]>;
  focusAreas: string[];
  sections: Array<[string, unknown]>;
  timelineGroups?: Array<[string, RuntimeEvent[]]>;
}

export class InspectorPane {
  constructor(
    private readonly rootEl: HTMLElement,
    private readonly tree: StructuredTreeRenderer,
  ) {}

  render(state: RefreshState): void {
    const selected = state.focusTurns.find((turn) => turn.operationId === state.selectedOperationId);
    const cards = this.buildCards(selected, state);
    const opened = state.openedCard ? cards.find((item) => item.id === state.openedCard) ?? null : null;

    this.rootEl.innerHTML = `
      <section class="dashboard-shell">
        <div class="dashboard-waterfall">
          ${cards.map((card) => this.renderDigestCard(card)).join('')}
        </div>
        ${opened ? this.renderModal(opened) : ''}
      </section>
    `;
  }

  private buildCards(selected: FocusTurn | undefined, state: RefreshState): CardSpec[] {
    const providerAccepted = asRecord(findEvent(selected?.events ?? [], 'provider.operation_accepted')?.payload);
    const providerResponse = asRecord(
      findEvent(selected?.events ?? [], 'provider.completed')?.payload
      ?? findEvent(selected?.events ?? [], 'provider.gateway_response_received')?.payload,
    );
    const providerDebug = asRecord(providerResponse.debug ?? providerAccepted.debug);
    const context = selected?.contextSnapshot;
    const request = asRecord(findEvent(selected?.events ?? [], 'inference.started')?.payload);
    const latestEventNames = (selected?.events ?? []).slice(-4).map((event) => String(event.event_type ?? '-')).join(' → ');
    const currentContext = asRecord(context?.context);
    const continuityTail = Array.isArray(currentContext.continuity_tail)
      ? currentContext.continuity_tail.map((item) => String(item)).join(' | ')
      : '-';

    return [
      {
        id: 'provider',
        title: 'Provider',
        subtitle: '最新 provider 更新摘要',
        digestLines: [
          ['target', `${providerResponse.provider_name ?? providerAccepted.provider_name ?? '-'} / ${providerResponse.model ?? providerAccepted.model ?? request.model ?? '-'}`],
          ['result', `status=${scalar(providerResponse.status)} · finish=${scalar(providerResponse.stop_reason)} · response=${shortText(scalar(providerResponse.response_id), 48)}`],
          ['output', shortText(scalar(providerResponse.output_text), 180)],
        ],
        focusAreas: ['Request', 'Response', 'Sanitized Debug', 'Projection Summary'],
        sections: [
          ['Request', providerAccepted],
          ['Response', providerResponse],
          ['Sanitized Debug', providerDebug],
          ['Projection Summary', {
            selected_operation_id: selected?.operationId ?? '-',
            trace_id: selected?.traceId ?? '-',
            request_header_names: Object.keys(asRecord(providerDebug.request_headers)),
            recent_event_names: (selected?.events ?? []).map((event) => event.event_type ?? '-'),
          }],
        ],
      },
      {
        id: 'context',
        title: 'Context',
        subtitle: '当前请求上下文摘要',
        digestLines: [
          ['input', shortText(scalar(context?.input ?? selected?.userMessage?.content ?? '-'), 160)],
          ['continuity', shortText(continuityTail, 180)],
          ['profile', `role=${shortText(scalar(context?.role), 48)} · protocol=${scalar(context?.protocol_version)} · stream=${scalar(context?.stream)}`],
        ],
        focusAreas: ['Context View', 'Context Meta'],
        sections: [
          ['Context View', currentContext],
          ['Context Meta', {
            input: context?.input ?? '-',
            role: context?.role ?? '-',
            provider_path: context?.provider_path ?? '-',
            provider_strategy: context?.provider_strategy ?? '-',
            protocol_version: context?.protocol_version ?? '-',
            stream: context?.stream ?? '-',
            captured_at: context?.captured_at ?? '-',
          }],
        ],
      },
      {
        id: 'system',
        title: 'System',
        subtitle: 'project / session / runtime 摘要',
        digestLines: [
          ['scope', `${state.binding?.project_label ?? 'fin'} · session=${state.binding?.session_id ?? '-'} · task=${state.binding?.task_id ?? state.projection.task_id ?? '-'}`],
          ['runtime', shortText(state.binding?.runtime_home ?? '-', 120)],
          ['artifacts', `messages=${shortPath(state.binding?.session_messages_path)} · context=${shortPath(state.binding?.recent_contexts_path)} · digest=${shortPath(state.binding?.recent_digests_path)}`],
        ],
        focusAreas: ['Binding', 'Projection', 'Paths', 'Selected Refs'],
        sections: [
          ['Binding', state.binding ?? {}],
          ['Projection', { note: 'UI render now consumes session artifacts as truth.', recent_contexts_count: state.recentContexts.length, recent_digests_count: state.recentDigests.length, recent_events_count: state.sessionEvents.length, recent_messages_count: state.messages.length }],
          ['Paths', {
            runtime_home: state.binding?.runtime_home ?? '-',
            session_messages_path: state.binding?.session_messages_path ?? '-',
            recent_contexts_path: state.binding?.recent_contexts_path ?? '-',
            recent_digests_path: state.binding?.recent_digests_path ?? '-',
          }],
          ['Selected Refs', {
            operation_id: selected?.operationId ?? '-',
            trace_id: selected?.traceId ?? '-',
            session_id: selected?.userMessage?.session_id ?? context?.session_id ?? state.binding?.session_id ?? '-',
            task_id: selected?.userMessage?.task_id ?? context?.task_id ?? state.binding?.task_id ?? '-',
          }],
        ],
      },
      {
        id: 'operation',
        title: 'Operation & Event',
        subtitle: '消息 / 请求 / 事件链摘要',
        digestLines: [
          ['operation', `${selected?.operationId ?? '-'} · trace=${selected?.traceId ?? '-'}`],
          ['timeline', shortText(latestEventNames || '-', 180)],
          ['closure', shortText(scalar(selected?.digest?.summary ?? selected?.assistantMessage?.content ?? '-'), 180)],
        ],
        focusAreas: ['Turn Messages', 'Request Structure', 'Selected Timeline', 'Recent Session Timeline', 'Digest'],
        sections: [
          ['Turn Messages', { user: selected?.userMessage ?? {}, assistant: selected?.assistantMessage ?? {} }],
          ['Request Structure', request],
          ['Selected Timeline', { events: selected?.events ?? [] }],
          ['Recent Session Timeline', { events: state.sessionEvents.slice(-8) }],
          ['Digest', selected?.digest ?? {}],
        ],
        timelineGroups: [
          ['Selected Request Timeline', selected?.events ?? []],
          ['Recent Session Timeline', state.sessionEvents.slice(-8)],
        ],
      },
    ];
  }

  private renderDigestCard(card: CardSpec): string {
    return `
      <button class="dashboard-digest-card" type="button" data-open-card="${this.tree.escapeHtml(card.id)}">
        <div class="dashboard-digest-header">
          <div>
            <h3>${this.tree.escapeHtml(card.title)}</h3>
            <p class="muted">${this.tree.escapeHtml(card.subtitle)}</p>
          </div>
          <span class="dashboard-digest-open">View</span>
        </div>
        <div class="dashboard-digest-lines">
          ${card.digestLines.map(([label, value]) => `
            <article class="digest-line">
              <span class="digest-line-label">${this.tree.escapeHtml(label)}</span>
              <span class="digest-line-value">${this.tree.escapeHtml(value)}</span>
            </article>
          `).join('')}
        </div>
      </button>
    `;
  }

  private renderModal(card: CardSpec): string {
    return `
      <div class="detail-modal-backdrop" data-modal-backdrop="true">
        <section class="detail-modal">
          <header class="detail-modal-header">
            <div>
              <div class="section-kicker">Detail</div>
              <h3>${this.tree.escapeHtml(card.title)}</h3>
              <p class="muted">${this.tree.escapeHtml(card.subtitle)}</p>
            </div>
            <button class="detail-modal-close" type="button" data-close-modal="true">Close</button>
          </header>
          <div class="detail-focus-strip">
            ${card.focusAreas.map((area) => `
              <a class="detail-focus-chip" href="#${this.anchorId(card.id, area)}">${this.tree.escapeHtml(area)}</a>
            `).join('')}
          </div>
          <div class="detail-modal-body">
            ${card.timelineGroups ? `
              <div class="detail-timeline-grid">
                ${card.timelineGroups.map(([title, events]) => `
                  <section class="timeline-block">
                    <div class="detail-subtitle">${this.tree.escapeHtml(title)}</div>
                    ${this.renderEventTimeline(events)}
                  </section>
                `).join('')}
              </div>
            ` : ''}
            <div class="detail-section-stack">
              ${card.sections.map(([label, value], index) => `
                <details class="detail-section" ${index === 0 ? 'open' : ''} id="${this.anchorId(card.id, label)}">
                  <summary>${this.tree.escapeHtml(label)}</summary>
                  <div class="detail-section-body">${this.tree.renderNode(label.toLowerCase().replaceAll(' ', '_'), value, true)}</div>
                </details>
              `).join('')}
            </div>
          </div>
        </section>
      </div>
    `;
  }

  private renderEventTimeline(events: RuntimeEvent[]): string {
    if (!events.length) return this.empty('No events');
    return `
      <div class="timeline-list">
        ${events.map((event) => `
          <article class="timeline-item ${timelineLevel(event.event_type)}">
            <div class="timeline-item-header">
              <span class="timeline-seq">${this.tree.escapeHtml(String(event.sequence ?? '-'))}</span>
              <span class="timeline-name">${this.tree.escapeHtml(String(event.event_type ?? '-'))}</span>
              <span class="timeline-time">${this.tree.escapeHtml(formatLocalTimestamp(event.occurred_at ?? event.timestamp))}</span>
            </div>
            <div class="timeline-meta">
              <span>trace=${this.tree.escapeHtml(scalar(event.trace_id))}</span>
              <span>source=${this.tree.escapeHtml(scalar(event.source))}</span>
            </div>
          </article>
        `).join('')}
      </div>
    `;
  }

  private anchorId(cardId: DashboardCardId, label: string): string {
    return `${cardId}-${label.toLowerCase().replaceAll(/[^a-z0-9]+/g, '-')}`;
  }

  private empty(text: string): string {
    return `<div class="empty-state compact"><div class="empty-text">${this.tree.escapeHtml(text)}</div></div>`;
  }
}

function findEvent(events: RuntimeEvent[], eventType: string): RuntimeEvent | undefined {
  return events.find((event) => event.event_type === eventType);
}

function asRecord(value: unknown): JsonRecord {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
  return value as JsonRecord;
}

function scalar(value: unknown): string {
  if (value === null || value === undefined) return '-';
  if (typeof value === 'string') return value || '-';
  return JSON.stringify(value);
}

function shortText(value: string, limit: number): string {
  const text = value.trim();
  if (!text) return '-';
  return text.length <= limit ? text : `${text.slice(0, limit)}…`;
}

function shortPath(value: unknown): string {
  const text = scalar(value);
  if (text === '-') return text;
  const parts = text.split('/').filter(Boolean);
  return parts.length <= 3 ? text : `…/${parts.slice(-3).join('/')}`;
}

function timelineLevel(eventType?: string): string {
  if (!eventType) return 'neutral';
  if (eventType.includes('failed') || eventType.includes('timeout')) return 'error';
  if (eventType.includes('completed') || eventType.includes('finalized')) return 'ok';
  return 'neutral';
}
