import { renderEventLedger } from './event_ledger.js';
import { buildEventLedgerView } from './event_ledger_view_state.js';
import { formatLocalTimestamp } from './time.js';
import { renderInspectorSection } from './section_renderers.js';
import { StructuredTreeRenderer } from './tree.js';
import type {
  DashboardCardId,
  EventLedgerView,
  FocusTurn,
  JsonRecord,
  RefreshState,
  RuntimeEvent,
} from './types.js';

interface CardSpec {
  id: DashboardCardId;
  title: string;
  subtitle: string;
  digestLines: Array<[string, string]>;
  focusAreas: string[];
  sections: Array<[string, unknown]>;
  timelineGroups?: Array<[string, RuntimeEvent[]]>;
  eventLedger?: EventLedgerView;
}

export class InspectorPane {
  constructor(
    private readonly rootEl: HTMLElement,
    private readonly tree: StructuredTreeRenderer,
  ) {}

  render(state: RefreshState): void {
    const selected = state.focusTurns.find((turn) => turn.operationId === state.selectedOperationId);
    const cards = this.buildCards(selected, state);
    const openedCard = cards.find((card) => card.id === state.openedCard) ?? null;

    this.rootEl.innerHTML = `
      <section class="dashboard-shell dashboard-stack">
        ${cards.map((card) => this.renderDigestCard(card)).join('')}
      </section>
      ${openedCard ? this.renderDetailModal(openedCard) : ''}
    `;
  }

  private buildCards(selected: FocusTurn | undefined, state: RefreshState): CardSpec[] {
    const ledgerOperationId = state.eventLedgerSelectedOperationId;
    const filteredLedgerEvents = ledgerOperationId
      ? state.eventLedgerEvents.filter((event) => event.operation_id === ledgerOperationId)
      : state.eventLedgerEvents;
    const ledgerLatestEventNames = filteredLedgerEvents
      .slice(-4)
      .map((event) => String(event.event_type ?? '-'))
      .join(' → ');
    const ledgerView = buildEventLedgerView(state);
    const providerAccepted = asRecord(findEvent(selected?.events ?? [], 'provider.operation_accepted')?.payload);
    const providerResponse = asRecord(
      findEvent(selected?.events ?? [], 'provider.completed')?.payload
      ?? findEvent(selected?.events ?? [], 'provider.gateway_response_received')?.payload,
    );
    const providerDebug = asRecord(providerResponse.debug ?? providerAccepted.debug);
    const context = selected?.contextSnapshot;
    const contextView = asRecord(context?.context);
    const controlBlock = asRecord(contextView.control);
    const rolePromptBlock = asRecord(contextView.role_prompt);
    const toolBlock = asRecord(contextView.tools);
    const promptModules = Array.isArray(rolePromptBlock.prompt_modules)
      ? rolePromptBlock.prompt_modules.map((item) => asRecord(item))
      : [];
    const promptLayers = Array.isArray(rolePromptBlock.prompt_layers)
      ? rolePromptBlock.prompt_layers.map((item) => asRecord(item))
      : [];
    const stableCoreLayer = promptLayerById(promptLayers, 'stable_core');
    const roleBaselineLayer = promptLayerById(promptLayers, 'role_baseline');
    const modelOverlayLayer = promptLayerById(promptLayers, 'model_overlay');
    const promptLayerDigest = promptLayers.length
      ? promptLayers
          .map((layer) => `${scalar(layer.layer_id)}:${arrayCount(layer.module_ids)}`)
          .join(' · ')
      : '-';
    const historyBlock = asRecord(contextView.history);
    const knowledgeBlock = asRecord(contextView.knowledge);
    const projectBlock = asRecord(contextView.project);
    const currentInputBlock = asRecord(contextView.current_input);
    const reasoningView = asRecord(selected?.reasoningView);
    const closureTrace = asRecord(selected?.closureTrace);
    const toolRecords = selected?.toolRecords ?? [];
    const request = asRecord(findEvent(selected?.events ?? [], 'inference.started')?.payload);
    const notePayload = asRecord(findEvent(selected?.events ?? [], 'execution_note.appended')?.payload);
    const controlFeedback = asRecord(
      findEvent(selected?.events ?? [], 'control.feedback_recorded')?.payload
      ?? notePayload.control_feedback
      ?? asRecord(selected?.digest).control_feedback,
    );
    const latestEventNames = (selected?.events ?? []).slice(-4).map((event) => String(event.event_type ?? '-')).join(' → ');
    const continuityTail = Array.isArray(contextView.continuity_tail)
      ? contextView.continuity_tail.map((item) => String(item)).join(' | ')
      : '-';
    const stableCoreModules = modulesForLayer(promptModules, stableCoreLayer);
    const roleBaselineModules = modulesForLayer(promptModules, roleBaselineLayer);
    const modelOverlayModules = modulesForLayer(promptModules, modelOverlayLayer);
    const sessionTaskOverlay = {
      current_prompt_summary: rolePromptBlock.current_prompt_summary ?? '-',
      role_id: rolePromptBlock.role_id ?? '-',
      prompt_lineage: rolePromptBlock.prompt_lineage ?? [],
      prompt_history: rolePromptBlock.prompt_history ?? [],
      behavior_rules: rolePromptBlock.behavior_rules ?? [],
      output_contract: rolePromptBlock.output_contract ?? [],
      primary_project: projectBlock.primary_project ?? null,
      active_projects: projectBlock.active_projects ?? [],
      projects: projectBlock.projects ?? [],
    };
    const turnContextEnvelope = {
      control: controlBlock,
      tools: toolBlock,
      project_focus: {
        project_label: projectBlock.project_label ?? '-',
        project_root: projectBlock.project_root ?? '-',
        runtime_home: projectBlock.runtime_home ?? '-',
        cwd: projectBlock.cwd ?? '-',
        selected_paths: projectBlock.selected_paths ?? [],
        relative_selected_paths: projectBlock.relative_selected_paths ?? [],
        scope_summary: projectBlock.scope_summary ?? '-',
        focus_summary: projectBlock.focus_summary ?? '-',
      },
      current_input: currentInputBlock,
      summary: contextView.summary ?? '-',
      continuity_tail: contextView.continuity_tail ?? [],
      provider_meta: {
        role: context?.role ?? '-',
        provider_path: context?.provider_path ?? '-',
        provider_strategy: context?.provider_strategy ?? '-',
        protocol_version: context?.protocol_version ?? '-',
        stream: context?.stream ?? '-',
        captured_at: context?.captured_at ?? '-',
      },
    };
    const growingConversationContext = {
      recent_messages: historyBlock.recent_messages ?? [],
      recent_digests: historyBlock.recent_digests ?? [],
      recent_reasoning: historyBlock.recent_reasoning ?? [],
      recent_tool_activity: historyBlock.recent_tool_activity ?? [],
      knowledge_digest_summaries: knowledgeBlock.digest_summaries ?? [],
      knowledge_artifact_candidates: knowledgeBlock.artifact_candidates ?? [],
      current_input: currentInputBlock.input ?? '-',
    };

    return [
      {
        id: 'provider',
        title: 'Provider',
        subtitle: '最新 provider 更新摘要',
        digestLines: [
          ['target', `${providerResponse.provider_name ?? providerAccepted.provider_name ?? '-'} / ${providerResponse.model ?? providerAccepted.model ?? request.model ?? '-'}`],
          ['result', `status=${scalar(providerResponse.status)} · finish=${scalar(providerResponse.stop_reason)} · response=${shortText(scalar(providerResponse.response_id), 48)}`],
          ['output', shortText(scalar(providerResponse.output_text), 180)],
        ],
        focusAreas: ['Request', 'Response', 'Sanitized Debug', 'Projection Summary'],
        sections: [
          ['Request', providerAccepted],
          ['Response', providerResponse],
          ['Sanitized Debug', providerDebug],
          ['Provider Raw Output', {
            provider_raw_output: closureTrace.provider_raw_output ?? providerResponse.output_text ?? '-',
            provider_endpoint: closureTrace.provider_endpoint ?? providerAccepted.endpoint ?? providerResponse.endpoint ?? '-',
            response_id: providerResponse.response_id ?? '-',
          }],
          ['Projection Summary', {
            selected_operation_id: selected?.operationId ?? '-',
            trace_id: selected?.traceId ?? '-',
            request_header_names: Object.keys(asRecord(providerDebug.request_headers)),
            recent_event_names: (selected?.events ?? []).map((event) => event.event_type ?? '-'),
          }],
        ],
      },
      {
        id: 'context',
        title: 'Context',
        subtitle: '按真实 prompt 装配顺序显示：静态在上，增长上下文在最下',
        digestLines: [
          ['assembly', shortText(promptLayerDigest, 180)],
          ['stable core', shortText(layerDigest(stableCoreLayer, stableCoreModules), 180)],
          ['role modules', shortText(layerDigest(roleBaselineLayer, roleBaselineModules), 180)],
          ['model overlay', shortText(layerDigest(modelOverlayLayer, modelOverlayModules), 180)],
          ['session/task', `lineage=${arrayCount(rolePromptBlock.prompt_lineage)} · prompt-history=${arrayCount(rolePromptBlock.prompt_history)} · projects=${arrayCount(projectBlock.projects)}`],
          ['turn envelope', `tools=${toolCount(toolBlock)} · focus=${shortText(scalar(projectBlock.focus_summary ?? projectBlock.scope_summary), 80)} · input=${shortText(scalar(context?.input ?? selected?.userMessage?.content ?? '-'), 60)}`],
          ['growing context', `messages=${arrayCount(historyBlock.recent_messages)} · digests=${arrayCount(historyBlock.recent_digests)} · reasoning=${arrayCount(historyBlock.recent_reasoning)} · tools=${arrayCount(historyBlock.recent_tool_activity)}`],
          ['profile', `role=${shortText(scalar(context?.role), 48)} · protocol=${scalar(context?.protocol_version)} · stream=${scalar(context?.stream)}`],
          ['contract', shortText(scalar(firstArrayItem(rolePromptBlock.output_contract) ?? rolePromptBlock.current_prompt_summary), 120)],
        ],
        focusAreas: ['Stable Core Prompt', 'Role Prompt Modules', 'Model Overlay', 'Session / Task Overlay', 'Turn Context Envelope', 'Growing Conversation Context', 'Rendered Prompt Trace'],
        sections: [
          ['Stable Core Prompt', {
            layer: stableCoreLayer,
            modules: stableCoreModules,
          }],
          ['Role Prompt Modules', {
            layer: roleBaselineLayer,
            modules: roleBaselineModules,
          }],
          ['Model Overlay', {
            layer: modelOverlayLayer,
            modules: modelOverlayModules,
          }],
          ['Session / Task Overlay', sessionTaskOverlay],
          ['Turn Context Envelope', turnContextEnvelope],
          ['Growing Conversation Context', growingConversationContext],
          ['Rendered Prompt Trace', {
            rendered_input: closureTrace.rendered_input ?? request.rendered_input ?? '-',
            user_input: closureTrace.user_input ?? currentInputBlock.input ?? selected?.userMessage?.content ?? '-',
            assistant_response: closureTrace.assistant_response ?? selected?.assistantMessage?.content ?? '-',
          }],
        ],
      },
      {
        id: 'system',
        title: 'System',
        subtitle: 'project / session / runtime 摘要',
        digestLines: [
          ['scope', `${state.binding?.project_label ?? 'fin'} · session=${state.binding?.session_id ?? '-'} · task=${state.binding?.task_id ?? state.projection.task_id ?? '-'}`],
          ['runtime', shortText(state.binding?.runtime_home ?? '-', 120)],
          ['project', shortText(scalar(projectBlock.project_root ?? projectBlock.cwd ?? '-'), 120)],
          ['projects', `primary=${shortText(scalar(asRecord(projectBlock.primary_project).label), 40)} · active=${arrayCount(projectBlock.active_projects)} · all=${arrayCount(projectBlock.projects)}`],
          ['artifacts', `messages=${shortPath(state.binding?.session_messages_path)} · context=${shortPath(state.binding?.recent_contexts_path)} · digest=${shortPath(state.binding?.recent_digests_path)}`],
          ['trace stores', `reasoning=${state.recentReasoningViews.length} · tools=${state.recentToolRecords.length} · closures=${state.recentClosures.length}`],
        ],
        focusAreas: ['Binding', 'Projection', 'Paths', 'Selected Refs'],
        sections: [
          ['Binding', state.binding ?? {}],
          ['Projection', { note: 'UI render now consumes session artifacts as truth.', recent_contexts_count: state.recentContexts.length, recent_digests_count: state.recentDigests.length, recent_events_count: state.sessionEvents.length, recent_messages_count: state.messages.length, recent_reasoning_count: state.recentReasoningViews.length, recent_tool_record_count: state.recentToolRecords.length, recent_closure_count: state.recentClosures.length }],
          ['Paths', {
            runtime_home: state.binding?.runtime_home ?? '-',
            session_messages_path: state.binding?.session_messages_path ?? '-',
            recent_contexts_path: state.binding?.recent_contexts_path ?? '-',
            recent_digests_path: state.binding?.recent_digests_path ?? '-',
          }],
          ['Selected Refs', {
            operation_id: selected?.operationId ?? '-',
            trace_id: selected?.traceId ?? '-',
            session_id: selected?.userMessage?.session_id ?? context?.session_id ?? state.binding?.session_id ?? '-',
            task_id: selected?.userMessage?.task_id ?? context?.task_id ?? state.binding?.task_id ?? '-',
          }],
        ],
      },
      {
        id: 'operation',
        title: 'Operation & Event',
        subtitle: '消息 / 请求 / 事件链摘要',
        digestLines: [
          ['operation', `${selected?.operationId ?? '-'} · trace=${selected?.traceId ?? '-'}`],
          ['timeline', shortText(latestEventNames || '-', 180)],
          ['ledger', `scope=${state.eventLedgerScope} · segment=${state.eventLedgerSegment ?? '-'} · op=${ledgerOperationId ?? '-'} · events=${filteredLedgerEvents.length}`],
          ['control', `continuity=${scalar(controlFeedback.continuity_confidence)} · shift=${scalar(controlFeedback.topic_shift_confidence)} · simple=${scalar(controlFeedback.simple_query_confidence)}`],
          ['reasoning', shortText(scalar(reasoningView.summary ?? notePayload.summary), 120)],
          ['tools', `records=${toolRecords.length} · ${shortText(toolRecords.map((record) => `${scalar(record.tool_name)}:${scalar(record.status)}`).join(' | '), 120)}`],
          ['closure', shortText(scalar(selected?.digest?.summary ?? selected?.assistantMessage?.content ?? '-'), 180)],
        ],
        focusAreas: ['Turn Messages', 'Request Structure', 'Control Feedback', 'Reasoning View', 'Tool Activity', 'Execution Note', 'Closure Trace', 'Selected Timeline', 'Event Ledger', 'Digest'],
        sections: [
          ['Turn Messages', { user: selected?.userMessage ?? {}, assistant: selected?.assistantMessage ?? {} }],
          ['Request Structure', request],
          ['Control Feedback', controlFeedback],
          ['Reasoning View', reasoningView],
          ['Tool Activity', toolRecords],
          ['Execution Note', notePayload],
          ['Closure Trace', closureTrace],
          ['Selected Timeline', { events: selected?.events ?? [] }],
          ['Event Ledger', {
            scope: state.eventLedgerScope,
            segment: state.eventLedgerSegment ?? null,
            selected_operation_id: ledgerOperationId,
            live_event_count: state.eventArchiveIndex?.live_event_count ?? state.sessionEvents.length,
            local_segments: state.eventArchiveIndex?.local_segments ?? [],
            cold_segments: state.eventArchiveIndex?.cold_segments ?? [],
          }],
          ['Ledger Timeline', { events: filteredLedgerEvents.slice(-32), latest_event_names: ledgerLatestEventNames || '-' }],
          ['Digest', selected?.digest ?? {}],
        ],
        timelineGroups: [
          ['Selected Request Timeline', selected?.events ?? []],
          ['Event Ledger Timeline', filteredLedgerEvents.slice(-32)],
        ],
        eventLedger: ledgerView,
      },
    ];
  }

  private renderDigestCard(card: CardSpec): string {
    return `
      <article class="dashboard-card">
        <button
          class="dashboard-digest-card"
          type="button"
          data-open-card="${this.tree.escapeHtml(card.id)}"
          aria-expanded="false"
        >
          <div class="dashboard-digest-header">
            <div>
              <h3>${this.tree.escapeHtml(card.title)}</h3>
              <p class="muted">${this.tree.escapeHtml(card.subtitle)}</p>
            </div>
            <span class="dashboard-digest-open">Expand</span>
          </div>
          <div class="dashboard-digest-lines">
            ${card.digestLines.slice(0, 3).map(([label, value]) => `
              <article class="digest-line">
                <span class="digest-line-label">${this.tree.escapeHtml(label)}</span>
                <span class="digest-line-value">${this.tree.escapeHtml(value)}</span>
              </article>
            `).join('')}
          </div>
        </button>
      </article>
    `;
  }

  private renderDetailModal(card: CardSpec): string {
    return `
      <div class="detail-modal-backdrop" data-close-card-backdrop>
        <section class="detail-modal dashboard-detail-modal" role="dialog" aria-modal="true">
          <header class="detail-modal-header">
            <div>
              <div class="section-kicker">Debug Detail</div>
              <h3>${this.tree.escapeHtml(card.title)}</h3>
              <p class="muted">${this.tree.escapeHtml(card.subtitle)}</p>
            </div>
            <button class="detail-modal-close" type="button" data-close-card-detail>Close</button>
          </header>
          ${card.focusAreas.length ? `
            <div class="detail-focus-strip">
              ${card.focusAreas.map((focus) => `
                <span class="detail-focus-chip">${this.tree.escapeHtml(focus)}</span>
              `).join('')}
            </div>
          ` : ''}
          <div class="detail-modal-body">
            ${card.eventLedger ? renderEventLedger(this.tree, card.eventLedger) : ''}
            <div class="detail-modal-summary">
              ${card.digestLines.map(([label, value]) => `
                <article class="summary-chip">
                  <span class="summary-chip-label">${this.tree.escapeHtml(label)}</span>
                  <span class="summary-chip-value">${this.tree.escapeHtml(value)}</span>
                </article>
              `).join('')}
            </div>
            ${card.timelineGroups ? `
              <div class="detail-timeline-grid">
                ${card.timelineGroups.map(([title, events]) => `
                  <section class="timeline-block subtle">
                    <div class="detail-subtitle">${this.tree.escapeHtml(title)}</div>
                    ${this.renderEventTimeline(events)}
                  </section>
                `).join('')}
              </div>
            ` : ''}
            <div class="detail-section-stack">
              ${card.sections.map(([label, value], index) => this.renderInlineSection(card.id, label, value, index === 0)).join('')}
            </div>
          </div>
        </section>
      </div>
    `;
  }

  private renderInlineSection(
    cardId: DashboardCardId,
    label: string,
    value: unknown,
    open: boolean,
  ): string {
    return `
      <details class="inline-detail-section" ${open ? 'open' : ''}>
        <summary class="inline-detail-summary">
          <span>${this.tree.escapeHtml(label)}</span>
          <span class="inline-detail-open">toggle</span>
        </summary>
        <div class="detail-section-body">${renderInspectorSection(cardId, label, value, this.tree)}</div>
      </details>
    `;
  }

  private renderEventTimeline(events: RuntimeEvent[]): string {
    if (!events.length) return this.empty('No events');
    return `
      <div class="timeline-list">
        ${events.map((event) => `
          <article class="timeline-item ${timelineLevel(event.event_type)}">
            <div class="timeline-item-header">
              <span class="timeline-seq">${this.tree.escapeHtml(String(event.sequence ?? '-'))}</span>
              <span class="timeline-name">${this.tree.escapeHtml(String(event.event_type ?? '-'))}</span>
              <span class="timeline-time">${this.tree.escapeHtml(formatLocalTimestamp(event.occurred_at ?? event.timestamp))}</span>
            </div>
            <div class="timeline-meta">
              <span>trace=${this.tree.escapeHtml(scalar(event.trace_id))}</span>
              <span>source=${this.tree.escapeHtml(scalar(event.source))}</span>
            </div>
          </article>
        `).join('')}
      </div>
    `;
  }

  private empty(text: string): string {
    return `<div class="empty-state compact"><div class="empty-text">${this.tree.escapeHtml(text)}</div></div>`;
  }
}

function findEvent(events: RuntimeEvent[], eventType: string): RuntimeEvent | undefined {
  return events.find((event) => event.event_type === eventType);
}

function asRecord(value: unknown): JsonRecord {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
  return value as JsonRecord;
}

function scalar(value: unknown): string {
  if (value === null || value === undefined) return '-';
  if (typeof value === 'string') return value || '-';
  return JSON.stringify(value);
}

function shortText(value: string, limit: number): string {
  const text = value.trim();
  if (!text) return '-';
  return text.length <= limit ? text : `${text.slice(0, limit)}…`;
}

function shortPath(value: unknown): string {
  const text = scalar(value);
  if (text === '-') return text;
  const parts = text.split('/').filter(Boolean);
  return parts.length <= 3 ? text : `…/${parts.slice(-3).join('/')}`;
}

function promptLayerById(layers: JsonRecord[], layerId: string): JsonRecord {
  return layers.find((layer) => scalar(layer.layer_id) === layerId) ?? {};
}

function modulesForLayer(modules: JsonRecord[], layer: JsonRecord): JsonRecord[] {
  const moduleIds = Array.isArray(layer.module_ids)
    ? layer.module_ids.map((item) => String(item))
    : [];
  if (!moduleIds.length) return [];
  const order = new Map(moduleIds.map((moduleId, index) => [moduleId, index]));
  return modules
    .filter((module) => order.has(scalar(module.module_id)))
    .sort((left, right) => {
      const leftIndex = order.get(scalar(left.module_id)) ?? 0;
      const rightIndex = order.get(scalar(right.module_id)) ?? 0;
      return leftIndex - rightIndex;
    });
}

function layerDigest(layer: JsonRecord, modules: JsonRecord[]): string {
  if (!countKeys(layer)) return 'inactive';
  const title = scalar(layer.title);
  const summary = scalar(layer.summary);
  return `${title} · modules=${modules.length} · ${summary}`;
}

function countKeys(value: JsonRecord): number {
  return Object.keys(value).length;
}

function arrayCount(value: unknown): number {
  return Array.isArray(value) ? value.length : 0;
}

function firstArrayItem(value: unknown): unknown {
  return Array.isArray(value) && value.length ? value[0] : undefined;
}

function toolCount(value: JsonRecord): number {
  return arrayCount(value.model_tools) + arrayCount(value.framework_tools);
}

function timelineLevel(eventType?: string): string {
  if (!eventType) return 'neutral';
  if (eventType.includes('failed') || eventType.includes('timeout')) return 'error';
  if (eventType.includes('completed') || eventType.includes('finalized')) return 'ok';
  return 'neutral';
}
