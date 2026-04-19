import { setStatusPill, syncRichnessButtons } from './app_ui.js';
import { loadRefreshState } from './app_refresh.js';
import { ChatPane, type PendingAssistantState } from './chat.js';
import { FocusPane } from './focus.js';
import { InspectorPane } from './inspector.js';
import { renderSidebar, type SidebarSectionId } from './sidebar.js';
import { formatLocalTimestamp } from './time.js';
import { StructuredTreeRenderer } from './tree.js';
import type {
  ConversationRichness,
  DashboardCardId,
  DebugBinding,
  EventLedgerScope,
  RefreshState,
} from './types.js';

class DebugApp {
  private readonly statusPill = this.requireEl('status-pill');
  private readonly lastUpdatedEl = this.requireEl('last-updated');
  private readonly sidebarEl = this.requireEl('nav-sidebar');
  private readonly refreshBtn = this.requireEl('refresh-btn') as HTMLButtonElement;
  private readonly chatForm = this.requireEl('chat-form') as HTMLFormElement;
  private readonly chatInput = this.requireEl('chat-input') as HTMLTextAreaElement;
  private readonly sendBtn = this.requireEl('send-btn') as HTMLButtonElement;
  private readonly composerStatusEl = this.requireEl('composer-status');
  private readonly composerProviderPillEl = this.requireEl('composer-provider-pill');
  private readonly composerModelPillEl = this.requireEl('composer-model-pill');
  private readonly composerRichnessPillEl = this.requireEl('composer-richness-pill');
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
    this.composerProviderPillEl,
    this.composerModelPillEl,
    this.composerRichnessPillEl,
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
  private expandedRailSectionId: SidebarSectionId | null = 'session';
  private openedRailSectionId: SidebarSectionId | null = null;
  private expandedCardId: DashboardCardId | null = 'operation';
  private state: RefreshState = {
    binding: null,
    projection: {},
    events: [],
    sessionEvents: [],
    eventArchiveIndex: null,
    eventLedgerScope: 'live',
    eventLedgerSegment: null,
    eventLedgerEvents: [],
    eventLedgerSelectedOperationId: null,
    lastRun: null,
    currentContext: null,
    recentContexts: [],
    recentDigests: [],
    recentReasoningViews: [],
    recentToolRecords: [],
    recentClosures: [],
    recentTurns: [],
    currentExecutionState: null,
    currentPendingInputs: [],
    currentPauseCheckpoint: null,
    currentInterruptedSegment: null,
    currentSegmentMerge: null,
    currentRoutingDecision: null,
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
    this.sidebarEl.addEventListener('click', (event: Event) => this.onSidebarClick(event));
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
    if (!element) throw new Error(`missing required element: ${id}`); return element;
  }

  private setStatus(text: string, ok: boolean): void {
    setStatusPill(this.statusPill, text, ok);
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
    this.watchSource?.close(); this.watchSource = null;
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
      this.state = await loadRefreshState(this.fetchJson.bind(this), this.state);
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
    renderSidebar(
      this.sidebarEl,
      this.state,
      this.tree,
      this.expandedRailSectionId,
      this.openedRailSectionId,
    );
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
    this.inspectorPane.render(this.state, this.expandedCardId, this.state.openedCard);
  }

  private syncRichnessButtons(): void {
    syncRichnessButtons(this.richnessButtons, this.state.conversationRichness);
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
    window.clearInterval(this.pendingTimer); this.pendingTimer = null;
  }

  private onMessageClick(event: Event): void {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;

    const detailTrigger = target.closest<HTMLElement>('[data-open-chat-detail]');
    if (detailTrigger) {
      const detailKey = detailTrigger.dataset.openChatDetail?.trim() ?? null;
      this.openedChatDetailKey = this.openedChatDetailKey === detailKey ? null : detailKey;
      const parentMessage = detailTrigger.closest<HTMLElement>('.message[data-operation-id]');
      const operationId = parentMessage?.dataset.operationId?.trim();
      if (operationId) this.state.selectedOperationId = operationId;
      if (this.state.eventLedgerScope === 'live') {
        this.state.eventLedgerSelectedOperationId = operationId ?? this.state.eventLedgerSelectedOperationId;
      }
      this.render();
      return;
    }

    const article = target.closest<HTMLElement>('.message[data-operation-id]');
    const operationId = article?.dataset.operationId?.trim();
    if (!operationId) return;

    this.state.selectedOperationId = operationId;
    if (this.state.eventLedgerScope === 'live') {
      this.state.eventLedgerSelectedOperationId = operationId;
    }
    this.render();
  }

  private onSidebarClick(event: Event): void {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;

    const detailTrigger = target.closest<HTMLElement>('[data-open-rail-detail]');
    const detailSectionId = detailTrigger?.dataset.openRailDetail;
    if (
      detailSectionId === 'project'
      || detailSectionId === 'session'
      || detailSectionId === 'execution'
      || detailSectionId === 'skills'
      || detailSectionId === 'plugins'
    ) {
      this.openedRailSectionId = detailSectionId;
      this.render();
      return;
    }

    const closeButton = target.closest<HTMLElement>('[data-close-rail-detail]');
    if (closeButton) {
      this.openedRailSectionId = null;
      this.render();
      return;
    }

    const backdrop = target.closest<HTMLElement>('[data-close-rail-backdrop]');
    if (backdrop && target === backdrop) {
      this.openedRailSectionId = null;
      this.render();
      return;
    }

    const trigger = target.closest<HTMLElement>('[data-toggle-rail-section]');
    const sectionId = trigger?.dataset.toggleRailSection;
    if (
      sectionId !== 'project'
      && sectionId !== 'session'
      && sectionId !== 'execution'
      && sectionId !== 'skills'
      && sectionId !== 'plugins'
    ) return;

    this.expandedRailSectionId = this.expandedRailSectionId === sectionId ? null : sectionId;
    this.render();
  }

  private onInspectorClick(event: Event): void {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;

    const ledgerScopeButton = target.closest<HTMLElement>('[data-event-ledger-scope]');
    if (ledgerScopeButton) {
      const scope = ledgerScopeButton.dataset.eventLedgerScope;
      if (scope === 'live' || scope === 'local_archive' || scope === 'cold_archive') {
        this.onInspectorLedgerScope(scope, null);
      }
      return;
    }

    const ledgerSegmentButton = target.closest<HTMLElement>('[data-event-ledger-segment]');
    if (ledgerSegmentButton) {
      const scope = ledgerSegmentButton.dataset.eventLedgerTier;
      const segment = ledgerSegmentButton.dataset.eventLedgerSegment?.trim() ?? null;
      if (
        segment
        && (scope === 'local_archive' || scope === 'cold_archive')
      ) {
        this.onInspectorLedgerScope(scope, segment);
      }
      return;
    }

    const ledgerOperationButton = target.closest<HTMLElement>('[data-event-ledger-operation]');
    if (ledgerOperationButton) {
      const operationId = ledgerOperationButton.dataset.eventLedgerOperation?.trim() ?? null;
      if (operationId) {
        this.state.eventLedgerSelectedOperationId = operationId;
        if (this.state.focusTurns.some((turn) => turn.operationId === operationId)) {
          this.state.selectedOperationId = operationId;
        }
        this.render();
      }
      return;
    }

    const closeButton = target.closest<HTMLElement>('[data-close-card-detail]');
    if (closeButton) {
      this.state.openedCard = null;
      this.state.openedSectionKey = null;
      this.render();
      return;
    }

    const backdrop = target.closest<HTMLElement>('[data-close-card-backdrop]');
    if (backdrop && target === backdrop) {
      this.state.openedCard = null;
      this.state.openedSectionKey = null;
      this.render();
      return;
    }

    const detailTrigger = target.closest<HTMLElement>('[data-open-card-detail]');
    const detailCardId = detailTrigger?.dataset.openCardDetail;
    if (
      detailCardId === 'provider'
      || detailCardId === 'context'
      || detailCardId === 'system'
      || detailCardId === 'operation'
    ) {
      this.state.openedCard = detailCardId;
      this.render();
      return;
    }

    const card = target.closest<HTMLElement>('[data-toggle-card], [data-open-card]');
    const cardId = card?.dataset.toggleCard ?? card?.dataset.openCard;
    if (
      cardId !== 'provider'
      && cardId !== 'context'
      && cardId !== 'system'
      && cardId !== 'operation'
    ) return;

    this.expandedCardId = this.expandedCardId === cardId ? null : cardId as DashboardCardId;
    this.render();
  }

  private onInspectorLedgerScope(scope: EventLedgerScope, segment: string | null): void {
    this.state.eventLedgerScope = scope;
    this.state.eventLedgerSegment = segment;
    this.state.eventLedgerSelectedOperationId = null;
    this.render();
    void this.refresh();
  }

  private onKeyDown(event: KeyboardEvent): void {
    if (event.key !== 'Escape') return;
    if (this.openedChatDetailKey) {
      this.openedChatDetailKey = null;
      this.render();
      return;
    }
    if (this.openedRailSectionId) {
      this.openedRailSectionId = null;
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
