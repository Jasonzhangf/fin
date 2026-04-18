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
