import type { ConversationRichness } from './types.js';

export function setStatusPill(statusPill: HTMLElement, text: string, ok: boolean): void {
  statusPill.textContent = text;
  statusPill.style.borderColor = ok ? 'rgba(56,189,248,0.45)' : 'rgba(255,123,114,0.45)';
  statusPill.style.color = ok ? 'var(--accent-strong)' : 'var(--error)';
}

export function syncRichnessButtons(
  buttons: HTMLButtonElement[],
  richness: ConversationRichness,
): void {
  buttons.forEach((button) => {
    button.classList.toggle('active', button.dataset.richness === richness);
  });
}
