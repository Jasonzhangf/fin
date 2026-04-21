import { formatLocalTimestamp } from './time.js';
import { StructuredTreeRenderer } from './tree.js';
import type { RefreshState, RuntimeEvent } from './types.js';

export function renderClosedLoopReceipt(state: RefreshState, tree: StructuredTreeRenderer): string {
  const events = state.sessionEvents;
  const milestones = buildClosedLoopMilestones(events);
  if (!milestones.length) return '';

  const pathKind = detectLoopPath(events);
  const stage = detectLoopStage(events, pathKind);
  const createdTasks = countEvents(events, 'project.task.created');
  const assignments = countEvents(events, 'agent.assignment_requested');
  const reviewed = countEvents(events, 'project.task.review_completed');

  return `
    <section class="selected-request">
      <div class="selected-request-header">
        <div>
          <div class="section-kicker">Closed Loop Receipt</div>
          <h3>${tree.escapeHtml(`${pathKind} path · ${stage}`)}</h3>
        </div>
        <span class="request-state-pill ${tree.escapeHtml(stage === 'closed' ? 'ok' : 'neutral')}">
          ${tree.escapeHtml(stage)}
        </span>
      </div>
      <section class="chip-grid">
        ${renderChip(tree, 'session', state.binding?.session_id ?? '-')}
        ${renderChip(tree, 'task', state.binding?.task_id ?? '-')}
        ${renderChip(tree, 'created', String(createdTasks))}
        ${renderChip(tree, 'assignments', String(assignments))}
        ${renderChip(tree, 'reviews', String(reviewed))}
      </section>
      <div class="timeline-list">
        ${milestones.map((milestone) => `
          <article class="timeline-item ${tree.escapeHtml(milestone.tone)}">
            <div class="timeline-item-header">
              <span class="timeline-name">${tree.escapeHtml(milestone.label)}</span>
              <span class="timeline-time">${tree.escapeHtml(formatLocalTimestamp(milestone.time))}</span>
            </div>
            <div class="timeline-meta">
              <span>${tree.escapeHtml(milestone.summary)}</span>
            </div>
          </article>
        `).join('')}
      </div>
    </section>
  `;
}

export function renderFrameworkTimeline(state: RefreshState, tree: StructuredTreeRenderer): string {
  const items = state.sessionEvents.filter(isFrameworkProgressEvent).slice(-8).reverse();
  if (!items.length) return '';
  return `
    <section class="selected-request">
      <div class="selected-request-header">
        <div>
          <div class="section-kicker">Framework Progress</div>
          <h3>formalize / planning / scheduler / supervisor timeline</h3>
        </div>
        <span class="request-state-pill neutral">${tree.escapeHtml(String(items.length))} events</span>
      </div>
      <div class="timeline-list">
        ${items.map((event) => `
          <article class="timeline-item ${frameworkEventTone(event)}">
            <div class="timeline-item-header">
              <span class="timeline-name">${tree.escapeHtml(String(event.event_type ?? '-'))}</span>
              <span class="timeline-time">${tree.escapeHtml(formatLocalTimestamp(event.occurred_at ?? event.timestamp))}</span>
            </div>
            <div class="timeline-meta">
              <span>source=${tree.escapeHtml(String(event.source ?? '-'))}</span>
              <span>${tree.escapeHtml(frameworkEventSummary(event))}</span>
            </div>
          </article>
        `).join('')}
      </div>
    </section>
  `;
}

type ClosedLoopMilestone = { label: string; summary: string; time?: string; tone: 'ok' | 'subtle' };

function renderChip(tree: StructuredTreeRenderer, label: string, value: string): string {
  return `<article class="summary-chip"><span class="summary-chip-label">${tree.escapeHtml(label)}</span><span class="summary-chip-value">${tree.escapeHtml(value)}</span></article>`;
}

function buildClosedLoopMilestones(events: RuntimeEvent[]): ClosedLoopMilestone[] {
  const milestones: ClosedLoopMilestone[] = [];
  pushMilestone(milestones, findLastEvent(events, 'session.formalized'), 'Formalized', (event) => {
    const payload = asRecord(event.payload);
    return `task=${String(payload.task_id ?? '-')} · topic=${String(payload.topic_thread_id ?? '-')}`;
  });
  pushMilestone(milestones, findLastEvent(events, 'framework.task_kickoff_enqueued'), 'Planning Kickoff', (event) => {
    const payload = asRecord(event.payload);
    return String(payload.goal_summary ?? payload.enqueue_reason ?? '-');
  });
  pushMilestone(milestones, findLastEvent(events, 'plan.updated'), 'Direct Plan', (event) => {
    const payload = asRecord(event.payload);
    return compactSummary(String(payload.explanation ?? payload.summary ?? 'plan updated'));
  });
  pushMilestone(milestones, findLastEvent(events, 'project.task.created'), 'Managed Planning', (event) => {
    const payload = asRecord(event.payload);
    return `task=${String(payload.task_id ?? '-')} · status=${String(payload.status ?? '-')}`;
  });
  pushMilestone(milestones, findLastEvent(events, 'agent.assignment_requested'), 'Owner Dispatch', (event) => {
    const payload = asRecord(event.payload);
    return `worker=${String(payload.target_worker_id ?? '-')} · ${compactSummary(String(payload.task_summary ?? '-'))}`;
  });
  pushMilestone(milestones, findLastEvent(events, 'project.task.claim_completed'), 'Task Claimed', (event) => {
    const task = asRecord(asRecord(event.payload).task);
    return `task=${String(task.task_id ?? '-')} · worker=${String(task.claimed_by_worker_id ?? '-')}`;
  });
  pushMilestone(milestones, findLastEvent(events, 'project.task.submit_completed'), 'Worker Submitted', (event) => {
    const task = asRecord(asRecord(event.payload).task);
    return compactSummary(String(task.result_summary ?? task.task_id ?? 'submitted'));
  });
  pushMilestone(milestones, findLastEvent(events, 'project.task.review_completed'), 'Owner Review', (event) => {
    const task = asRecord(asRecord(event.payload).task);
    return `decision=${String(task.decision ?? '-')} · status=${String(task.status ?? '-')}`;
  }, 'ok');
  return milestones;
}

function pushMilestone(
  items: ClosedLoopMilestone[],
  event: RuntimeEvent | null,
  label: string,
  render: (event: RuntimeEvent) => string,
  tone: 'ok' | 'subtle' = 'subtle',
): void {
  if (!event) return;
  items.push({ label, summary: render(event), time: event.occurred_at ?? event.timestamp, tone });
}

function countEvents(events: RuntimeEvent[], eventType: string): number {
  return events.filter((event) => String(event.event_type ?? '') === eventType).length;
}

function findLastEvent(events: RuntimeEvent[], eventType: string): RuntimeEvent | null {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    if (String(events[index].event_type ?? '') === eventType) return events[index];
  }
  return null;
}

function detectLoopPath(events: RuntimeEvent[]): string {
  if (countEvents(events, 'project.task.created')) return 'managed';
  if (countEvents(events, 'plan.updated')) return 'direct';
  if (countEvents(events, 'framework.task_kickoff_enqueued')) return 'planning';
  return 'tentative';
}

function detectLoopStage(events: RuntimeEvent[], pathKind: string): string {
  if (countEvents(events, 'project.task.review_completed')) return 'closed';
  if (countEvents(events, 'project.task.submit_completed')) return 'awaiting owner review';
  if (countEvents(events, 'project.task.claim_completed')) return 'worker running';
  if (countEvents(events, 'agent.assignment_requested')) return 'dispatching';
  if (pathKind === 'managed' && countEvents(events, 'project.task.created')) return 'managed planned';
  if (pathKind === 'direct' && countEvents(events, 'plan.updated')) return 'direct planned';
  if (countEvents(events, 'framework.task_kickoff_enqueued')) return 'planning';
  if (countEvents(events, 'session.formalized')) return 'formalized';
  return 'tentative';
}

function isFrameworkProgressEvent(event: RuntimeEvent): boolean {
  const eventType = String(event.event_type ?? '');
  const source = String(event.source ?? '');
  return eventType === 'session.formalized'
    || eventType === 'framework.task_kickoff_enqueued'
    || eventType.startsWith('scheduler.tick_')
    || eventType.startsWith('supervisor.cycle_')
    || source === 'cli.session_routing'
    || source === 'cli.scheduler_tick'
    || source === 'cli.supervisor_cycle';
}

function frameworkEventSummary(event: RuntimeEvent): string {
  const payload = asRecord(event.payload);
  const eventType = String(event.event_type ?? '');
  if (eventType === 'session.formalized') return compactSummary(`task=${String(payload.task_id ?? '-')} · topic=${String(payload.topic_thread_id ?? '-')}`);
  if (eventType === 'framework.task_kickoff_enqueued') return compactSummary(`queued planning kickoff · ${String(payload.goal_summary ?? payload.enqueue_reason ?? '-')}`);
  if (eventType === 'scheduler.tick_decision_recorded') return compactSummary(`action=${String(payload.action_kind ?? '-')} · pending=${String(payload.pending_input_count ?? '-')}`);
  if (eventType === 'scheduler.tick_completed') return compactSummary(`final=${String(payload.final_action_kind ?? '-')} · drove=${String(payload.drove_count ?? '-')}`);
  if (eventType === 'supervisor.cycle_completed') return compactSummary(`blocked=${String(payload.blocked_kind ?? '-')} · next=${String(payload.next_wake_hint ?? '-')}`);
  return compactSummary(String(payload.result_summary ?? payload.reason ?? payload.source ?? payload.goal_summary ?? '-'));
}

function frameworkEventTone(event: RuntimeEvent): string {
  const eventType = String(event.event_type ?? '');
  if (eventType.includes('blocked')) return 'error';
  if (eventType === 'session.formalized' || eventType === 'framework.task_kickoff_enqueued' || eventType.endsWith('_completed')) return 'ok';
  return 'subtle';
}

function asRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : {};
}

function compactSummary(input: string): string {
  const text = input.trim();
  if (text.length <= 120) return text || '-';
  return `${text.slice(0, 120)}…`;
}
