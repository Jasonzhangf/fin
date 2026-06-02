import type {
  ActivityCardsSnapshot,
  SourceActivityCardView,
  ToolSemanticView,
  UserActivityCardView,
} from './types.js';

export function activityUserCard(cards: ActivityCardsSnapshot | null): UserActivityCardView | null {
  return cards?.user_card ?? null;
}

export function activitySourceCards(cards: ActivityCardsSnapshot | null): SourceActivityCardView[] {
  return Array.isArray(cards?.source_cards) ? cards.source_cards : [];
}

export function activityFocusSource(cards: ActivityCardsSnapshot | null): SourceActivityCardView | null {
  const sources = activitySourceCards(cards);
  const focusSourceId = activityUserCard(cards)?.focus_source_id ?? null;
  if (focusSourceId) {
    const matched = sources.find((card) => card.source_id === focusSourceId);
    if (matched) return matched;
  }
  return sources.find((card) => card.auto_promoted) ?? sources[0] ?? null;
}

export function activityToolSemantics(cards: ActivityCardsSnapshot | null): ToolSemanticView[] {
  return Array.isArray(cards?.tool_semantics) ? cards.tool_semantics : [];
}

export function activityRecentItems(cards: ActivityCardsSnapshot | null, limit = 3): string[] {
  const userCard = activityUserCard(cards);
  const focus = activityFocusSource(cards);
  const fromUser = Array.isArray(userCard?.recent_items)
    ? userCard.recent_items.filter(isNonEmptyString)
    : [];
  if (fromUser.length) return fromUser.slice(0, limit);
  const fromSource = Array.isArray(focus?.recent_actions)
    ? focus.recent_actions.map((item) => item.summary).filter(isNonEmptyString)
    : [];
  return fromSource.slice(0, limit);
}

export function activityHeader(cards: ActivityCardsSnapshot | null): string {
  return activityUserCard(cards)?.header?.trim() || 'system frontstage · idle';
}

export function activityStage(cards: ActivityCardsSnapshot | null): string {
  return activityUserCard(cards)?.stage?.trim()
    || activityFocusSource(cards)?.current_activity?.trim()
    || activityFocusSource(cards)?.summary?.trim()
    || '-';
}

export function activityState(cards: ActivityCardsSnapshot | null): string {
  return activityUserCard(cards)?.state?.trim()
    || activityFocusSource(cards)?.state?.trim()
    || 'idle';
}

export function activityStateTone(state: string | null | undefined): 'running' | 'waiting' | 'failed' | 'idle' {
  const normalized = (state ?? '').trim().toLowerCase();
  if (['failed', 'error', 'cancelled', 'timed_out', 'disconnected'].includes(normalized)) {
    return 'failed';
  }
  if (['waiting', 'paused', 'pending', 'blocked'].includes(normalized)) {
    return 'waiting';
  }
  if (['running', 'active', 'in_progress', 'working', 'connected'].includes(normalized)) {
    return 'running';
  }
  return 'idle';
}

export function toneClassForSource(card: SourceActivityCardView | null | undefined): string {
  return activityStateTone(card?.state).replace(/_/g, '-');
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}
