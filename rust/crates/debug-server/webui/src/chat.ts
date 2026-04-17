import type { DebugBinding, SessionMessage } from './types.js';
import { formatLocalTimestamp } from './time.js';
import { StructuredTreeRenderer } from './tree.js';

export class ChatPane {
  private lastRenderedSignature = '';

  constructor(
    private readonly messagesEl: HTMLElement,
    private readonly projectEl: HTMLElement,
    private readonly sessionEl: HTMLElement,
    private readonly taskEl: HTMLElement,
    private readonly composerStatusEl: HTMLElement,
    private readonly tree: StructuredTreeRenderer,
  ) {}

  render(
    binding: DebugBinding | null,
    messages: SessionMessage[],
    selectedOperationId: string | null,
  ): void {
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
      this.lastRenderedSignature = '';
      return;
    }

    const previousScrollTop = this.messagesEl.scrollTop;
    const wasNearBottom = isNearBottom(this.messagesEl);
    const signature = messages.map((message) => `${message.message_id}:${message.created_at}`).join('|');

    this.messagesEl.innerHTML = messages
      .map((message) => {
        const role = message.role === 'user' ? 'user' : 'agent';
        const presentation = role === 'user'
          ? { avatar: '🧑', label: 'User', badge: 'user' }
          : {
              avatar: '🤖',
              label: message.role === 'assistant' || !message.role ? 'Assistant' : String(message.role),
              badge: message.role === 'assistant' || !message.role ? 'assistant' : 'agent',
            };
        const operationId = message.operation_id ?? inferOperationId(message.message_id);
        const selectedClass = operationId && operationId === selectedOperationId ? 'selected' : '';
        const turnLabel = operationId ? shortTurnLabel(operationId) : null;
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
            </div>
          </article>
        `;
      })
      .join('');

    if (signature !== this.lastRenderedSignature && wasNearBottom) {
      this.messagesEl.scrollTop = this.messagesEl.scrollHeight;
    } else {
      this.messagesEl.scrollTop = previousScrollTop;
    }
    this.lastRenderedSignature = signature;
  }
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
