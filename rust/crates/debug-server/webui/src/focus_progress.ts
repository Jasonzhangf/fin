import { buildClosedLoopReceiptView, buildFrameworkFlowView } from './framework_flow_view.js';
import { formatLocalTimestamp } from './time.js';
import { StructuredTreeRenderer } from './tree.js';
import type { RefreshState } from './types.js';

export function renderClosedLoopReceipt(state: RefreshState, tree: StructuredTreeRenderer): string {
  const receipt = buildClosedLoopReceiptView(state.sessionEvents, state.binding);
  if (!receipt) return '';

  return `
    <section class="selected-request">
      <div class="selected-request-header">
        <div>
          <div class="section-kicker">Closed Loop Receipt</div>
          <h3>${tree.escapeHtml(`${receipt.pathKind} path · ${receipt.stage}`)}</h3>
        </div>
        <span class="request-state-pill ${tree.escapeHtml(receipt.stage === 'closed' ? 'ok' : receipt.blocker === '-' ? 'neutral' : 'error')}">
          ${tree.escapeHtml(receipt.stage)}
        </span>
      </div>
      <section class="chip-grid compact-grid">
        ${renderChip(tree, 'session', receipt.sessionId)}
        ${renderChip(tree, 'task', receipt.taskId)}
        ${renderChip(tree, 'created', String(receipt.createdTasks))}
        ${renderChip(tree, 'assignments', String(receipt.assignments))}
        ${renderChip(tree, 'reviews', String(receipt.reviewed))}
      </section>
      <section class="receipt-digest-grid">
        ${renderDigestStat(tree, 'last action', receipt.lastAction)}
        ${renderDigestStat(tree, 'next handoff', receipt.nextHandoff)}
        ${renderDigestStat(tree, 'blocker', receipt.blocker)}
      </section>
      <div class="timeline-list compact-timeline-list">
        ${receipt.milestones.map((milestone) => `
          <article class="timeline-item ${tree.escapeHtml(milestone.tone)} compact-timeline-item">
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
  const flow = buildFrameworkFlowView(state.sessionEvents);
  if (!flow) return '';

  return `
    <section class="selected-request">
      <div class="selected-request-header">
        <div>
          <div class="section-kicker">Framework Flow</div>
          <h3>framework / owner / worker / review</h3>
        </div>
        <span class="request-state-pill neutral">${tree.escapeHtml(String(flow.eventCount))} events</span>
      </div>
      <section class="receipt-digest-grid">
        ${renderDigestStat(tree, 'current stage', flow.stage)}
        ${renderDigestStat(tree, 'latest event', flow.latestEvent)}
        ${renderDigestStat(tree, 'latest at', formatLocalTimestamp(flow.latestAt))}
      </section>
      <section class="flow-lane-grid">
        ${flow.lanes.map((lane) => `
          <article class="flow-lane-card ${tree.escapeHtml(lane.tone)}">
            <div class="flow-lane-header">
              <div>
                <div class="flow-lane-title">${tree.escapeHtml(lane.title)}</div>
                <div class="flow-lane-summary">${tree.escapeHtml(lane.summary)}</div>
              </div>
              <span class="flow-lane-pill ${tree.escapeHtml(lane.tone)}">${tree.escapeHtml(lane.status)}</span>
            </div>
            <div class="flow-lane-meta">
              <span>${tree.escapeHtml(lane.lastAt ? formatLocalTimestamp(lane.lastAt) : '-')}</span>
              <span>${tree.escapeHtml(`${lane.events.length} events`)}</span>
            </div>
            <div class="flow-lane-events">
              ${lane.events.map((event) => `
                <article class="flow-lane-event">
                  <div class="flow-lane-event-head">
                    <span class="flow-lane-event-name">${tree.escapeHtml(event.label)}</span>
                    <span class="flow-lane-event-time">${tree.escapeHtml(formatLocalTimestamp(event.time))}</span>
                  </div>
                  <div class="flow-lane-event-summary">${tree.escapeHtml(event.summary)}</div>
                </article>
              `).join('')}
            </div>
          </article>
        `).join('')}
      </section>
    </section>
  `;
}

function renderChip(tree: StructuredTreeRenderer, label: string, value: string): string {
  return `<article class="summary-chip"><span class="summary-chip-label">${tree.escapeHtml(label)}</span><span class="summary-chip-value">${tree.escapeHtml(value)}</span></article>`;
}

function renderDigestStat(tree: StructuredTreeRenderer, label: string, value: string): string {
  return `<article class="digest-line"><span class="digest-line-label">${tree.escapeHtml(label)}</span><span class="digest-line-value">${tree.escapeHtml(value)}</span></article>`;
}
