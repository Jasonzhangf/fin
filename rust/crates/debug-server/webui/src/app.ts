import { ChatPane } from './chat.js';
import { buildFocusTurns, FocusPane } from './focus.js';
import { InspectorPane } from './inspector.js';
import { formatLocalTimestamp } from './time.js';
import { StructuredTreeRenderer } from './tree.js';
import type {
  ContextSnapshotRecord,
  DashboardCardId,
  DebugBinding,
  DebugSnapshot,
  DigestRecord,
  JsonRecord,
  RefreshState,
  RuntimeEvent,
  SessionMessage,
} from './types.js';

class DebugApp {
  private readonly statusPill = this.requireEl('status-pill');
  private readonly lastUpdatedEl = this.requireEl('last-updated');
  private readonly refreshBtn = this.requireEl('refresh-btn') as HTMLButtonElement;
  private readonly chatForm = this.requireEl('chat-form') as HTMLFormElement;
  private readonly chatInput = this.requireEl('chat-input') as HTMLTextAreaElement;
  private readonly sendBtn = this.requireEl('send-btn') as HTMLButtonElement;
  private readonly composerStatusEl = this.requireEl('composer-status');
  private readonly messagesEl = this.requireEl('chat-messages');
  private readonly tree = new StructuredTreeRenderer();
  private readonly chatPane = new ChatPane(
    this.messagesEl,
    this.requireEl('binding-project'),
    this.requireEl('binding-session'),
    this.requireEl('binding-task'),
    this.composerStatusEl,
    this.tree,
  );
  private readonly focusPane = new FocusPane(this.requireEl('focus-pane'), this.tree);
  private readonly inspectorPane = new InspectorPane(this.requireEl('inspector-content'), this.tree);
  private refreshInFlight = false;
  private sending = false;
  private watchSource: EventSource | null = null;
  private state: RefreshState = {
    binding: null,
    projection: {},
    events: [],
    sessionEvents: [],
    lastRun: null,
    currentContext: null,
    recentContexts: [],
    recentDigests: [],
    messages: [],
    focusTurns: [],
    selectedOperationId: null,
    openedCard: null,
  };

  constructor() {
    this.chatForm.addEventListener('submit', (event: Event) => {
      event.preventDefault();
      void this.sendCurrentMessage();
    });
    this.refreshBtn.addEventListener('click', () => {
      void this.refresh();
    });
    this.messagesEl.addEventListener('click', (event: Event) => this.onMessageClick(event));
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
    this.sendBtn.disabled = true;
    this.composerStatusEl.textContent = 'sending...';
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
      this.sendBtn.disabled = false;
    }
  }

  private async refresh(): Promise<void> {
    if (this.refreshInFlight) return;

    this.refreshInFlight = true;
    try {
      const [binding, recentContexts, recentDigests, messages, sessionEvents] = await Promise.all([
        this.fetchJson<DebugBinding>('/api/binding.json').catch(() => null),
        this.fetchJson<ContextSnapshotRecord[]>('/api/recent_contexts.json').catch(() => []),
        this.fetchJson<DigestRecord[]>('/api/recent_digests.json').catch(() => []),
        this.fetchJson<SessionMessage[]>('/api/session_messages.json').catch(() => []),
        this.fetchJson<RuntimeEvent[]>('/api/session_events.json').catch(() => []),
      ]);

      const focusTurns = buildFocusTurns(messages, recentContexts, recentDigests, sessionEvents);
      const selectedOperationId = focusTurns.some((turn) => turn.operationId === this.state.selectedOperationId)
        ? this.state.selectedOperationId
        : (focusTurns.length ? focusTurns[focusTurns.length - 1].operationId : null);

      this.state = {
        binding,
        projection: {},
        events: [],
        sessionEvents: Array.isArray(sessionEvents) ? sessionEvents : [],
        lastRun: null,
        currentContext: null,
        recentContexts: Array.isArray(recentContexts) ? recentContexts : [],
        recentDigests: Array.isArray(recentDigests) ? recentDigests : [],
        messages: Array.isArray(messages) ? messages : [],
        focusTurns,
        selectedOperationId,
        openedCard: this.state.openedCard,
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
    this.chatPane.render(this.state.binding, this.state.messages, this.state.selectedOperationId);
    this.focusPane.render(this.state);
    this.inspectorPane.render(this.state);
  }

  private onMessageClick(event: Event): void {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;

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
      this.render();
      return;
    }

    const backdrop = target.closest<HTMLElement>('[data-modal-backdrop]');
    if (backdrop && target === backdrop) {
      this.state.openedCard = null;
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
    this.render();
  }

  private onKeyDown(event: KeyboardEvent): void {
    if (event.key !== 'Escape' || !this.state.openedCard) return;
    this.state.openedCard = null;
    this.render();
  }
}

new DebugApp().start();
