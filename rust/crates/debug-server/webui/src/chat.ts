import type { DebugBinding, SessionMessage } from './types.js';
import { StructuredTreeRenderer } from './tree.js';

export class ChatPane {
  constructor(
    private readonly messagesEl: HTMLElement,
    private readonly projectEl: HTMLElement,
    private readonly sessionEl: HTMLElement,
    private readonly taskEl: HTMLElement,
    private readonly composerStatusEl: HTMLElement,
    private readonly tree: StructuredTreeRenderer,
  ) {}

  render(binding: DebugBinding | null, messages: SessionMessage[]): void {
    const sessionLabel = binding?.session_id ?? 'tentative';
    const taskLabel = binding?.task_id ?? '-';
    this.projectEl.textContent = `project: ${binding?.project_label ?? 'fin'}`;
    this.sessionEl.textContent = `session: ${sessionLabel}`;
    this.taskEl.textContent = `task: ${taskLabel}`;
    this.composerStatusEl.textContent = `project=${binding?.project_id ?? 'fin'} · session=${sessionLabel}`;

    if (!messages.length) {
      this.messagesEl.innerHTML = `
        <div class="empty-state">
          <div class="empty-icon">💬</div>
          <div class="empty-text">No session messages yet. Send the first message into this bound session.</div>
        </div>
      `;
      return;
    }

    this.messagesEl.innerHTML = messages
      .map((message) => {
        const role = message.role === 'user' ? 'user' : 'agent';
        const avatar = role === 'user' ? '🧑' : '🤖';
        const sender = role === 'user' ? 'User' : 'Assistant';
        const nameClass = role === 'user' ? 'sender-label' : 'agent-name';
        return `
          <article class="message ${role}">
            <div class="message-avatar">${avatar}</div>
            <div class="message-content-wrapper">
              <div class="message-header">
                <span class="${nameClass}">${sender}</span>
                <span class="message-time">${this.tree.escapeHtml(message.created_at ?? '-')}</span>
              </div>
              <div class="message-body">${this.tree.escapeHtml(message.content ?? '')}</div>
            </div>
          </article>
        `;
      })
      .join('');

    this.messagesEl.scrollTop = this.messagesEl.scrollHeight;
  }
}
