import { StructuredTreeRenderer } from './tree.js';
import type { DashboardCardId, JsonRecord } from './types.js';

export function renderInspectorSection(
  cardId: DashboardCardId,
  label: string,
  value: unknown,
  tree: StructuredTreeRenderer,
): string {
  if (cardId === 'provider' && label === 'Provider Raw Output') {
    return renderProviderRawOutput(asRecord(value), tree);
  }
  if (cardId === 'operation' && label === 'Tool Activity') {
    return renderToolActivity(asArray(value), tree);
  }
  if (cardId === 'operation' && label === 'Reasoning View') {
    return renderReasoningView(asRecord(value), tree);
  }
  if (cardId === 'operation' && label === 'Closure Trace') {
    return renderClosureTrace(asRecord(value), tree);
  }
  if (cardId === 'operation' && label === 'Closed Loop Receipt') {
    return renderClosedLoopReceipt(asRecord(value), tree);
  }
  if (cardId === 'operation' && label === 'Framework Flow') {
    return renderFrameworkFlow(asRecord(value), tree);
  }
  if (cardId === 'operation' && label === 'Framework Timeline') {
    return renderFrameworkTimeline(asRecord(value), tree);
  }
  if (cardId === 'system' && label === 'Framework Session Timeline') {
    return renderFrameworkTimeline(asRecord(value), tree);
  }
  return tree.renderPanelValue(value);
}

function renderProviderRawOutput(value: JsonRecord, tree: StructuredTreeRenderer): string {
  const raw = scalar(value.provider_raw_output);
  const parsed = parseStructuredOutput(raw);
  return `
    <div class="semantic-stack">
      ${renderSummaryGrid([
        ['endpoint', scalar(value.provider_endpoint)],
        ['response id', scalar(value.response_id)],
        ['blocks', `${parsed.answer ? 'user_response' : 'raw'} · ${parsed.control ? 'control_feedback' : 'no_control'}`],
      ], tree)}
      ${parsed.answer ? renderTextPanel('User Response Block', parsed.answer, tree) : ''}
      ${parsed.control ? renderTextPanel('Control Feedback Block', parsed.control, tree) : ''}
      ${renderTextPanel('Raw Provider Output', raw, tree)}
    </div>
  `;
}

function renderReasoningView(value: JsonRecord, tree: StructuredTreeRenderer): string {
  if (!Object.keys(value).length) return tree.renderPanelValue(value);
  return `
    <div class="semantic-stack">
      ${renderSummaryGrid([
        ['reasoning id', scalar(value.reasoning_id)],
        ['created', scalar(value.created_at)],
        ['next step', scalar(value.next_step)],
      ], tree)}
      ${renderTextPanel('Summary', scalar(value.summary), tree)}
      ${renderMetaRows([
        ['decision', scalar(value.decision_summary)],
        ['continuity', scalar(value.continuity_summary)],
        ['tool intent', scalar(value.tool_intent_summary)],
        ['risk', scalar(value.risk_summary)],
      ], tree)}
      ${renderListPanel('Source Refs', asStringArray(value.source_refs), tree)}
    </div>
  `;
}

function renderToolActivity(items: unknown[], tree: StructuredTreeRenderer): string {
  if (!items.length) return tree.renderPanelValue(items);
  return `
    <div class="semantic-card-list">
      ${items.map((item) => renderToolCard(asRecord(item), tree)).join('')}
    </div>
  `;
}

function renderToolCard(value: JsonRecord, tree: StructuredTreeRenderer): string {
  return `
    <article class="semantic-card">
      <header class="semantic-card-header">
        <div>
          <div class="semantic-card-title">${tree.escapeHtml(scalar(value.title) !== '-' ? scalar(value.title) : scalar(value.tool_name))}</div>
          <div class="semantic-card-subtitle">${tree.escapeHtml(scalar(value.purpose))}</div>
        </div>
        <span class="value-pill ${toolStatusTone(scalar(value.status))}">${tree.escapeHtml(scalar(value.status))}</span>
      </header>
      ${renderSummaryGrid([
        ['tool', scalar(value.tool_name)],
        ['target', scalar(value.target_ref)],
        ['kind', scalar(value.tool_kind)],
        ['object', scalar(value.target_kind)],
      ], tree)}
      ${renderTextPanel('Input Summary', scalar(value.input_summary), tree)}
      ${renderTextPanel('Output Summary', scalar(value.output_summary), tree)}
      ${renderMetaRows([
        ['started', scalar(value.started_at)],
        ['ended', scalar(value.ended_at)],
        ['duration ms', scalar(value.duration_ms)],
        ['error', scalar(value.error_summary)],
      ], tree)}
      ${renderListPanel('Side Effects', asStringArray(value.side_effects), tree)}
      ${renderListPanel('Artifact Refs', asStringArray(value.artifact_refs), tree)}
    </article>
  `;
}

function renderClosureTrace(value: JsonRecord, tree: StructuredTreeRenderer): string {
  if (!Object.keys(value).length) return tree.renderPanelValue(value);
  const raw = scalar(value.provider_raw_output);
  const parsed = parseStructuredOutput(raw);
  return `
    <div class="semantic-stack">
      ${renderSummaryGrid([
        ['closure', scalar(value.closure_id)],
        ['digest', scalar(value.digest_id)],
        ['provider', `${scalar(value.provider_name)} / ${scalar(value.provider_model)}`],
        ['endpoint', scalar(value.provider_endpoint)],
      ], tree)}
      ${renderTextPanel('User Input', scalar(value.user_input), tree)}
      ${renderTextPanel('Assistant Response', scalar(value.assistant_response), tree)}
      ${parsed.answer ? renderTextPanel('Provider User Response Block', parsed.answer, tree) : ''}
      ${parsed.control ? renderTextPanel('Provider Control Block', parsed.control, tree) : ''}
      ${renderTextPanel('Rendered Prompt', scalar(value.rendered_input), tree)}
      ${renderListPanel('Tool Record Paths', asStringArray(value.tool_record_paths), tree)}
      ${renderListPanel('Source Event IDs', asStringArray(value.source_event_ids), tree)}
    </div>
  `;
}

function renderClosedLoopReceipt(value: JsonRecord, tree: StructuredTreeRenderer): string {
  if (!Object.keys(value).length) return tree.renderPanelValue(value);
  const milestones = asArray(value.milestones).map((item) => asRecord(item));
  const blocker = scalar(value.blocker);
  return `
    <div class="semantic-stack">
      ${renderSummaryGrid([
        ['path', scalar(value.pathKind)],
        ['stage', scalar(value.stage)],
        ['session', scalar(value.sessionId)],
        ['task', scalar(value.taskId)],
        ['created', scalar(value.createdTasks)],
        ['assignments', scalar(value.assignments)],
        ['reviews', scalar(value.reviewed)],
      ], tree)}
      ${renderMetaRows([
        ['last action', scalar(value.lastAction)],
        ['next handoff', scalar(value.nextHandoff)],
        ['blocker', blocker],
      ], tree)}
      ${blocker !== '-' ? `<section class="alert-strip error"><strong>Blocked:</strong> ${tree.escapeHtml(blocker)}</section>` : ''}
      ${milestones.length ? `
        <section class="semantic-panel">
          <div class="semantic-panel-title">Milestones</div>
          <div class="timeline-list compact-timeline-list">
            ${milestones.map((milestone) => `
              <article class="timeline-item ${tree.escapeHtml(scalar(milestone.tone) === 'ok' ? 'ok' : 'subtle')} compact-timeline-item">
                <div class="timeline-item-header">
                  <span class="timeline-name">${tree.escapeHtml(scalar(milestone.label))}</span>
                  <span class="timeline-time">${tree.escapeHtml(scalar(milestone.time))}</span>
                </div>
                <div class="timeline-meta"><span>${tree.escapeHtml(scalar(milestone.summary))}</span></div>
              </article>
            `).join('')}
          </div>
        </section>
      ` : ''}
    </div>
  `;
}

function renderFrameworkFlow(value: JsonRecord, tree: StructuredTreeRenderer): string {
  if (!Object.keys(value).length) return tree.renderPanelValue(value);
  const lanes = asArray(value.lanes).map((item) => asRecord(item));
  return `
    <div class="semantic-stack">
      ${renderSummaryGrid([
        ['path', scalar(value.pathKind)],
        ['stage', scalar(value.stage)],
        ['latest event', scalar(value.latestEvent)],
        ['latest at', scalar(value.latestAt)],
        ['event count', scalar(value.eventCount)],
      ], tree)}
      <section class="flow-lane-grid">
        ${lanes.map((lane) => renderFlowLaneCard(lane, tree)).join('')}
      </section>
    </div>
  `;
}

function renderFrameworkTimeline(value: JsonRecord, tree: StructuredTreeRenderer): string {
  const events = asArray(value.events).map((item) => asRecord(item));
  if (!events.length) return tree.renderPanelValue(value);
  const frameworkEvents = events.filter((event) => isFrameworkEvent(scalar(event.event_type) !== '-' ? scalar(event.event_type) : scalar(event.eventType)));
  const blocker = scalar(value.blocker) !== '-' ? scalar(value.blocker) : firstBlocker(frameworkEvents);
  return `
    <div class="semantic-stack">
      ${blocker ? `<section class="alert-strip error"><strong>Current blocker:</strong> ${tree.escapeHtml(blocker)}</section>` : ''}
      ${renderSummaryGrid([
        ['path', scalar(value.pathKind)],
        ['stage', scalar(value.stage)],
        ['latest event', scalar(value.latestEvent)],
        ['latest at', scalar(value.latestAt)],
        ['events', scalar(value.eventCount)],
        ['ops', scalar(value.uniqueOperationCount)],
        ['selected op events', scalar(value.selectedOperationEventCount)],
      ], tree)}
      <section class="semantic-panel">
        <div class="semantic-panel-title">Framework Timeline</div>
        <div class="timeline-list compact-timeline-list">
          ${frameworkEvents.map((event) => renderFrameworkTimelineEvent(event, tree)).join('')}
        </div>
      </section>
    </div>
  `;
}

function renderFlowLaneCard(value: JsonRecord, tree: StructuredTreeRenderer): string {
  const events = asArray(value.events).map((item) => asRecord(item));
  const tone = flowLaneTone(scalar(value.tone));
  return `
    <article class="flow-lane-card ${tree.escapeHtml(tone)}">
      <div class="flow-lane-header">
        <div>
          <div class="flow-lane-title">${tree.escapeHtml(scalar(value.title))}</div>
          <div class="flow-lane-summary">${tree.escapeHtml(scalar(value.summary))}</div>
        </div>
        <span class="flow-lane-pill ${tree.escapeHtml(tone)}">${tree.escapeHtml(scalar(value.status))}</span>
      </div>
      <div class="flow-lane-meta">
        <span>${tree.escapeHtml(scalar(value.lastAt))}</span>
        <span>${tree.escapeHtml(`${events.length} events`)}</span>
      </div>
      <div class="flow-lane-events">
        ${events.length ? events.map((event) => `
          <article class="flow-lane-event">
            <div class="flow-lane-event-head">
              <span class="flow-lane-event-name">${tree.escapeHtml(scalar(event.label))}</span>
              <span class="flow-lane-event-time">${tree.escapeHtml(scalar(event.time))}</span>
            </div>
            <div class="flow-lane-event-summary">${tree.escapeHtml(scalar(event.summary))}</div>
          </article>
        `).join('') : '<div class="empty-state compact"><div class="empty-text">No events</div></div>'}
      </div>
    </article>
  `;
}

function renderFrameworkTimelineEvent(value: JsonRecord, tree: StructuredTreeRenderer): string {
  const eventType = scalar(value.event_type) !== '-' ? scalar(value.event_type) : scalar(value.eventType);
  const tone = frameworkTimelineTone(eventType);
  const operationId = scalar(value.operation_id) !== '-' ? scalar(value.operation_id) : scalar(value.operationId);
  const selected = value.isSelectedOperation === true;
  return `
    <article class="timeline-item ${tree.escapeHtml(tone)} compact-timeline-item framework-timeline-item ${tree.escapeHtml(`framework-${tone}`)} ${selected ? 'selected-operation' : ''}" ${operationId !== '-' ? `data-focus-operation="${tree.escapeHtml(operationId)}"` : ''}>
      <div class="timeline-item-header">
        <span class="timeline-name">${tree.escapeHtml(eventType)}</span>
        <span class="timeline-time">${tree.escapeHtml(scalar(value.occurred_at) !== '-' ? scalar(value.occurred_at) : scalar(value.timestamp))}</span>
      </div>
      <div class="timeline-meta">
        <span>${tree.escapeHtml(frameworkTimelineSummary(value))}</span>
        ${operationId !== '-' ? `<span>op=${tree.escapeHtml(operationId)}</span>` : ''}
        ${selected ? '<span class="timeline-focus-badge">focus</span>' : ''}
      </div>
    </article>
  `;
}

function renderSummaryGrid(lines: Array<[string, string]>, tree: StructuredTreeRenderer): string {
  const valid = lines.filter(([, value]) => value !== '-');
  if (!valid.length) return '';
  return `
    <div class="semantic-summary-grid">
      ${valid.map(([label, value]) => `
        <article class="semantic-summary-item">
          <span class="semantic-summary-label">${tree.escapeHtml(label)}</span>
          <span class="semantic-summary-value">${tree.escapeHtml(value)}</span>
        </article>
      `).join('')}
    </div>
  `;
}

function renderMetaRows(lines: Array<[string, string]>, tree: StructuredTreeRenderer): string {
  const valid = lines.filter(([, value]) => value !== '-');
  if (!valid.length) return '';
  return `
    <div class="semantic-meta-list">
      ${valid.map(([label, value]) => `
        <div class="semantic-meta-row">
          <span class="semantic-meta-key">${tree.escapeHtml(label)}</span>
          <span class="semantic-meta-value">${tree.escapeHtml(value)}</span>
        </div>
      `).join('')}
    </div>
  `;
}

function renderTextPanel(title: string, text: string, tree: StructuredTreeRenderer): string {
  if (text === '-') return '';
  return `
    <section class="semantic-panel">
      <div class="semantic-panel-title">${tree.escapeHtml(title)}</div>
      <div class="text-block compact">${tree.escapeHtml(text)}</div>
    </section>
  `;
}

function renderListPanel(title: string, items: string[], tree: StructuredTreeRenderer): string {
  if (!items.length) return '';
  return `
    <section class="semantic-panel">
      <div class="semantic-panel-title">${tree.escapeHtml(title)}</div>
      <div class="pill-row">
        ${items.map((item) => `<span class="value-pill plain">${tree.escapeHtml(item)}</span>`).join('')}
      </div>
    </section>
  `;
}

function parseStructuredOutput(raw: string): { answer: string | null; control: string | null } {
  return {
    answer: extractTag(raw, 'fin_user_response'),
    control: extractTag(raw, 'fin_control_feedback'),
  };
}

function extractTag(raw: string, tag: string): string | null {
  const match = raw.match(new RegExp(`<${tag}>\\s*([\\s\\S]*?)\\s*</${tag}>`));
  return match?.[1]?.trim() || null;
}

function toolStatusTone(status: string): string {
  if (status.includes('fail') || status.includes('error')) return 'error';
  if (status.includes('complete') || status.includes('success')) return 'good';
  if (status.includes('running') || status.includes('active')) return 'info';
  return 'plain';
}

function flowLaneTone(tone: string): string {
  if (tone === 'error') return 'error';
  if (tone === 'ok') return 'ok';
  return 'neutral';
}

function frameworkTimelineTone(eventType: string): string {
  if (eventType.includes('blocked')) return 'error';
  if (eventType.includes('review') || eventType.includes('submit') || eventType.includes('completed')) return 'ok';
  return 'subtle';
}

function frameworkTimelineSummary(value: JsonRecord): string {
  const payload = asRecord(value.payload);
  const eventType = scalar(value.event_type);
  if (eventType === 'session.formalized') return compactText(`task=${scalar(payload.task_id)} · topic=${scalar(payload.topic_thread_id)}`);
  if (eventType === 'framework.task_kickoff_enqueued') return compactText(`kickoff · ${scalar(payload.goal_summary) !== '-' ? scalar(payload.goal_summary) : scalar(payload.enqueue_reason)}`);
  if (eventType === 'scheduler.tick_decision_recorded') return compactText(`action=${scalar(payload.action_kind)} · pending=${scalar(payload.pending_input_count)}`);
  if (eventType === 'scheduler.tick_blocked') return compactText(`blocked=${scalar(payload.blocked_by)} · ${scalar(payload.result_summary)}`);
  if (eventType === 'supervisor.cycle_completed') return compactText(`blocked=${scalar(payload.blocked_kind)} · next=${scalar(payload.next_wake_hint)}`);
  if (eventType === 'agent.assignment_requested') return compactText(`worker=${scalar(payload.target_worker_id)} · ${scalar(payload.task_summary)}`);
  if (eventType.startsWith('project.task.')) {
    const task = asRecord(payload.task);
    return compactText(`task=${scalar(task.task_id) !== '-' ? scalar(task.task_id) : scalar(payload.task_id)} · status=${scalar(task.status) !== '-' ? scalar(task.status) : scalar(payload.status)}`);
  }
  return compactText(scalar(payload.result_summary) !== '-' ? scalar(payload.result_summary) : scalar(payload.reason));
}

function isFrameworkEvent(eventType: string): boolean {
  return eventType === 'session.formalized'
    || eventType === 'framework.task_kickoff_enqueued'
    || eventType === 'agent.assignment_requested'
    || eventType.startsWith('scheduler.tick_')
    || eventType.startsWith('supervisor.cycle_')
    || eventType.startsWith('project.task.');
}

function firstBlocker(events: JsonRecord[]): string | null {
  for (const event of events) {
    const payload = asRecord(event.payload);
    const eventType = scalar(event.event_type);
    if (eventType === 'scheduler.tick_blocked') {
      return scalar(payload.blocked_by) !== '-' ? scalar(payload.blocked_by) : scalar(payload.result_summary);
    }
    if (eventType === 'supervisor.cycle_completed' && scalar(payload.blocked_kind) !== '-') {
      return compactText(`${scalar(payload.blocked_kind)} · ${scalar(payload.next_wake_hint)}`);
    }
  }
  return null;
}

function compactText(value: string): string {
  return value.length > 140 ? `${value.slice(0, 140)}…` : value;
}

function scalar(value: unknown): string {
  if (value === null || value === undefined) return '-';
  if (typeof value === 'string') return value.trim() || '-';
  return JSON.stringify(value);
}

function asRecord(value: unknown): JsonRecord {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
  return value as JsonRecord;
}

function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function asStringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.map((item) => scalar(item)).filter((item) => item !== '-') : [];
}
