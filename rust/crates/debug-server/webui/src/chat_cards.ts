import type {
  FocusTurn,
  JsonRecord,
  SourceActivityCardView,
  ToolExecutionRecord,
  ToolSemanticView,
} from './types.js';

export interface TurnCard {
  key: string;
  tone: 'reasoning' | 'meta' | 'tool' | 'source';
  verb: string;
  title: string;
  body?: string;
  extra?: string;
  chips: string[];
  detailTitle: string;
  detailSubtitle: string;
  detailValue: unknown;
}

export function buildTurnCards(
  focusTurn: FocusTurn,
  semanticTools: ToolSemanticView[],
  options: {
    sourceCard?: SourceActivityCardView | null;
    includeMeta?: boolean;
  } = {},
): TurnCard[] {
  const reasoning = asRecord(focusTurn.reasoningView);
  const control = extractControlFeedback(focusTurn);
  const provider = extractProviderMeta(focusTurn);
  const cards: TurnCard[] = [];
  const reasoningSummary = scalar(reasoning.summary);

  if (options.sourceCard) {
    cards.push(buildSourceCard(focusTurn.operationId, options.sourceCard));
  }

  if (reasoningSummary !== '-') {
    cards.push({
      key: detailKey(focusTurn.operationId, 'reasoning'),
      tone: 'reasoning',
      verb: 'Reasoning',
      title: shortText(reasoningSummary, 220),
      body: optionalShortText(
        joinNonEmpty(
          [
            scalar(reasoning.decision_summary),
            scalar(reasoning.continuity_summary),
            scalar(reasoning.next_step),
          ],
          ' · ',
        ),
        200,
      ),
      extra: optionalShortText(
        joinNonEmpty(
          [scalar(reasoning.tool_intent_summary), scalar(reasoning.risk_summary)],
          ' · ',
        ),
        180,
      ),
      chips: [],
      detailTitle: 'Reasoning View',
      detailSubtitle: `turn ${shortTurnLabel(focusTurn.operationId)}`,
      detailValue: reasoning,
    });
  }

  const toolCards = semanticTools.length
    ? semanticTools
    : (focusTurn.toolRecords ?? []).map((tool) => semanticToolFallback(tool));

  for (const [index, tool] of toolCards.entries()) {
    cards.push({
      key: detailKey(focusTurn.operationId, `tool:${tool.tool_call_id ?? index}`),
      tone: 'tool',
      verb: scalar(tool.verb ?? 'Ran'),
      title: scalar(tool.object_label ?? tool.summary ?? tool.tool_name),
      chips: compactSemanticToolChips(tool),
      body: shortText(scalar(tool.summary), 180),
      extra: optionalShortText(scalar(tool.detail), 180),
      detailTitle: scalar(tool.summary ?? tool.tool_name),
      detailSubtitle: `${scalar(tool.verb ?? 'Ran')} · turn ${shortTurnLabel(focusTurn.operationId)}`,
      detailValue: tool,
    });
  }

  if (options.includeMeta) {
    const metaChips = [...compactControlChips(control), ...compactProviderChips(provider)];
    if (provider.title !== '-' || metaChips.length) {
      cards.push({
        key: detailKey(focusTurn.operationId, 'meta'),
        tone: 'meta',
        verb: 'Trace',
        title: provider.title !== '-' ? provider.title : 'Provider / control summary',
        body: undefined,
        extra: undefined,
        chips: metaChips.slice(0, 4),
        detailTitle: 'Provider + Control Summary',
        detailSubtitle: `turn ${shortTurnLabel(focusTurn.operationId)}`,
        detailValue: { provider, control },
      });
    }
  }

  return cards;
}

function buildSourceCard(operationId: string, source: SourceActivityCardView): TurnCard {
  const chips = [
    scalar(source.state),
    scalar(source.visibility),
    source.auto_promoted ? 'promoted' : '-',
  ].filter((item) => item !== '-');
  return {
    key: detailKey(operationId, `source:${scalar(source.source_id)}`),
    tone: 'source',
    verb: 'Source',
    title: scalar(source.title ?? source.source_id),
    body: shortText(scalar(source.current_activity ?? source.summary), 180),
    extra: optionalShortText(
      joinNonEmpty(
        [scalar(source.waiting_detail), scalar(source.failure_detail), scalar(source.focus_label)],
        ' · ',
      ),
      180,
    ),
    chips: chips.slice(0, 4),
    detailTitle: scalar(source.title ?? source.source_id),
    detailSubtitle: `frontstage source · ${shortTurnLabel(operationId)}`,
    detailValue: source,
  };
}

function compactControlChips(control: JsonRecord): string[] {
  const chips: string[] = [];
  const topic = mergedTopicPercent(control);
  if (topic >= 0) chips.push(`topic ${topic}%`);
  pushPercentChip(chips, 'simple', control.simple_query_confidence);
  return chips.slice(0, 2);
}

function compactProviderChips(
  provider: { finish: string; status: string; closureStatus: string; stopSource: string },
): string[] {
  const chips: string[] = [];
  if (provider.closureStatus !== '-') chips.push(`closure ${provider.closureStatus}`);
  if (provider.stopSource !== '-') chips.push(`stop ${provider.stopSource}`);
  if (provider.finish !== '-') chips.push(`finish ${provider.finish}`);
  if (provider.status !== '-') chips.push(`http ${provider.status}`);
  return chips.slice(0, 3);
}

function compactSemanticToolChips(tool: ToolSemanticView): string[] {
  const chips = [scalar(tool.status)];
  const category = scalar(tool.category);
  if (category !== '-') chips.push(category);
  const objectKind = scalar(tool.object_kind);
  if (objectKind !== '-') chips.push(objectKind);
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

function extractProviderMeta(turn: FocusTurn): {
  title: string;
  finish: string;
  status: string;
  closureStatus: string;
  stopSource: string;
} {
  const accepted = asRecord(turn.events.find((event) => event.event_type === 'provider.operation_accepted')?.payload);
  const completed = asRecord(
    turn.events.find((event) => event.event_type === 'provider.completed')?.payload
    ?? turn.events.find((event) => event.event_type === 'provider.gateway_response_received')?.payload,
  );
  const operationCompleted = asRecord(
    turn.events.find((event) => event.event_type === 'operation.completed')?.payload,
  );
  const providerName = scalar(completed.provider_name ?? accepted.provider_name);
  const model = scalar(completed.model ?? accepted.model);
  return {
    title: joinNonEmpty([providerName, model], ' / '),
    finish: scalar(completed.stop_reason),
    status: scalar(completed.status),
    closureStatus: scalar(operationCompleted.status),
    stopSource: scalar(operationCompleted.stop_source),
  };
}

function semanticToolFallback(tool: ToolExecutionRecord): ToolSemanticView {
  return {
    tool_call_id: tool.tool_call_id,
    operation_id: tool.operation_id,
    tool_name: tool.tool_name,
    verb: toolVerb(tool),
    category: tool.target_kind,
    object_kind: tool.target_kind,
    object_label: tool.target_ref,
    summary: scalar(tool.purpose ?? tool.title ?? tool.tool_name),
    detail: joinNonEmpty([scalar(tool.input_summary), scalar(tool.output_summary)], ' → '),
    status: tool.status,
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
