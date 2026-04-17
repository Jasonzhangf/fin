import type { DebugBinding, JsonRecord, ProjectionView, RefreshState, RuntimeEvent } from './types.js';
import { StructuredTreeRenderer } from './tree.js';

export class InspectorPane {
  constructor(
    private readonly rootEl: HTMLElement,
    private readonly tree: StructuredTreeRenderer,
  ) {}

  render(tab: string, state: RefreshState): void {
    let html = '';
    if (tab === 'overview') html = this.renderOverview(state.projection, state.binding);
    if (tab === 'provider') html = this.renderProvider(state.projection, state.lastRun);
    if (tab === 'context') {
      html = state.currentContext
        ? this.tree.renderNode('current_context', state.currentContext)
        : this.empty('No current context yet.');
    }
    if (tab === 'history') html = this.renderRecentContexts(state.recentContexts);
    if (tab === 'events') html = this.renderEvents(state.events);
    if (tab === 'last-run') {
      html = state.lastRun ? this.tree.renderNode('last_run', state.lastRun) : this.empty('No last run yet.');
    }
    if (tab === 'alerts') html = this.renderAlerts(state.projection, state.events);
    this.rootEl.innerHTML = html;
  }

  private renderOverview(projection: ProjectionView, binding: DebugBinding | null): string {
    const rows: Array<[string, unknown]> = [
      ['project_id', binding?.project_id],
      ['project_label', binding?.project_label],
      ['runtime_home', binding?.runtime_home],
      ['session_id', projection.session_id ?? binding?.session_id],
      ['task_id', projection.task_id ?? binding?.task_id],
      ['current_phase', projection.current_phase],
      ['latest_progress_id', projection.latest_progress_id],
      ['latest_note_id', projection.latest_note_id],
      ['latest_digest_id', projection.latest_digest_id],
      ['latest_provider_activity', projection.latest_provider_activity],
    ];

    return `
      <section class="grid-cards">
        ${rows
          .map(
            ([label, value]) => `
              <article class="info-card">
                <span class="label">${this.tree.escapeHtml(label)}</span>
                <span class="value">${this.tree.escapeHtml(this.tree.prettyScalar(value))}</span>
              </article>
            `,
          )
          .join('')}
      </section>
    `;
  }

  private renderProvider(projection: ProjectionView, lastRun: JsonRecord | null): string {
    const headerNames = Array.isArray(projection.latest_provider_header_names)
      ? projection.latest_provider_header_names
      : Array.isArray(lastRun?.provider_header_names)
        ? lastRun.provider_header_names
        : [];
    return `
      <article class="info-card">
        <span class="label">user_agent</span>
        <span class="value">${this.tree.escapeHtml(this.tree.prettyScalar(projection.latest_provider_user_agent ?? lastRun?.provider_user_agent))}</span>
      </article>
      <article class="info-card">
        <span class="label">latest_activity</span>
        <span class="value">${this.tree.escapeHtml(this.tree.prettyScalar(projection.latest_provider_activity))}</span>
      </article>
      ${this.tree.renderNode('header_names', headerNames)}
    `;
  }

  private renderRecentContexts(items: JsonRecord[]): string {
    if (!items.length) return this.empty('No recent context history yet.');
    return `<div class="list-stack">${items
      .slice()
      .reverse()
      .map(
        (item, idx) => `
          <article class="context-card">
            <div class="context-header">
              <span class="context-title">turn ${items.length - idx}</span>
              <span class="muted">${this.tree.escapeHtml(String(item.captured_at ?? '-'))}</span>
            </div>
            ${this.tree.renderNode('context_snapshot', item, false)}
          </article>
        `,
      )
      .join('')}</div>`;
  }

  private renderEvents(events: RuntimeEvent[]): string {
    if (!events.length) return this.empty('No events yet.');
    return `<div class="list-stack">${events
      .slice()
      .reverse()
      .map(
        (event) => `
          <article class="event-card">
            <div class="event-header">
              <span class="event-name">${this.tree.escapeHtml(`${event.sequence ?? '-'} . ${event.event_type ?? '-'}`)}</span>
              <span class="muted">${this.tree.escapeHtml(event.occurred_at ?? event.timestamp ?? '-')}</span>
            </div>
            <div class="event-meta">
              <span>trace=${this.tree.escapeHtml(this.tree.prettyScalar(event.trace_id))}</span>
              <span>session=${this.tree.escapeHtml(this.tree.prettyScalar(event.refs?.session_id))}</span>
              <span>task=${this.tree.escapeHtml(this.tree.prettyScalar(event.refs?.task_id))}</span>
              <span>source=${this.tree.escapeHtml(this.tree.prettyScalar(event.source))}</span>
            </div>
            ${this.tree.renderNode('payload', event.payload ?? {}, false)}
          </article>
        `,
      )
      .join('')}</div>`;
  }

  private renderAlerts(projection: ProjectionView, events: RuntimeEvent[]): string {
    const rows: Array<{ level: 'warn' | 'error'; title: string; detail: string }> = [];
    const warnings = Array.isArray(projection.warnings) ? projection.warnings : [];
    for (const warning of warnings) {
      rows.push({ level: 'warn', title: String(warning), detail: 'projection warning' });
    }
    for (const event of events) {
      const eventType = String(event.event_type ?? '');
      if (eventType.includes('failed') || eventType.includes('timeout')) {
        rows.push({
          level: 'error',
          title: eventType,
          detail: `${event.occurred_at ?? event.timestamp ?? '-'} · trace=${event.trace_id ?? '-'}`,
        });
      }
    }
    if (!rows.length) return this.empty('No warnings yet.');
    return `<div class="list-stack">${rows
      .map(
        (row) => `
          <article class="alert-card ${row.level}">
            <div class="event-header">
              <span class="event-name">${this.tree.escapeHtml(row.title)}</span>
              <span class="muted">${this.tree.escapeHtml(row.detail)}</span>
            </div>
          </article>
        `,
      )
      .join('')}</div>`;
  }

  private empty(text: string): string {
    return `<div class="empty-state"><div class="empty-text">${this.tree.escapeHtml(text)}</div></div>`;
  }
}