import {
  activityFocusSource,
  activityRecentItems,
  activitySourceCards,
  activityStage,
  activityState,
  activityStateTone,
  activityUserCard,
} from './activity_cards_ui.js';
import { formatLocalTimestamp } from './time.js';
import { StructuredTreeRenderer } from './tree.js';
import type {
  ClosureTraceRecord,
  ContextSnapshotRecord,
  DigestRecord,
  FocusTurn,
  JsonRecord,
  RefreshState,
  ReasoningViewRecord,
  RuntimeEvent,
  SessionMessage,
  ToolExecutionRecord,
} from './types.js';

export class FocusPane {
  constructor(
    private readonly rootEl: HTMLElement,
    private readonly tree: StructuredTreeRenderer,
  ) {}

  render(state: RefreshState): void {
    const selected = selectTurn(state);
    const frontstage = renderFrontstageSummary(state, this.tree);
    const frameworkTimeline = renderFrameworkTimeline(state, this.tree);
    if (!selected) {
      this.rootEl.innerHTML = `
        ${frontstage}
        ${frameworkTimeline}
        <section class="selected-request empty">
          <div class="section-kicker">Selected Request</div>
          <h3>还没有可聚焦的请求</h3>
          <p class="muted">点击左侧任意消息后，这里会显示该轮 request 的摘要、provider 状态与 message 片段。</p>
        </section>
      `;
      return;
    }

    const requestPayload = asRecord(findEventPayload(selected.events, 'inference.started'));
    const providerAccepted = asRecord(findEventPayload(selected.events, 'provider.operation_accepted'));
    const providerCompleted = asRecord(
      findEventPayload(selected.events, 'provider.completed')
      ?? findEventPayload(selected.events, 'provider.gateway_response_received'),
    );
    const debug = asRecord(providerCompleted.debug ?? providerAccepted.debug);
    const userMessage = selected.userMessage?.content ?? requestPayload.input ?? '-';
    const assistantMessage = selected.assistantMessage?.content ?? providerCompleted.output_text ?? '-';

    const chips: Array<[string, unknown]> = [
      ['operation', selected.operationId],
      ['trace', selected.traceId ?? '-'],
      ['provider', providerCompleted.provider_name ?? providerAccepted.provider_name ?? requestPayload.provider ?? '-'],
      ['model', providerCompleted.model ?? providerAccepted.model ?? requestPayload.model ?? '-'],
      ['status', providerCompleted.status ?? '-'],
      ['finish', providerCompleted.stop_reason ?? '-'],
      ['headers', Object.keys(asRecord(debug.request_headers)).length || 0],
      ['captured', selected.contextSnapshot?.captured_at ?? selected.userMessage?.created_at ?? '-'],
    ];

    this.rootEl.innerHTML = `
      ${frontstage}
      ${frameworkTimeline}
      <section class="selected-request">
        <div class="selected-request-header">
          <div>
            <div class="section-kicker">Selected Request</div>
            <h3>${this.tree.escapeHtml(userMessageSnippet(userMessage))}</h3>
          </div>
          <span class="request-state-pill ${stateClass(providerCompleted.status)}">
            ${this.tree.escapeHtml(providerCompleted.stop_reason ? String(providerCompleted.stop_reason) : 'active')}
          </span>
        </div>
        <section class="chip-grid">
          ${chips.map(([label, value]) => this.renderChip(label, value)).join('')}
        </section>
        <section class="summary-columns">
          ${this.renderMessageSummaryBlock('User Message', 'user', userMessage, selected.userMessage?.created_at)}
          ${this.renderMessageSummaryBlock('Assistant Output', 'assistant', assistantMessage, selected.assistantMessage?.created_at)}
        </section>
      </section>
    `;
  }

  private renderChip(label: string, value: unknown): string {
    return `
      <article class="summary-chip">
        <span class="summary-chip-label">${this.tree.escapeHtml(label)}</span>
        <span class="summary-chip-value">${this.tree.escapeHtml(this.tree.prettyScalar(value))}</span>
      </article>
    `;
  }

  private renderMessageSummaryBlock(
    title: string,
    tone: 'user' | 'assistant',
    content: unknown,
    timestamp?: string,
  ): string {
    return `
      <article class="summary-block ${tone}">
        <div class="summary-block-header">
          <span>${this.tree.escapeHtml(title)}</span>
          <span class="muted">${this.tree.escapeHtml(formatLocalTimestamp(timestamp))}</span>
        </div>
        <div class="summary-block-body">${this.tree.escapeHtml(String(content ?? '-'))}</div>
      </article>
    `;
  }
}


function renderFrontstageSummary(state: RefreshState, tree: StructuredTreeRenderer): string {
  const userCard = activityUserCard(state.activityCards);
  const focusSource = activityFocusSource(state.activityCards);
  const sources = activitySourceCards(state.activityCards);
  const recentItems = activityRecentItems(state.activityCards, 3);
  if (!userCard && !focusSource && !sources.length) return '';

  const stateLabel = activityState(state.activityCards);
  const tone = activityStateTone(stateLabel);
  const stage = activityStage(state.activityCards);

  return `
    <section class="selected-request frontstage-summary-block ${tree.escapeHtml(tone)}">
      <div class="selected-request-header">
        <div>
          <div class="section-kicker">Frontstage Summary</div>
          <h3>${tree.escapeHtml(userCard?.header ?? focusSource?.summary ?? 'system frontstage · idle')}</h3>
        </div>
        <span class="request-state-pill ${tree.escapeHtml(tone === 'failed' ? 'error' : tone === 'running' ? 'ok' : 'neutral')}">
          ${tree.escapeHtml(stateLabel)}
        </span>
      </div>
      <section class="chip-grid">
        <article class="summary-chip">
          <span class="summary-chip-label">focus</span>
          <span class="summary-chip-value">${tree.escapeHtml(focusSource?.title ?? userCard?.focus_source_id ?? 'system-agent')}</span>
        </article>
        <article class="summary-chip">
          <span class="summary-chip-label">stage</span>
          <span class="summary-chip-value">${tree.escapeHtml(stage)}</span>
        </article>
        <article class="summary-chip">
          <span class="summary-chip-label">sources</span>
          <span class="summary-chip-value">${tree.escapeHtml(String(sources.length || 1))}</span>
        </article>
      </section>
      ${recentItems.length ? `
        <section class="summary-columns">
          <article class="summary-block assistant">
            <div class="summary-block-header">
              <span>Recent Activity</span>
              <span class="muted">activity cards</span>
            </div>
            <div class="summary-block-body">${tree.escapeHtml(recentItems.join('\n'))}</div>
          </article>
        </section>
      ` : ''}
    </section>
  `;
}

function renderFrameworkTimeline(state: RefreshState, tree: StructuredTreeRenderer): string {
  const items = state.sessionEvents
    .filter(isFrameworkProgressEvent)
    .slice(-8)
    .reverse();
  if (!items.length) return '';

  return `
    <section class="selected-request">
      <div class="selected-request-header">
        <div>
          <div class="section-kicker">Framework Progress</div>
          <h3>formalize / planning / scheduler / supervisor timeline</h3>
        </div>
        <span class="request-state-pill neutral">${tree.escapeHtml(String(items.length))} events</span>
      </div>
      <div class="timeline-list">
        ${items.map((event) => `
          <article class="timeline-item ${frameworkEventTone(event)}">
            <div class="timeline-item-header">
              <span class="timeline-name">${tree.escapeHtml(String(event.event_type ?? '-'))}</span>
              <span class="timeline-time">${tree.escapeHtml(formatLocalTimestamp(event.occurred_at ?? event.timestamp))}</span>
            </div>
            <div class="timeline-meta">
              <span>source=${tree.escapeHtml(String(event.source ?? '-'))}</span>
              <span>${tree.escapeHtml(frameworkEventSummary(event))}</span>
            </div>
          </article>
        `).join('')}
      </div>
    </section>
  `;
}

export function buildFocusTurns(
  messages: SessionMessage[],
  recentContexts: ContextSnapshotRecord[],
  recentDigests: DigestRecord[],
  recentReasoningViews: ReasoningViewRecord[],
  recentToolRecords: ToolExecutionRecord[],
  recentClosures: ClosureTraceRecord[],
  sessionEvents: RuntimeEvent[],
): FocusTurn[] {
  const turns = new Map<string, FocusTurn>();
  const contextsByOperation = new Map(recentContexts.map((item) => [String(item.operation_id ?? ''), item]));
  const digestEntries: Array<[string, DigestRecord]> = recentDigests
    .map((item): [string, DigestRecord] => [digestOperationId(item), item])
    .filter(([operationId]) => operationId.length > 0);
  const digestsByOperation = new Map<string, DigestRecord>(digestEntries);
  const reasoningByOperation = new Map<string, ReasoningViewRecord>(
    recentReasoningViews
      .map((item): [string, ReasoningViewRecord] => [String(item.operation_id ?? ''), item])
      .filter(([operationId]) => operationId.length > 0),
  );
  const closuresByOperation = new Map<string, ClosureTraceRecord>(
    recentClosures
      .map((item): [string, ClosureTraceRecord] => [String(item.operation_id ?? ''), item])
      .filter(([operationId]) => operationId.length > 0),
  );

  for (const event of sessionEvents) {
    const operationId = event.operation_id ?? '';
    if (!operationId) continue;
    const turn = ensureTurn(turns, operationId);
    turn.traceId ??= event.trace_id;
    turn.events.push(event);
  }

  for (const message of messages) {
    const operationId = message.operation_id ?? inferOperationId(message.message_id);
    if (!operationId) continue;
    const turn = ensureTurn(turns, operationId);
    turn.traceId ??= message.trace_id;
    if (message.role === 'user') {
      turn.userMessage = message;
    } else {
      turn.assistantMessage = message;
    }
  }

  for (const [operationId, turn] of turns) {
    turn.contextSnapshot = contextsByOperation.get(operationId);
    turn.digest = digestsByOperation.get(operationId);
    turn.reasoningView = reasoningByOperation.get(operationId);
    turn.closureTrace = closuresByOperation.get(operationId);
    turn.toolRecords = recentToolRecords.filter((item) => item.operation_id === operationId);
    turn.events.sort((left, right) => (left.sequence ?? 0) - (right.sequence ?? 0));
  }

  return Array.from(turns.values()).sort((left, right) => {
    const leftTime = messageTime(left.userMessage, left.contextSnapshot);
    const rightTime = messageTime(right.userMessage, right.contextSnapshot);
    return leftTime.localeCompare(rightTime);
  });
}

function selectTurn(state: RefreshState): FocusTurn | undefined {
  return state.focusTurns.find((turn) => turn.operationId === state.selectedOperationId);
}

function ensureTurn(turns: Map<string, FocusTurn>, operationId: string): FocusTurn {
  const existing = turns.get(operationId);
  if (existing) return existing;

  const created: FocusTurn = {
    operationId,
    toolRecords: [],
    events: [],
  };
  turns.set(operationId, created);
  return created;
}

function inferOperationId(messageId?: string): string | null {
  if (!messageId) return null;
  if (messageId.startsWith('user-')) return messageId.slice('user-'.length);
  if (messageId.startsWith('assistant-closure-')) return messageId.slice('assistant-closure-'.length);
  return null;
}

function digestOperationId(digest: DigestRecord): string {
  const digestId = String(digest.digest_id ?? '');
  return digestId.startsWith('digest-') ? digestId.slice('digest-'.length) : '';
}

function messageTime(message?: SessionMessage, context?: ContextSnapshotRecord): string {
  return String(message?.created_at ?? context?.captured_at ?? '');
}

function findEventPayload(events: RuntimeEvent[], eventType: string): JsonRecord | undefined {
  const event = events.find((item) => item.event_type === eventType);
  return event ? asRecord(event.payload) : undefined;
}

function asRecord(value: unknown): JsonRecord {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
  return value as JsonRecord;
}

function stateClass(status: unknown): string {
  if (typeof status === 'number' && status >= 200 && status < 300) return 'ok';
  if (typeof status === 'number' && status >= 400) return 'error';
  return 'neutral';
}

function userMessageSnippet(content: unknown): string {
  const text = String(content ?? '-').trim();
  if (text.length <= 72) return text || '-';
  return `${text.slice(0, 72)}…`;
}

function isFrameworkProgressEvent(event: RuntimeEvent): boolean {
  const eventType = String(event.event_type ?? '');
  const source = String(event.source ?? '');
  return (
    eventType === 'session.formalized'
    || eventType === 'framework.task_kickoff_enqueued'
    || eventType.startsWith('scheduler.tick_')
    || eventType.startsWith('supervisor.cycle_')
    || source === 'cli.session_routing'
    || source === 'cli.scheduler_tick'
    || source === 'cli.supervisor_cycle'
  );
}

function frameworkEventSummary(event: RuntimeEvent): string {
  const payload = asRecord(event.payload);
  const eventType = String(event.event_type ?? '');
  if (eventType === 'session.formalized') {
    return compactSummary(
      `task=${String(payload.task_id ?? '-')} · topic=${String(payload.topic_thread_id ?? '-')}`,
    );
  }
  if (eventType === 'framework.task_kickoff_enqueued') {
    return compactSummary(
      `queued planning kickoff · ${String(payload.goal_summary ?? payload.enqueue_reason ?? '-')}`,
    );
  }
  if (eventType === 'scheduler.tick_decision_recorded') {
    return compactSummary(
      `action=${String(payload.action_kind ?? '-')} · pending=${String(payload.pending_input_count ?? '-')}`,
    );
  }
  if (eventType === 'scheduler.tick_completed') {
    return compactSummary(
      `final=${String(payload.final_action_kind ?? '-')} · drove=${String(payload.drove_count ?? '-')}`,
    );
  }
  if (eventType === 'supervisor.cycle_completed') {
    return compactSummary(
      `blocked=${String(payload.blocked_kind ?? '-')} · next=${String(payload.next_wake_hint ?? '-')}`,
    );
  }
  return compactSummary(
    String(
      payload.result_summary
      ?? payload.reason
      ?? payload.source
      ?? payload.goal_summary
      ?? '-',
    ),
  );
}

function frameworkEventTone(event: RuntimeEvent): string {
  const eventType = String(event.event_type ?? '');
  if (eventType.includes('blocked')) return 'error';
  if (
    eventType === 'session.formalized'
    || eventType === 'framework.task_kickoff_enqueued'
    || eventType.endsWith('_completed')
  ) {
    return 'ok';
  }
  return 'subtle';
}

function compactSummary(input: string): string {
  const text = input.trim();
  if (text.length <= 120) return text || '-';
  return `${text.slice(0, 120)}…`;
}
