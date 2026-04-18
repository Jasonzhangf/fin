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
    if (!selected) {
      this.rootEl.innerHTML = `
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
