import type { DebugBinding, RuntimeEvent } from './types.js';

export interface ClosedLoopMilestoneView {
  label: string;
  summary: string;
  time?: string;
  tone: 'ok' | 'subtle';
}

export interface ClosedLoopReceiptView {
  pathKind: string;
  stage: string;
  createdTasks: number;
  assignments: number;
  reviewed: number;
  blocker: string;
  nextHandoff: string;
  lastAction: string;
  sessionId: string;
  taskId: string;
  milestones: ClosedLoopMilestoneView[];
}

export interface FlowLaneEventView {
  eventType: string;
  label: string;
  time?: string;
  summary: string;
}

export interface FlowLaneView {
  key: 'framework' | 'owner' | 'worker' | 'review';
  title: string;
  status: string;
  summary: string;
  tone: 'ok' | 'neutral' | 'error';
  lastAt?: string;
  events: FlowLaneEventView[];
}

export interface FrameworkFlowView {
  pathKind: string;
  stage: string;
  latestEvent: string;
  latestAt?: string;
  eventCount: number;
  lanes: FlowLaneView[];
}

export interface FrameworkTimelineEventView {
  eventType: string;
  label: string;
  time?: string;
  summary: string;
  tone: 'ok' | 'subtle' | 'error';
  operationId?: string;
}

export interface FrameworkSessionTimelineView {
  pathKind: string;
  stage: string;
  latestEvent: string;
  latestAt?: string;
  blocker: string;
  eventCount: number;
  uniqueOperationCount: number;
  selectedOperationEventCount: number;
  events: FrameworkTimelineEventView[];
}

export function buildClosedLoopReceiptView(events: RuntimeEvent[], binding: DebugBinding | null): ClosedLoopReceiptView | null {
  const milestones = buildClosedLoopMilestones(events);
  if (!milestones.length) return null;
  const pathKind = detectLoopPath(events);
  const stage = detectLoopStage(events, pathKind);
  return {
    pathKind,
    stage,
    createdTasks: countEvents(events, 'project.task.created'),
    assignments: countEvents(events, 'agent.assignment_requested'),
    reviewed: countEvents(events, 'project.task.review_completed'),
    blocker: detectCurrentBlocker(events),
    nextHandoff: detectNextHandoff(events, pathKind, stage),
    lastAction: detectLastActionable(events),
    sessionId: binding?.session_id ?? '-',
    taskId: binding?.task_id ?? '-',
    milestones,
  };
}

export function buildFrameworkFlowView(events: RuntimeEvent[]): FrameworkFlowView | null {
  const filtered = events.filter(isFrameworkFlowEvent);
  if (!filtered.length) return null;
  const pathKind = detectLoopPath(events);
  const stage = detectLoopStage(events, pathKind);
  const latest = filtered[filtered.length - 1];
  return {
    pathKind,
    stage,
    latestEvent: humanizeEventName(String(latest?.event_type ?? '-')),
    latestAt: latest?.occurred_at ?? latest?.timestamp,
    eventCount: filtered.length,
    lanes: buildFlowLanes(filtered, pathKind, stage),
  };
}

export function buildFrameworkSessionTimelineView(
  events: RuntimeEvent[],
  selectedOperationId: string | null,
): FrameworkSessionTimelineView | null {
  const filtered = events.filter(isFrameworkFlowEvent);
  if (!filtered.length) return null;
  const pathKind = detectLoopPath(events);
  const stage = detectLoopStage(events, pathKind);
  const latest = filtered[filtered.length - 1];
  const operationIds = new Set(
    filtered
      .map((event) => String(event.operation_id ?? '').trim())
      .filter((value) => value.length > 0),
  );
  return {
    pathKind,
    stage,
    latestEvent: humanizeEventName(String(latest?.event_type ?? '-')),
    latestAt: latest?.occurred_at ?? latest?.timestamp,
    blocker: detectCurrentBlocker(events),
    eventCount: filtered.length,
    uniqueOperationCount: operationIds.size,
    selectedOperationEventCount: selectedOperationId
      ? filtered.filter((event) => String(event.operation_id ?? '') === selectedOperationId).length
      : 0,
    events: filtered
      .slice(-24)
      .reverse()
      .map((event) => ({
        eventType: String(event.event_type ?? '-'),
        label: humanizeEventName(String(event.event_type ?? '-')),
        time: event.occurred_at ?? event.timestamp,
        summary: flowEventSummary(event),
        tone: frameworkTimelineTone(String(event.event_type ?? '-')),
        operationId: event.operation_id,
      })),
  };
}

function buildClosedLoopMilestones(events: RuntimeEvent[]): ClosedLoopMilestoneView[] {
  const milestones: ClosedLoopMilestoneView[] = [];
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

function buildFlowLanes(events: RuntimeEvent[], pathKind: string, stage: string): FlowLaneView[] {
  const laneConfigs = [
    { key: 'framework', title: 'Framework' },
    { key: 'owner', title: 'Owner' },
    { key: 'worker', title: 'Worker' },
    { key: 'review', title: 'Review' },
  ] as const;
  return laneConfigs.map((lane) => {
    const laneEvents = events.filter((event) => classifyFlowLane(event) === lane.key).slice(-4).reverse();
    const last = laneEvents[0] ?? null;
    return {
      key: lane.key,
      title: lane.title,
      status: laneStatus(lane.key, stage, laneEvents.length > 0),
      summary: laneSummary(lane.key, stage, pathKind, last),
      tone: laneTone(lane.key, stage, last),
      lastAt: last?.occurred_at ?? last?.timestamp,
      events: laneEvents.map((event) => ({
        eventType: String(event.event_type ?? '-'),
        label: humanizeEventName(String(event.event_type ?? '-')),
        time: event.occurred_at ?? event.timestamp,
        summary: flowEventSummary(event),
      })),
    };
  });
}

function pushMilestone(
  items: ClosedLoopMilestoneView[],
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

function detectCurrentBlocker(events: RuntimeEvent[]): string {
  const blocked = findLastMatchingEvent(events, (event) => {
    const eventType = String(event.event_type ?? '');
    return eventType === 'scheduler.tick_blocked' || eventType === 'supervisor.cycle_completed';
  });
  if (!blocked) return '-';
  const payload = asRecord(blocked.payload);
  const blockedKind = String(payload.blocked_kind ?? payload.blocked_by ?? '').trim();
  const nextWakeHint = String(payload.next_wake_hint ?? '').trim();
  if (blockedKind && blockedKind !== '-') return compactSummary(`${blockedKind}${nextWakeHint ? ` · ${nextWakeHint}` : ''}`);
  if (String(blocked.event_type ?? '') === 'scheduler.tick_blocked') return compactSummary(String(payload.result_summary ?? 'scheduler blocked'));
  return '-';
}

function detectNextHandoff(events: RuntimeEvent[], pathKind: string, stage: string): string {
  if (stage === 'closed') return 'none';
  if (stage === 'awaiting owner review') return 'owner should review submitted task';
  if (stage === 'worker running') return 'worker should submit result';
  if (stage === 'dispatching') return 'worker should claim + execute';
  if (stage === 'managed planned') return 'owner should dispatch ready task';
  if (stage === 'direct planned') return 'current agent should execute direct slice';
  if (stage === 'planning') return pathKind === 'managed' ? 'managed path planning continues' : 'framework should choose direct vs managed';
  if (stage === 'formalized') return 'framework planning kickoff';
  return 'formalize or continue current task';
}

function detectLastActionable(events: RuntimeEvent[]): string {
  const last = findLastMatchingEvent(events, (event) => isFrameworkFlowEvent(event) || String(event.event_type ?? '') === 'plan.updated');
  if (!last) return '-';
  return `${humanizeEventName(String(last.event_type ?? '-'))} · ${flowEventSummary(last)}`;
}

function findLastMatchingEvent(events: RuntimeEvent[], predicate: (event: RuntimeEvent) => boolean): RuntimeEvent | null {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    if (predicate(events[index])) return events[index];
  }
  return null;
}

function isFrameworkFlowEvent(event: RuntimeEvent): boolean {
  const eventType = String(event.event_type ?? '');
  const source = String(event.source ?? '');
  return eventType === 'session.formalized'
    || eventType === 'framework.task_kickoff_enqueued'
    || eventType === 'plan.updated'
    || eventType === 'project.task.created'
    || eventType === 'agent.assignment_requested'
    || eventType === 'project.task.claim_completed'
    || eventType === 'project.task.submit_completed'
    || eventType === 'project.task.review_completed'
    || eventType.startsWith('scheduler.tick_')
    || eventType.startsWith('supervisor.cycle_')
    || eventType.includes('assignment_runtime_resume')
    || source === 'cli.session_routing'
    || source === 'cli.scheduler_tick'
    || source === 'cli.supervisor_cycle';
}

function classifyFlowLane(event: RuntimeEvent): 'framework' | 'owner' | 'worker' | 'review' | 'ignore' {
  const eventType = String(event.event_type ?? '');
  if (eventType === 'project.task.review_completed') return 'review';
  if (eventType === 'project.task.claim_completed' || eventType === 'project.task.submit_completed' || eventType.includes('assignment_runtime_resume')) return 'worker';
  if (eventType === 'plan.updated' || eventType === 'project.task.created' || eventType === 'agent.assignment_requested' || eventType === 'scheduler.tick_owner_loop_action_recorded') return 'owner';
  if (eventType === 'session.formalized' || eventType === 'framework.task_kickoff_enqueued' || eventType.startsWith('scheduler.tick_') || eventType.startsWith('supervisor.cycle_')) return 'framework';
  return 'ignore';
}

function laneStatus(lane: string, stage: string, hasEvents: boolean): string {
  if (!hasEvents) return 'pending';
  if (stage === 'closed') return lane === 'review' ? 'done' : 'observed';
  if (lane === 'framework') return ['formalized', 'planning', 'managed planned', 'direct planned'].includes(stage) ? 'active' : 'done';
  if (lane === 'owner') return ['managed planned', 'dispatching', 'awaiting owner review'].includes(stage) ? 'active' : stage === 'closed' ? 'done' : hasEvents ? 'observed' : 'pending';
  if (lane === 'worker') return stage === 'worker running' ? 'active' : countAsDone(stage, 'worker') ? 'done' : 'pending';
  if (lane === 'review') return stage === 'awaiting owner review' ? 'active' : stage === 'closed' ? 'done' : 'pending';
  return 'observed';
}

function countAsDone(stage: string, lane: string): boolean {
  if (lane === 'worker') return ['awaiting owner review', 'closed'].includes(stage);
  return false;
}

function laneTone(lane: string, stage: string, last: RuntimeEvent | null): 'ok' | 'neutral' | 'error' {
  const eventType = String(last?.event_type ?? '');
  if (eventType.includes('blocked')) return 'error';
  const status = laneStatus(lane, stage, Boolean(last));
  return status === 'pending' ? 'neutral' : 'ok';
}

function laneSummary(lane: string, stage: string, pathKind: string, last: RuntimeEvent | null): string {
  if (!last) {
    if (lane === 'framework') return pathKind === 'tentative' ? 'waiting for formalize / kickoff' : 'no framework events yet';
    if (lane === 'owner') return pathKind === 'managed' ? 'waiting for managed planning / dispatch' : 'owner lane not needed yet';
    if (lane === 'worker') return 'waiting for assignment';
    return 'waiting for submission';
  }
  if (lane === 'framework') return stage === 'closed' ? 'framework chain observed end-to-end' : `latest ${humanizeEventName(String(last.event_type ?? '-'))}`;
  if (lane === 'owner') return stage === 'awaiting owner review' ? 'owner should review submitted task' : flowEventSummary(last);
  if (lane === 'worker') return stage === 'worker running' ? 'worker currently owns execution slice' : flowEventSummary(last);
  return stage === 'closed' ? 'review finished and task closed' : flowEventSummary(last);
}

function flowEventSummary(event: RuntimeEvent): string {
  const payload = asRecord(event.payload);
  const eventType = String(event.event_type ?? '');
  if (eventType === 'session.formalized') return compactSummary(`task=${String(payload.task_id ?? '-')} · topic=${String(payload.topic_thread_id ?? '-')}`);
  if (eventType === 'framework.task_kickoff_enqueued') return compactSummary(`kickoff queued · ${String(payload.goal_summary ?? payload.enqueue_reason ?? '-')}`);
  if (eventType === 'plan.updated') return compactSummary(String(payload.explanation ?? payload.summary ?? 'plan updated'));
  if (eventType === 'project.task.created') return compactSummary(`task=${String(payload.task_id ?? '-')} · status=${String(payload.status ?? '-')}`);
  if (eventType === 'agent.assignment_requested') return compactSummary(`worker=${String(payload.target_worker_id ?? '-')} · ${String(payload.task_summary ?? '-')}`);
  if (eventType === 'project.task.claim_completed' || eventType === 'project.task.submit_completed' || eventType === 'project.task.review_completed') {
    const task = asRecord(payload.task);
    return compactSummary(`task=${String(task.task_id ?? payload.task_id ?? '-')} · status=${String(task.status ?? payload.status ?? '-')} · ${String(task.latest_submission_summary ?? task.latest_review_summary ?? task.decision ?? '')}`);
  }
  if (eventType === 'scheduler.tick_decision_recorded') return compactSummary(`action=${String(payload.action_kind ?? '-')} · pending=${String(payload.pending_input_count ?? '-')}`);
  if (eventType === 'scheduler.tick_drove_pending') return compactSummary(`drove=${String(payload.drove_count ?? '-')} · final=${String(payload.final_action_kind ?? '-')}`);
  if (eventType === 'scheduler.tick_blocked') return compactSummary(`blocked=${String(payload.blocked_by ?? '-')} · ${String(payload.result_summary ?? '-')}`);
  if (eventType === 'scheduler.tick_completed') return compactSummary(`final=${String(payload.final_action_kind ?? '-')} · drove=${String(payload.drove_count ?? '-')}`);
  if (eventType === 'supervisor.cycle_completed') return compactSummary(`blocked=${String(payload.blocked_kind ?? '-')} · next=${String(payload.next_wake_hint ?? '-')}`);
  return compactSummary(String(payload.result_summary ?? payload.reason ?? payload.summary ?? payload.source ?? '-'));
}

export function humanizeEventName(eventType: string): string {
  const map: Record<string, string> = {
    'session.formalized': 'formalized',
    'framework.task_kickoff_enqueued': 'kickoff queued',
    'plan.updated': 'plan updated',
    'project.task.created': 'task created',
    'agent.assignment_requested': 'assignment requested',
    'project.task.claim_completed': 'task claimed',
    'project.task.submit_completed': 'task submitted',
    'project.task.review_completed': 'review completed',
    'scheduler.tick_started': 'scheduler started',
    'scheduler.tick_owner_loop_action_recorded': 'owner action',
    'scheduler.tick_decision_recorded': 'scheduler decision',
    'scheduler.tick_drove_pending': 'scheduler drove pending',
    'scheduler.tick_blocked': 'scheduler blocked',
    'scheduler.tick_completed': 'scheduler completed',
    'supervisor.cycle_started': 'supervisor started',
    'supervisor.cycle_completed': 'supervisor completed',
  };
  return map[eventType] ?? eventType;
}

export function frameworkTimelineTone(eventType: string): 'ok' | 'subtle' | 'error' {
  if (eventType.includes('blocked')) return 'error';
  if (
    eventType.includes('review')
    || eventType.includes('submit')
    || eventType.includes('claim')
    || eventType.includes('completed')
  ) return 'ok';
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
