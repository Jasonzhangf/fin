import { ChatPane } from './chat.js';
import { InspectorPane } from './inspector.js';
import { StructuredTreeRenderer } from './tree.js';
import type { DebugBinding, DebugSnapshot, InspectorTab, JsonRecord, RefreshState, SessionMessage } from './types.js';

class DebugApp {
  private readonly statusPill = this.requireEl('status-pill');
  private readonly lastUpdatedEl = this.requireEl('last-updated');
  private readonly refreshBtn = this.requireEl('refresh-btn') as HTMLButtonElement;
  private readonly chatForm = this.requireEl('chat-form') as HTMLFormElement;
  private readonly chatInput = this.requireEl('chat-input') as HTMLTextAreaElement;
  private readonly sendBtn = this.requireEl('send-btn') as HTMLButtonElement;
  private readonly composerStatusEl = this.requireEl('composer-status');
  private readonly tabBar = this.requireEl('tab-bar');
  private readonly tree = new StructuredTreeRenderer();
  private readonly chatPane = new ChatPane(
    this.requireEl('chat-messages'),
    this.requireEl('binding-project'),
    this.requireEl('binding-session'),
    this.requireEl('binding-task'),
    this.composerStatusEl,
    this.tree,
  );
  private readonly inspectorPane = new InspectorPane(this.requireEl('inspector-content'), this.tree);
  private activeTab: InspectorTab = 'overview';
  private refreshInFlight = false;
  private sending = false;
  private state: RefreshState = {
    binding: null,
    projection: {},
    events: [],
    lastRun: null,
    currentContext: null,
    recentContexts: [],
    messages: [],
  };

  constructor() {
    this.tabBar.addEventListener('click', (event: Event) => this.onTabClick(event));
    this.chatForm.addEventListener('submit', (event: Event) => {
      event.preventDefault();
      void this.sendCurrentMessage();
    });
    this.refreshBtn.addEventListener('click', () => {
      void this.refresh();
    });
    document.addEventListener('visibilitychange', () => {
      if (!document.hidden) void this.refresh();
    });
  }

  start(): void {
    void this.refresh();
    window.setInterval(() => {
      void this.refresh();
    }, 3000);
  }

  private requireEl(id: string): HTMLElement {
    const element = document.getElementById(id);
    if (!element) throw new Error(`missing required element: ${id}`);
    return element;
  }

  private setStatus(text: string, ok: boolean): void {
    this.statusPill.textContent = text;
    this.statusPill.style.borderColor = ok ? 'rgba(63,185,80,0.45)' : 'rgba(255,123,114,0.45)';
    this.statusPill.style.color = ok ? 'var(--ok)' : 'var(--error)';
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
    if (this.refreshInFlight || document.hidden) return;

    this.refreshInFlight = true;
    try {
      const [binding, snapshot, lastRun, currentContext, recentContexts, messages] = await Promise.all([
        this.fetchJson<DebugBinding>('/api/binding.json').catch(() => null),
        this.fetchJson<DebugSnapshot>('/api/current_snapshot.json'),
        this.fetchJson<JsonRecord>('/api/last_run.json').catch(() => null),
        this.fetchJson<JsonRecord>('/api/current_context.json').catch(() => null),
        this.fetchJson<JsonRecord[]>('/api/recent_contexts.json').catch(() => []),
        this.fetchJson<SessionMessage[]>('/api/session_messages.json').catch(() => []),
      ]);

      this.state = {
        binding,
        projection: snapshot.projection ?? {},
        events: Array.isArray(snapshot.events) ? snapshot.events : [],
        lastRun,
        currentContext,
        recentContexts: Array.isArray(recentContexts) ? recentContexts : [],
        messages: Array.isArray(messages) ? messages : [],
      };
      this.chatPane.render(this.state.binding, this.state.messages);
      this.inspectorPane.render(this.activeTab, this.state);
      this.lastUpdatedEl.textContent = `updated ${new Date().toLocaleTimeString()}`;
      this.setStatus('connected', true);
    } catch (error) {
      this.setStatus('waiting for runtime artifacts', false);
      this.lastUpdatedEl.textContent = error instanceof Error ? error.message : String(error);
    } finally {
      this.refreshInFlight = false;
    }
  }

  private onTabClick(event: Event): void {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;

    const button = target.closest<HTMLElement>('[data-tab]');
    if (!button) return;

    this.activeTab = (button.dataset.tab as InspectorTab | undefined) ?? 'overview';
    this.tabBar.querySelectorAll<HTMLElement>('[data-tab]').forEach((node) => {
      node.classList.toggle('active', node === button);
    });
    this.inspectorPane.render(this.activeTab, this.state);
  }
}

new DebugApp().start();