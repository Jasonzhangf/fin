import type {
  ConversationRichness,
  DebugBinding,
  FocusTurn,
  JsonRecord,
  SessionMessage,
  ToolExecutionRecord,
} from './types.js';
import { formatLocalTimestamp } from './time.js';
import { StructuredTreeRenderer } from './tree.js';

export interface PendingAssistantState {
  prompt: string;
  startedAtMs: number;
}

interface TurnCard {
  key: string;
  tone: 'reasoning' | 'meta' | 'tool';
  verb: string;
  title: string;
  body?: string;
  extra?: string;
  chips: string[];
  detailTitle: string;
  detailSubtitle: string;
  detailValue: unknown;
}

export class ChatPane {
  private lastRenderedSignature = '';

  constructor(
    private readonly messagesEl: HTMLElement,
    private readonly projectEl: HTMLElement,
    private readonly sessionEl: HTMLElement,
    private readonly taskEl: HTMLElement,
    private readonly modelEl: HTMLElement,
    private readonly contextSizeEl: HTMLElement,
    private readonly projectPathEl: HTMLElement,
    private readonly composerStatusEl: HTMLElement,
    private readonly composerContextEl: HTMLElement,
    private readonly tree: StructuredTreeRenderer,
  ) {}

  render(
    binding: DebugBinding | null,
    messages: SessionMessage[],
    focusTurns: FocusTurn[],
    selectedOperationId: string | null,
    richness: ConversationRichness,
    pendingAssistant: PendingAssistantState | null,
    lastRun: JsonRecord | null,
    currentContext: JsonRecord | null,
    openedChatDetailKey: string | null,
  ): void {
    const sessionLabel = binding?.session_id ?? 'tentative';
    const taskLabel = binding?.task_id ?? '-';
    const projectPath = resolveProjectPath(currentContext, binding);
    const contextSize = approximateContextSize(currentContext);
    const activeTurn = selectedOperationId ? focusTurns.find((turn) => turn.operationId === selectedOperationId) : undefined;

    this.projectEl.textContent = `project: ${binding?.project_label ?? 'fin'}`;
    this.sessionEl.textContent = `session: ${sessionLabel}`;
    this.taskEl.textContent = `task: ${taskLabel}`;
    this.modelEl.textContent = `model: ${scalar(lastRun?.model)}`;
    this.contextSizeEl.textContent = `context: ${contextSize}`;
    this.projectPathEl.textContent = `path: ${projectPath}`;
    this.modelEl.setAttribute('title', `model=${scalar(lastRun?.model)} provider=${scalar(lastRun?.provider)}`);
    this.contextSizeEl.setAttribute('title', `approx size of current_context.json = ${contextSize}`);
    this.projectPathEl.setAttribute('title', projectPath);
    this.composerStatusEl.textContent = `session truth · ${binding?.project_id ?? 'fin'} / ${sessionLabel} / ${richness}`;
    this.composerContextEl.innerHTML = renderComposerContextBar(
      binding,
      projectPath,
      contextSize,
      pendingAssistant,
      activeTurn,
      this.tree,
    );

    if (!messages.length) {
      this.messagesEl.innerHTML = `
        <div class="empty-state">
          <div class="empty-icon">💬</div>
          <div class="empty-text">No session messages yet. Send the first message into this bound session.</div>
        </div>
        ${this.renderChatDetailModal(focusTurns, openedChatDetailKey)}
      `;
      this.lastRenderedSignature = '';
      return;
    }

    const previousScrollTop = this.messagesEl.scrollTop;
    const wasNearBottom = isNearBottom(this.messagesEl);
    const signature = messages.map((message) => `${message.message_id}:${message.created_at}`).join('|');
    const focusByOperation = new Map(focusTurns.map((turn) => [turn.operationId, turn]));

    this.messagesEl.innerHTML = [
      ...messages.map((message) => this.renderMessage(message, focusByOperation, selectedOperationId, richness)),
      this.renderPendingAssistant(pendingAssistant),
      this.renderChatDetailModal(focusTurns, openedChatDetailKey),
    ].join('');

    if (signature !== this.lastRenderedSignature && wasNearBottom) {
      this.messagesEl.scrollTop = this.messagesEl.scrollHeight;
    } else {
      this.messagesEl.scrollTop = previousScrollTop;
    }
    this.lastRenderedSignature = signature;
  }

  private renderMessage(
    message: SessionMessage,
    focusByOperation: Map<string, FocusTurn>,
    selectedOperationId: string | null,
    richness: ConversationRichness,
  ): string {
    const role = message.role === 'user' ? 'user' : 'agent';
    const presentation = role === 'user'
      ? { avatar: '🧑', label: 'User', badge: 'user' }
      : {
          avatar: '🤖',
          label: message.role === 'assistant' || !message.role ? 'Assistant' : String(message.role),
          badge: message.role === 'assistant' || !message.role ? 'assistant' : 'agent',
        };
    const operationId = message.operation_id ?? inferOperationId(message.message_id);
    const focusTurn = operationId ? focusByOperation.get(operationId) : undefined;
    const selectedClass = operationId && operationId === selectedOperationId ? 'selected' : '';
    const turnLabel = operationId ? shortTurnLabel(operationId) : null;
    const richPanel = role === 'user' ? '' : this.renderRichPanel(focusTurn, richness);

    return `
      <article class="message ${role} ${selectedClass}" data-operation-id="${this.tree.escapeHtml(operationId ?? '')}">
        <div class="message-avatar">${presentation.avatar}</div>
        <div class="message-content-wrapper">
          <div class="message-header">
            <span class="role-badge ${presentation.badge}">${this.tree.escapeHtml(presentation.label)}</span>
            ${turnLabel ? `<span class="turn-badge">${this.tree.escapeHtml(turnLabel)}</span>` : ''}
            <span class="message-time">${this.tree.escapeHtml(formatLocalTimestamp(message.created_at))}</span>
          </div>
          <div class="message-body">${this.tree.escapeHtml(message.content ?? '')}</div>
          ${richPanel}
        </div>
      </article>
    `;
  }

  private renderRichPanel(focusTurn: FocusTurn | undefined, richness: ConversationRichness): string {
    if (!focusTurn || richness === 'minimal') return '';
    const cards = buildTurnCards(focusTurn);
    if (!cards.length) return '';
    return `
      <section class="message-rich-panel ${richness}">
        <div class="turn-card-stack">
          ${cards.map((card) => this.renderTurnCard(card)).join('')}
        </div>
      </section>
    `;
  }

  private renderTurnCard(card: TurnCard): string {
    return `
      <article
        class="turn-card ${card.tone} interactive ${!card.body && !card.extra ? 'compact' : ''}"
        data-open-chat-detail="${this.tree.escapeHtml(card.key)}"
      >
        <div class="turn-card-header">
          <span class="turn-card-verb">${this.tree.escapeHtml(card.verb)}</span>
          <span class="turn-card-title">${this.tree.escapeHtml(card.title)}</span>
          <span class="turn-card-open">detail</span>
        </div>
        ${card.chips.length ? `<div class="turn-card-chip-row">${card.chips.map((chip) => `<span class="turn-card-chip">${this.tree.escapeHtml(chip)}</span>`).join('')}</div>` : ''}
        ${card.body ? `<div class="turn-card-body">${this.tree.escapeHtml(card.body)}</div>` : ''}
        ${card.extra ? `<div class="turn-card-extra">${this.tree.escapeHtml(card.extra)}</div>` : ''}
      </article>
    `;
  }

  private renderPendingAssistant(pendingAssistant: PendingAssistantState | null): string {
    if (!pendingAssistant) return '';
    const elapsed = formatElapsed(Date.now() - pendingAssistant.startedAtMs);
    return `
      <article class="message agent pending" data-operation-id="">
        <div class="message-avatar">🤖</div>
        <div class="message-content-wrapper">
          <div class="message-header">
            <span class="role-badge assistant">Assistant</span>
            <span class="turn-badge waiting">waiting</span>
            <span class="message-time">elapsed ${this.tree.escapeHtml(elapsed)}</span>
          </div>
          <div class="message-body pending-body">
            <span class="pending-copy">Thinking on: ${this.tree.escapeHtml(pendingAssistant.prompt)}</span>
            <span class="typing-dots" aria-label="assistant busy"><span></span><span></span><span></span></span>
          </div>
        </div>
      </article>
    `;
  }

  private renderChatDetailModal(focusTurns: FocusTurn[], openedChatDetailKey: string | null): string {
    if (!openedChatDetailKey) return '';
    const match = resolveChatDetail(focusTurns, openedChatDetailKey);
    if (!match) return '';

    return `
      <div class="chat-detail-backdrop" data-chat-detail-backdrop>
        <section class="chat-detail-modal" role="dialog" aria-modal="true">
          <header class="chat-detail-header">
            <div>
              <div class="section-kicker">Conversation Detail</div>
              <h3>${this.tree.escapeHtml(match.detailTitle)}</h3>
              <p class="muted">${this.tree.escapeHtml(match.detailSubtitle)}</p>
            </div>
            <button class="chat-detail-close" type="button" data-close-chat-detail>Close</button>
          </header>
          <div class="chat-detail-body">
            ${this.tree.renderPanelValue(match.detailValue)}
          </div>
        </section>
      </div>
    `;
  }
}

function buildTurnCards(focusTurn: FocusTurn): TurnCard[] {
  const reasoning = asRecord(focusTurn.reasoningView);
  const control = extractControlFeedback(focusTurn);
  const provider = extractProviderMeta(focusTurn);
  const cards: TurnCard[] = [];
  const reasoningSummary = scalar(reasoning.summary);

  if (reasoningSummary !== '-') {
    cards.push({
      key: detailKey(focusTurn.operationId, 'reasoning'),
      tone: 'reasoning',
      verb: 'Reasoning',
      title: shortText(reasoningSummary, 220),
      body: undefined,
      extra: undefined,
      chips: [],
      detailTitle: 'Reasoning View',
      detailSubtitle: `turn ${shortTurnLabel(focusTurn.operationId)}`,
      detailValue: reasoning,
    });
  }

  for (const [index, tool] of (focusTurn.toolRecords ?? []).entries()) {
    cards.push({
      key: detailKey(focusTurn.operationId, `tool:${tool.tool_call_id ?? index}`),
      tone: 'tool',
      verb: toolVerb(tool),
      title: scalar(tool.title ?? tool.tool_name),
      chips: compactToolChips(tool),
      body: shortText(scalar(tool.purpose ?? tool.output_summary), 180),
      extra: optionalShortText(joinNonEmpty([scalar(tool.input_summary), scalar(tool.output_summary)], ' → '), 180),
      detailTitle: scalar(tool.title ?? tool.tool_name),
      detailSubtitle: `${toolVerb(tool)} · turn ${shortTurnLabel(focusTurn.operationId)}`,
      detailValue: tool,
    });
  }

  const metaChips = [...compactControlChips(control), ...compactProviderChips(provider)];
  if (provider.title !== '-' || metaChips.length) {
    cards.push({
      key: detailKey(focusTurn.operationId, 'meta'),
      tone: 'meta',
      verb: 'Meta',
      title: provider.title !== '-' ? provider.title : 'Provider / control summary',
      body: undefined,
      extra: undefined,
      chips: metaChips.slice(0, 4),
      detailTitle: 'Provider + Control Summary',
      detailSubtitle: `turn ${shortTurnLabel(focusTurn.operationId)}`,
      detailValue: { provider, control },
    });
  }

  return cards;
}

function resolveChatDetail(focusTurns: FocusTurn[], key: string): TurnCard | null {
  for (const turn of focusTurns) {
    const match = buildTurnCards(turn).find((card) => card.key === key);
    if (match) return match;
  }
  return null;
}

function renderComposerContextBar(
  binding: DebugBinding | null,
  projectPath: string,
  contextSize: string,
  pendingAssistant: PendingAssistantState | null,
  activeTurn: FocusTurn | undefined,
  tree: StructuredTreeRenderer,
): string {
  const elapsed = pendingAssistant ? formatElapsed(Date.now() - pendingAssistant.startedAtMs) : 'idle';
  const pills = [
    ['context', contextSize],
    ['path', projectPath],
    ['turn wait', elapsed],
    ['focus', activeTurn ? shortTurnLabel(activeTurn.operationId) : '-'],
    ['session', binding?.session_id ?? 'tentative'],
  ];

  return pills
    .map(([label, value]) => `
      <div class="composer-context-pill">
        <span class="composer-context-key">${tree.escapeHtml(label)}</span>
        <span class="composer-context-value">${tree.escapeHtml(String(value))}</span>
      </div>
    `)
    .join('');
}

function compactControlChips(control: JsonRecord): string[] {
  const chips: string[] = [];
  const topic = mergedTopicPercent(control);
  if (topic >= 0) chips.push(`topic ${topic}%`);
  pushPercentChip(chips, 'simple', control.simple_query_confidence);
  return chips.slice(0, 2);
}

function compactProviderChips(provider: { finish: string; status: string }): string[] {
  return [provider.finish, provider.status].filter((item) => item !== '-').slice(0, 2);
}

function compactToolChips(tool: ToolExecutionRecord): string[] {
  const chips = [scalar(tool.status)];
  const targetKind = scalar(tool.target_kind);
  if (targetKind !== '-') chips.push(targetKind);
  const target = scalar(tool.target_ref);
  if (target !== '-') chips.push(shortText(target, 36));
  return chips.filter((item) => item !== '-').slice(0, 3);
}

function pushPercentChip(chips: string[], label: string, value: unknown): void {
  const percent = toPercent(value);
  if (percent >= 0) chips.push(`${label} ${percent}%`);
}

function mergedTopicPercent(control: JsonRecord): number {
  const continuity = toPercent(control.continuity_confidence);
  if (continuity >= 0) return continuity;
  const shift = toPercent(control.topic_shift_confidence);
  if (shift >= 0) return Math.max(0, 100 - shift);
  return -1;
}

function extractControlFeedback(turn: FocusTurn): JsonRecord {
  const eventPayload = turn.events.find((event) => event.event_type === 'control.feedback_recorded')?.payload;
  if (eventPayload && typeof eventPayload === 'object' && !Array.isArray(eventPayload)) return eventPayload as JsonRecord;
  const digest = asRecord(turn.digest);
  const digestFeedback = digest.control_feedback;
  if (digestFeedback && typeof digestFeedback === 'object' && !Array.isArray(digestFeedback)) return digestFeedback as JsonRecord;
  return {};
}

function extractProviderMeta(turn: FocusTurn): { title: string; finish: string; status: string } {
  const accepted = asRecord(turn.events.find((event) => event.event_type === 'provider.operation_accepted')?.payload);
  const completed = asRecord(
    turn.events.find((event) => event.event_type === 'provider.completed')?.payload
    ?? turn.events.find((event) => event.event_type === 'provider.gateway_response_received')?.payload,
  );
  const providerName = scalar(completed.provider_name ?? accepted.provider_name);
  const model = scalar(completed.model ?? accepted.model);
  return {
    title: joinNonEmpty([providerName, model], ' / '),
    finish: scalar(completed.stop_reason),
    status: scalar(completed.status),
  };
}

function toolVerb(tool: ToolExecutionRecord): string {
  const name = scalar(tool.tool_name).toLowerCase();
  if (['read', 'find', 'search', 'list', 'open', 'cat'].some((needle) => name.includes(needle))) return 'Explored';
  if (['edit', 'write', 'patch', 'apply'].some((needle) => name.includes(needle))) return 'Edited';
  if (['spawn', 'delegate'].some((needle) => name.includes(needle))) return 'Delegated';
  return 'Ran';
}

function detailKey(operationId: string, kind: string): string {
  return `${operationId}::${kind}`;
}

function asRecord(value: unknown): JsonRecord {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
  return value as JsonRecord;
}

function scalar(value: unknown): string {
  if (value === null || value === undefined) return '-';
  if (typeof value === 'string') return value.trim() || '-';
  return JSON.stringify(value);
}

function shortText(value: string, limit: number): string {
  return value.length <= limit ? value : `${value.slice(0, limit)}…`;
}

function optionalShortText(value: string, limit: number): string | undefined {
  if (value === '-') return undefined;
  return shortText(value, limit);
}

function joinNonEmpty(items: string[], separator: string): string {
  const valid = items.filter((item) => item && item !== '-');
  return valid.length ? valid.join(separator) : '-';
}

function inferOperationId(messageId?: string): string | null {
  if (!messageId) return null;
  if (messageId.startsWith('user-')) return messageId.slice('user-'.length);
  if (messageId.startsWith('assistant-closure-')) return messageId.slice('assistant-closure-'.length);
  return null;
}

function isNearBottom(element: HTMLElement): boolean {
  return element.scrollHeight - element.scrollTop - element.clientHeight < 40;
}

function shortTurnLabel(operationId: string): string {
  const clean = operationId.replace(/^op-/, '');
  return clean.length <= 18 ? `turn:${clean}` : `turn:${clean.slice(0, 18)}…`;
}

function toPercent(value: unknown): number {
  if (typeof value === 'number' && Number.isFinite(value)) {
    return Math.max(0, Math.min(100, Math.round(value)));
  }
  if (typeof value === 'string' && value.trim()) {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) {
      return Math.max(0, Math.min(100, Math.round(parsed)));
    }
  }
  return -1;
}

function resolveProjectPath(currentContext: JsonRecord | null, binding: DebugBinding | null): string {
  const project = asRecord(asRecord(currentContext?.context).project);
  const value = scalar(project.project_root ?? project.cwd ?? binding?.runtime_home);
  return value.length > 48 ? `…${value.slice(-48)}` : value;
}

function approximateContextSize(currentContext: JsonRecord | null): string {
  if (!currentContext) return '-';
  const chars = JSON.stringify(currentContext).length;
  return chars >= 1000 ? `${(chars / 1000).toFixed(1)}k chars` : `${chars} chars`;
}

function formatElapsed(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const mins = Math.floor(total / 60);
  const secs = total % 60;
  return mins > 0 ? `${mins}:${String(secs).padStart(2, '0')}` : `0:${String(secs).padStart(2, '0')}`;
}
