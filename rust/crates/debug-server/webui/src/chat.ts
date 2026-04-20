import type {
  ActivityCardsSnapshot,
  ConversationRichness,
  DebugBinding,
  FocusTurn,
  JsonRecord,
  SessionMessage,
  ToolSemanticView,
  SourceActivityCardView,
} from './types.js';
import {
  activityFocusSource,
  activityHeader,
  activityRecentItems,
  activitySourceCards,
  activityStage,
  activityState,
  activityStateTone,
  activityToolSemantics,
  activityUserCard,
} from './activity_cards_ui.js';
import { buildTurnCards, type TurnCard } from './chat_cards.js';
import { formatLocalTimestamp } from './time.js';
import { StructuredTreeRenderer } from './tree.js';

export interface PendingAssistantState {
  prompt: string;
  startedAtMs: number;
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
    private readonly composerProviderPillEl: HTMLElement,
    private readonly composerModelPillEl: HTMLElement,
    private readonly composerRichnessPillEl: HTMLElement,
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
    activityCards: ActivityCardsSnapshot | null,
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
    this.composerStatusEl.textContent = `session truth · ${binding?.project_id ?? 'fin'} / ${sessionLabel}`;
    this.composerProviderPillEl.textContent = `provider: ${scalar(lastRun?.provider)}`;
    this.composerModelPillEl.textContent = `model: ${scalar(lastRun?.model)}`;
    this.composerRichnessPillEl.textContent = `richness: ${richness}`;
    this.composerContextEl.innerHTML = renderComposerContextBar(
      binding,
      projectPath,
      contextSize,
      pendingAssistant,
      activeTurn,
      this.tree,
    );

    const frontstagePanel = this.renderFrontstagePanel(activityCards, richness);

    if (!messages.length) {
      this.messagesEl.innerHTML = [
        frontstagePanel,
        `
          <div class="empty-state">
            <div class="empty-icon">💬</div>
            <div class="empty-text">No session messages yet. Send the first message into this bound session.</div>
          </div>
        `,
      ].filter(Boolean).join('');
      this.lastRenderedSignature = frontstagePanel;
      return;
    }

    const previousScrollTop = this.messagesEl.scrollTop;
    const wasNearBottom = isNearBottom(this.messagesEl);
    const signature = `${messages.map((message) => `${message.message_id}:${message.created_at}`).join('|')}::${activityCards?.generated_at ?? ''}`;
    const focusByOperation = new Map(focusTurns.map((turn) => [turn.operationId, turn]));
    const semanticsByOperation = groupSemanticsByOperation(activityToolSemantics(activityCards));
    const focusSource = activityFocusSource(activityCards);

    this.messagesEl.innerHTML = [
      frontstagePanel,
      ...messages.map((message) => this.renderMessage(
        message,
        focusByOperation,
        semanticsByOperation,
        focusSource,
        selectedOperationId,
        richness,
        openedChatDetailKey,
      )),
      this.renderPendingAssistant(pendingAssistant),
    ].filter(Boolean).join('');

    if (signature !== this.lastRenderedSignature && wasNearBottom) {
      this.messagesEl.scrollTop = this.messagesEl.scrollHeight;
    } else {
      this.messagesEl.scrollTop = previousScrollTop;
    }
    this.lastRenderedSignature = signature;
  }


  private renderFrontstagePanel(
    activityCards: ActivityCardsSnapshot | null,
    richness: ConversationRichness,
  ): string {
    const userCard = activityUserCard(activityCards);
    const focusSource = activityFocusSource(activityCards);
    const sources = activitySourceCards(activityCards);
    const recentItems = activityRecentItems(activityCards, 3);
    const toolItems = activityToolSemantics(activityCards).slice(0, richness === 'minimal' ? 0 : 4);
    if (!userCard && !focusSource && !toolItems.length) return '';

    const stateLabel = activityState(activityCards);
    const tone = activityStateTone(stateLabel);
    const stage = activityStage(activityCards);
    const updatedAt = userCard?.updated_at ?? focusSource?.updated_at ?? activityCards?.generated_at;

    return `
      <section class="activity-frontstage activity-tone-${this.tree.escapeHtml(tone)}">
        <div class="activity-frontstage-header">
          <div>
            <div class="section-kicker">Frontstage Activity</div>
            <div class="activity-frontstage-title">${this.tree.escapeHtml(activityHeader(activityCards))}</div>
          </div>
          <span class="activity-state-pill ${this.tree.escapeHtml(tone)}">${this.tree.escapeHtml(stateLabel)}</span>
        </div>
        <div class="activity-frontstage-summary">
          ${this.tree.escapeHtml(stage !== '-' ? stage : (focusSource?.summary ?? userCard?.focus_summary ?? 'idle'))}
        </div>
        <div class="activity-frontstage-chip-row">
          <span class="activity-chip">focus · ${this.tree.escapeHtml(focusSource?.title ?? userCard?.focus_source_id ?? 'system-agent')}</span>
          <span class="activity-chip">sources · ${this.tree.escapeHtml(String(sources.length || 1))}</span>
          ${updatedAt ? `<span class="activity-chip">updated · ${this.tree.escapeHtml(formatLocalTimestamp(updatedAt))}</span>` : ''}
        </div>
        ${recentItems.length ? `
          <div class="activity-frontstage-list">
            ${recentItems.map((item) => `
              <article class="activity-list-item">
                <span class="activity-list-bullet">•</span>
                <span>${this.tree.escapeHtml(item)}</span>
              </article>
            `).join('')}
          </div>
        ` : ''}
        ${richness === 'minimal' ? '' : `
          <div class="activity-source-grid">
            ${sources.slice(0, 3).map((card) => `
              <article class="activity-source-card ${this.tree.escapeHtml(activityStateTone(card.state))}">
                <div class="activity-source-header">
                  <span class="activity-source-title">${this.tree.escapeHtml(card.title ?? card.source_id ?? 'source')}</span>
                  <span class="activity-source-state">${this.tree.escapeHtml(card.state ?? '-')}</span>
                </div>
                <div class="activity-source-summary">${this.tree.escapeHtml(card.current_activity ?? card.summary ?? '-')}</div>
              </article>
            `).join('')}
          </div>
        `}
        ${toolItems.length ? `
          <div class="activity-tool-row">
            ${toolItems.map((item) => `
              <span class="activity-tool-pill">${this.tree.escapeHtml(item.summary ?? item.tool_name ?? 'tool')}</span>
            `).join('')}
          </div>
        ` : ''}
      </section>
    `;
  }

  private renderMessage(
    message: SessionMessage,
    focusByOperation: Map<string, FocusTurn>,
    semanticsByOperation: Map<string, ToolSemanticView[]>,
    focusSource: SourceActivityCardView | null,
    selectedOperationId: string | null,
    richness: ConversationRichness,
    openedChatDetailKey: string | null,
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
    const isSelected = Boolean(operationId && operationId === selectedOperationId);
    const turnLabel = operationId ? shortTurnLabel(operationId) : null;
    const semanticTools = operationId ? (semanticsByOperation.get(operationId) ?? []) : [];
    const richPanel = role === 'user'
      ? ''
      : this.renderRichPanel(
          focusTurn,
          semanticTools,
          isSelected ? focusSource : null,
          richness,
          openedChatDetailKey,
          isSelected,
        );

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

  private renderRichPanel(
    focusTurn: FocusTurn | undefined,
    semanticTools: ToolSemanticView[],
    sourceCard: SourceActivityCardView | null,
    richness: ConversationRichness,
    openedChatDetailKey: string | null,
    isSelected: boolean,
  ): string {
    if (!focusTurn || richness === 'minimal') return '';
    const cards = buildTurnCards(focusTurn, semanticTools, {
      sourceCard,
      includeMeta: richness === 'full_trace' || isSelected,
    });
    if (!cards.length) return '';
    return `
      <section class="message-rich-panel ${richness}">
        <div class="turn-card-stack">
          ${cards.map((card) => this.renderTurnCard(card, openedChatDetailKey === card.key)).join('')}
        </div>
      </section>
    `;
  }

  private renderTurnCard(card: TurnCard, expanded: boolean): string {
    return `
      <article
        class="turn-card ${card.tone} interactive ${!card.body && !card.extra ? 'compact' : ''} ${expanded ? 'expanded' : ''}"
        data-open-chat-detail="${this.tree.escapeHtml(card.key)}"
      >
        <div class="turn-card-header">
          <span class="turn-card-verb">${this.tree.escapeHtml(card.verb)}</span>
          <span class="turn-card-title">${this.tree.escapeHtml(card.title)}</span>
          <span class="turn-card-open">${expanded ? 'collapse' : 'detail'}</span>
        </div>
        ${card.chips.length ? `<div class="turn-card-chip-row">${card.chips.map((chip) => `<span class="turn-card-chip">${this.tree.escapeHtml(chip)}</span>`).join('')}</div>` : ''}
        ${card.body ? `<div class="turn-card-body">${this.tree.escapeHtml(card.body)}</div>` : ''}
        ${card.extra ? `<div class="turn-card-extra">${this.tree.escapeHtml(card.extra)}</div>` : ''}
        ${expanded ? `
          <div class="turn-card-detail">
            <div class="turn-card-detail-header">
              <div>
                <div class="section-kicker">Conversation Detail</div>
                <div class="turn-card-detail-title">${this.tree.escapeHtml(card.detailTitle)}</div>
                <div class="turn-card-detail-subtitle muted">${this.tree.escapeHtml(card.detailSubtitle)}</div>
              </div>
            </div>
            <div class="turn-card-detail-body">
              ${this.tree.renderPanelValue(card.detailValue)}
            </div>
          </div>
        ` : ''}
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


function groupSemanticsByOperation(semantics: ToolSemanticView[]): Map<string, ToolSemanticView[]> {
  const grouped = new Map<string, ToolSemanticView[]>();
  for (const item of semantics) {
    const operationId = scalar(item.operation_id);
    if (operationId === '-') continue;
    const existing = grouped.get(operationId) ?? [];
    existing.push(item);
    grouped.set(operationId, existing);
  }
  return grouped;
}

function scalar(value: unknown): string {
  if (value === null || value === undefined) return '-';
  if (typeof value === 'string') return value.trim() || '-';
  return JSON.stringify(value);
}

function asRecord(value: unknown): JsonRecord {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
  return value as JsonRecord;
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
