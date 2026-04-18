import { ChatPane, type PendingAssistantState } from './chat.js';
import { buildFocusTurns, FocusPane } from './focus.js';
import { InspectorPane } from './inspector.js';
import { formatLocalTimestamp } from './time.js';
import { StructuredTreeRenderer } from './tree.js';
import type {
  ClosureTraceRecord,
  ConversationRichness,
  ContextSnapshotRecord,
  DashboardCardId,
  DebugBinding,
  DebugSnapshot,
  DigestRecord,
  JsonRecord,
  ReasoningViewRecord,
  RefreshState,
  RuntimeEvent,
  SessionMessage,
  ToolExecutionRecord,
} from './types.js';

class DebugApp {
  private readonly statusPill = this.requireEl('status-pill');
  private readonly lastUpdatedEl = this.requireEl('last-updated');
  private readonly refreshBtn = this.requireEl('refresh-btn') as HTMLButtonElement;
  private readonly chatForm = this.requireEl('chat-form') as HTMLFormElement;
  private readonly chatInput = this.requireEl('chat-input') as HTMLTextAreaElement;
  private readonly sendBtn = this.requireEl('send-btn') as HTMLButtonElement;
  private readonly composerStatusEl = this.requireEl('composer-status');
  private readonly composerContextEl = this.requireEl('composer-context');
  private readonly messagesEl = this.requireEl('chat-messages');
  private readonly chatHeaderEl = this.requireEl('chat-header');
  private readonly richnessButtons = Array.from(
    document.querySelectorAll<HTMLButtonElement>('[data-richness]'),
  );
  private readonly tree = new StructuredTreeRenderer();
  private readonly chatPane = new ChatPane(
    this.messagesEl,
    this.requireEl('binding-project'),
    this.requireEl('binding-session'),
    this.requireEl('binding-task'),
    this.requireEl('binding-model'),
    this.requireEl('binding-context-size'),
    this.requireEl('binding-project-path'),
    this.composerStatusEl,
    this.composerContextEl,
    this.tree,
  );
  private readonly focusPane = new FocusPane(this.requireEl('focus-pane'), this.tree);
  private readonly inspectorPane = new InspectorPane(this.requireEl('inspector-content'), this.tree);
  private refreshInFlight = false;
  private sending = false;
  private pendingAssistant: PendingAssistantState | null = null;
  private pendingTimer: number | null = null;
  private watchSource: EventSource | null = null;
  private openedChatDetailKey: string | null = null;
  private state: RefreshState = {
    binding: null,
    projection: {},
    events: [],
    sessionEvents: [],
    lastRun: null,
    currentContext: null,
    recentContexts: [],
    recentDigests: [],
    recentReasoningViews: [],
    recentToolRecords: [],
    recentClosures: [],
    messages: [],
    focusTurns: [],
    selectedOperationId: null,
    conversationRichness: 'rich',
    openedCard: null,
    openedSectionKey: null,
  };

  constructor() {
    this.chatForm.addEventListener('submit', (event: Event) => {
      event.preventDefault();
      void this.sendCurrentMessage();
    });
    this.refreshBtn.addEventListener('click', () => {
      void this.refresh();
    });
    this.richnessButtons.forEach((button) => {
      button.addEventListener('click', () => {
        const richness = button.dataset.richness;
        if (richness === 'minimal' || richness === 'rich' || richness === 'full_trace') {
          this.state.conversationRichness = richness;
          this.syncRichnessButtons();
          this.render();
        }
      });
    });
    this.messagesEl.addEventListener('click', (event: Event) => this.onMessageClick(event));
    this.chatHeaderEl.addEventListener('click', (event: Event) => this.onInspectorClick(event));
    this.requireEl('inspector-content').addEventListener('click', (event: Event) => this.onInspectorClick(event));
    document.addEventListener('keydown', (event: KeyboardEvent) => this.onKeyDown(event));
    document.addEventListener('visibilitychange', () => {
      if (document.hidden) {
        this.disconnectWatchStream();
        return;
      }
      void this.refresh();
      this.connectWatchStream();
    });
  }

  start(): void {
    this.syncRichnessButtons();
    void this.refresh();
    this.connectWatchStream();
  }

  private requireEl(id: string): HTMLElement {
    const element = document.getElementById(id);
    if (!element) throw new Error(`missing required element: ${id}`);
    return element;
  }

  private setStatus(text: string, ok: boolean): void {
    this.statusPill.textContent = text;
    this.statusPill.style.borderColor = ok ? 'rgba(56,189,248,0.45)' : 'rgba(255,123,114,0.45)';
    this.statusPill.style.color = ok ? 'var(--accent-strong)' : 'var(--error)';
  }

  private connectWatchStream(): void {
    if (document.hidden || this.watchSource) return;

    const source = new EventSource('/api/watch');
    source.addEventListener('open', () => {
      this.setStatus('live', true);
    });
    source.addEventListener('runtime.ready', () => {
      void this.refresh();
    });
    source.addEventListener('runtime.updated', () => {
      void this.refresh();
    });
    source.onerror = () => {
      this.setStatus('stream reconnecting', false);
    };
    this.watchSource = source;
  }

  private disconnectWatchStream(): void {
    this.watchSource?.close();
    this.watchSource = null;
  }

  private async fetchJson<T>(url: string): Promise<T> {
    const response = await fetch(url, { cache: 'no-store' });
    if (!response.ok) throw new Error(`${url} -> ${response.status}`);
    return response.json() as Promise<T>;
  }

  private async sendCurrentMessage(): Promise<void> {
    const message = this.chatInput.value.trim();
    if (!message || this.sending) return;

    this.sending = true;
    this.pendingAssistant = { prompt: message, startedAtMs: Date.now() };
    this.ensurePendingTimer();
    this.sendBtn.disabled = true;
    this.composerStatusEl.textContent = 'sending...';
    this.render();
    try {
      const response = await fetch('/api/chat/send', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ message }),
      });
      if (!response.ok) {
        throw new Error(await response.text());
      }
      this.chatInput.value = '';
      await this.refresh();
    } finally {
      this.sending = false;
      this.pendingAssistant = null;
      this.stopPendingTimer();
      this.sendBtn.disabled = false;
    }
  }

  private async refresh(): Promise<void> {
    if (this.refreshInFlight) return;

    this.refreshInFlight = true;
    try {
      const [
        binding,
        recentContexts,
        recentDigests,
        recentReasoningViews,
        recentToolRecords,
        recentClosures,
        messages,
        sessionEvents,
        lastRun,
        currentContext,
      ] = await Promise.all([
        this.fetchJson<DebugBinding>('/api/binding.json').catch(() => null),
        this.fetchJson<ContextSnapshotRecord[]>('/api/recent_contexts.json').catch(() => []),
        this.fetchJson<DigestRecord[]>('/api/recent_digests.json').catch(() => []),
        this.fetchJson<ReasoningViewRecord[]>('/api/recent_reasoning_views.json').catch(() => []),
        this.fetchJson<ToolExecutionRecord[]>('/api/recent_tool_records.json').catch(() => []),
        this.fetchJson<ClosureTraceRecord[]>('/api/recent_closures.json').catch(() => []),
        this.fetchJson<SessionMessage[]>('/api/session_messages.json').catch(() => []),
        this.fetchJson<RuntimeEvent[]>('/api/session_events.json').catch(() => []),
        this.fetchJson<JsonRecord>('/api/last_run.json').catch(() => null),
        this.fetchJson<JsonRecord>('/api/current_context.json').catch(() => null),
      ]);

      const focusTurns = buildFocusTurns(
        messages,
        recentContexts,
        recentDigests,
        recentReasoningViews,
        recentToolRecords,
        recentClosures,
        sessionEvents,
      );
      const selectedOperationId = focusTurns.some((turn) => turn.operationId === this.state.selectedOperationId)
        ? this.state.selectedOperationId
        : (focusTurns.length ? focusTurns[focusTurns.length - 1].operationId : null);

      this.state = {
        binding,
        projection: {},
        events: [],
        sessionEvents: Array.isArray(sessionEvents) ? sessionEvents : [],
        lastRun,
        currentContext,
        recentContexts: Array.isArray(recentContexts) ? recentContexts : [],
        recentDigests: Array.isArray(recentDigests) ? recentDigests : [],
        recentReasoningViews: Array.isArray(recentReasoningViews) ? recentReasoningViews : [],
        recentToolRecords: Array.isArray(recentToolRecords) ? recentToolRecords : [],
        recentClosures: Array.isArray(recentClosures) ? recentClosures : [],
        messages: Array.isArray(messages) ? messages : [],
        focusTurns,
        selectedOperationId,
        conversationRichness: this.state.conversationRichness,
        openedCard: this.state.openedCard,
        openedSectionKey: this.state.openedSectionKey,
      };
      this.render();
      this.lastUpdatedEl.textContent = `updated ${formatLocalTimestamp(new Date().toISOString())}`;
      this.setStatus('connected', true);
    } catch (error) {
      this.setStatus('waiting for runtime artifacts', false);
      this.lastUpdatedEl.textContent = error instanceof Error ? error.message : String(error);
    } finally {
      this.refreshInFlight = false;
    }
  }

  private render(): void {
    this.syncRichnessButtons();
    this.chatPane.render(
      this.state.binding,
      this.state.messages,
      this.state.focusTurns,
      this.state.selectedOperationId,
      this.state.conversationRichness,
      this.pendingAssistant,
      this.state.lastRun,
      this.state.currentContext,
      this.openedChatDetailKey,
    );
    this.focusPane.render(this.state);
    this.inspectorPane.render(this.state);
  }

  private syncRichnessButtons(): void {
    this.richnessButtons.forEach((button) => {
      button.classList.toggle('active', button.dataset.richness === this.state.conversationRichness);
    });
  }

  private ensurePendingTimer(): void {
    if (this.pendingTimer !== null) return;
    this.pendingTimer = window.setInterval(() => {
      if (!this.pendingAssistant) {
        this.stopPendingTimer();
        return;
      }
      this.render();
    }, 500);
  }

  private stopPendingTimer(): void {
    if (this.pendingTimer === null) return;
    window.clearInterval(this.pendingTimer);
    this.pendingTimer = null;
  }

  private onMessageClick(event: Event): void {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;

    const closeChatDetail = target.closest<HTMLElement>('[data-close-chat-detail]');
    if (closeChatDetail) {
      this.openedChatDetailKey = null;
      this.render();
      return;
    }

    const chatDetailBackdrop = target.closest<HTMLElement>('[data-chat-detail-backdrop]');
    if (chatDetailBackdrop && target === chatDetailBackdrop) {
      this.openedChatDetailKey = null;
      this.render();
      return;
    }

    const detailTrigger = target.closest<HTMLElement>('[data-open-chat-detail]');
    if (detailTrigger) {
      this.openedChatDetailKey = detailTrigger.dataset.openChatDetail?.trim() ?? null;
      const parentMessage = detailTrigger.closest<HTMLElement>('.message[data-operation-id]');
      const operationId = parentMessage?.dataset.operationId?.trim();
      if (operationId) this.state.selectedOperationId = operationId;
      this.render();
      return;
    }

    const article = target.closest<HTMLElement>('.message[data-operation-id]');
    const operationId = article?.dataset.operationId?.trim();
    if (!operationId) return;

    this.state.selectedOperationId = operationId;
    this.render();
  }

  private onInspectorClick(event: Event): void {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;

    const close = target.closest<HTMLElement>('[data-close-modal]');
    if (close) {
      this.state.openedCard = null;
      this.state.openedSectionKey = null;
      this.render();
      return;
    }

    const closeSection = target.closest<HTMLElement>('[data-close-section-detail]');
    if (closeSection) {
      this.state.openedSectionKey = null;
      this.render();
      return;
    }

    const sectionBackdrop = target.closest<HTMLElement>('[data-section-backdrop]');
    if (sectionBackdrop && target === sectionBackdrop) {
      this.state.openedSectionKey = null;
      this.render();
      return;
    }

    const backdrop = target.closest<HTMLElement>('[data-modal-backdrop]');
    if (backdrop && target === backdrop) {
      this.state.openedCard = null;
      this.state.openedSectionKey = null;
      this.render();
      return;
    }

    const section = target.closest<HTMLElement>('[data-open-section-detail]');
    const sectionKey = section?.dataset.openSectionDetail?.trim();
    if (sectionKey) {
      this.state.openedSectionKey = sectionKey;
      this.render();
      return;
    }

    const card = target.closest<HTMLElement>('[data-open-card]');
    const cardId = card?.dataset.openCard;
    if (
      cardId !== 'provider'
      && cardId !== 'context'
      && cardId !== 'system'
      && cardId !== 'operation'
    ) return;

    this.state.openedCard = cardId as DashboardCardId;
    this.state.openedSectionKey = null;
    this.render();
  }

  private onKeyDown(event: KeyboardEvent): void {
    if (event.key !== 'Escape') return;
    if (this.openedChatDetailKey) {
      this.openedChatDetailKey = null;
      this.render();
      return;
    }
    if (this.state.openedSectionKey) {
      this.state.openedSectionKey = null;
      this.render();
      return;
    }
    if (!this.state.openedCard) return;
    this.state.openedCard = null;
    this.state.openedSectionKey = null;
    this.render();
  }
}

new DebugApp().start();
