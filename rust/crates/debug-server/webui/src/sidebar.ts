import type { JsonRecord, RefreshState } from './types.js';
import { StructuredTreeRenderer } from './tree.js';

export type SidebarSectionId = 'project' | 'session' | 'skills' | 'plugins';

interface SidebarSection {
  id: SidebarSectionId;
  kicker: string;
  title: string;
  summary: string;
  facts: Array<[string, string]>;
  list?: Array<{ title: string; meta: string }>;
  detailTitle: string;
  detailSubtitle: string;
}

export function renderSidebar(
  rootEl: HTMLElement,
  state: RefreshState,
  tree: StructuredTreeRenderer,
  openedSectionId: SidebarSectionId | null,
): void {
  const sections = buildSidebarSections(state);
  const openedSection = sections.find((section) => section.id === openedSectionId) ?? null;

  rootEl.innerHTML = `
    ${sections.map((section) => renderSidebarSection(section, tree)).join('')}
    ${openedSection ? renderSidebarModal(openedSection, tree) : ''}
  `;
}

function buildSidebarSections(state: RefreshState): SidebarSection[] {
  const context = asRecord(state.currentContext?.context);
  const project = asRecord(context.project);
  const rolePrompt = asRecord(context.role_prompt);
  const tools = asRecord(context.tools);
  const promptModules = asRecordArray(rolePrompt.prompt_modules);
  const skillSummary = scalar(
    promptModules.find((module) => scalar(module.module_id) === 'stable_core.loaded_global_skill_index')?.summary,
  );
  const promptLayers = asRecordArray(rolePrompt.prompt_layers);
  const activeLayerSummary = promptLayers.length
    ? promptLayers.map((layer) => `${scalar(layer.layer_id)}:${arrayCount(layer.module_ids)}`).join(' · ')
    : '-';
  const recentTurns = [...state.focusTurns].slice(-5).reverse();
  const projectRoot = scalar(project.project_root ?? project.cwd ?? state.binding?.runtime_home);
  const selectedOperation = state.selectedOperationId ?? recentTurns[0]?.operationId ?? '-';

  return [
    {
      id: 'project',
      kicker: 'Project Settings',
      title: scalar(project.project_label ?? state.binding?.project_label ?? 'fin'),
      summary: shortText(scalar(project.scope_summary ?? project.focus_summary), 92),
      facts: [
        ['root', shortPath(projectRoot)],
        ['runtime', shortPath(state.binding?.runtime_home)],
        ['provider', scalar(state.lastRun?.provider)],
        ['model', scalar(state.lastRun?.model)],
      ],
      detailTitle: 'Project Settings',
      detailSubtitle: '当前 project / runtime 绑定摘要',
    },
    {
      id: 'session',
      kicker: 'Session',
      title: scalar(state.binding?.session_id ?? 'tentative'),
      summary: shortText(
        `task=${scalar(state.binding?.task_id)} · messages=${state.messages.length} · turns=${state.focusTurns.length}`,
        92,
      ),
      facts: [
        ['active turn', shortText(selectedOperation, 24)],
        ['recent digests', String(state.recentDigests.length)],
        ['reasoning', String(state.recentReasoningViews.length)],
        ['closures', String(state.recentClosures.length)],
      ],
      list: recentTurns.map((turn) => ({
        title: shortText(turn.userMessage?.content ?? turn.assistantMessage?.content ?? turn.operationId, 54),
        meta: shortText(turn.operationId, 36),
      })),
      detailTitle: 'Session Detail',
      detailSubtitle: '当前 session 与最近 turn 摘要',
    },
    {
      id: 'skills',
      kicker: 'Skills',
      title: skillSummary !== '-' ? shortText(skillSummary, 36) : 'Runtime skill summary pending',
      summary: shortText(
        `layers=${activeLayerSummary} · behavior-rules=${arrayCount(rolePrompt.behavior_rules)} · modules=${promptModules.length}`,
        92,
      ),
      facts: [
        ['skill index', skillSummary !== '-' ? shortText(skillSummary, 28) : 'pending'],
        ['prompt layers', String(promptLayers.length)],
        ['modules', String(promptModules.length)],
        ['contract', shortText(scalar(firstArrayItem(rolePrompt.output_contract)), 20)],
      ],
      detailTitle: 'Skills Detail',
      detailSubtitle: 'prompt layers / modules / output contract 摘要',
    },
    {
      id: 'plugins',
      kicker: 'Plugins & Tools',
      title: 'Tool plane / plugin surface',
      summary: '首页只显示工具平面 digest；完整 guard 列表点开查看。',
      facts: [
        ['model tools', String(arrayCount(tools.model_tools))],
        ['framework', String(arrayCount(tools.framework_tools))],
        ['disabled', String(arrayCount(tools.disabled_tools))],
        ['guards', String(arrayCount(tools.hard_guards))],
      ],
      list: asScalarArray(tools.hard_guards).slice(0, 8).map((guard) => ({
        title: shortText(guard, 54),
        meta: 'guard',
      })),
      detailTitle: 'Plugins & Tools',
      detailSubtitle: 'tool counts / hard guards / plugin surface 摘要',
    },
  ];
}

function renderSidebarSection(section: SidebarSection, tree: StructuredTreeRenderer): string {
  return `
    <section class="rail-section">
      <div class="rail-section-kicker">${tree.escapeHtml(section.kicker)}</div>
      <button
        class="rail-digest-card"
        type="button"
        data-open-rail-section="${tree.escapeHtml(section.id)}"
      >
        <div class="rail-digest-header">
          <div class="rail-card-title">${tree.escapeHtml(section.title)}</div>
          <span class="rail-digest-open">Open</span>
        </div>
        <div class="rail-card-summary">${tree.escapeHtml(section.summary)}</div>
        <div class="rail-digest-facts">
          ${section.facts.slice(0, 2).map(([label, value]) => `
            <article class="rail-digest-pill">
              <span class="rail-fact-label">${tree.escapeHtml(label)}</span>
              <span class="rail-fact-value">${tree.escapeHtml(String(value))}</span>
            </article>
          `).join('')}
        </div>
      </button>
    </section>
  `;
}

function renderSidebarModal(section: SidebarSection, tree: StructuredTreeRenderer): string {
  return `
    <div class="detail-modal-backdrop rail-detail-backdrop" data-close-rail-backdrop>
      <section class="detail-modal rail-detail-modal" role="dialog" aria-modal="true">
        <header class="detail-modal-header">
          <div>
            <div class="section-kicker">${tree.escapeHtml(section.kicker)}</div>
            <h3>${tree.escapeHtml(section.detailTitle)}</h3>
            <p class="muted">${tree.escapeHtml(section.detailSubtitle)}</p>
          </div>
          <button class="detail-modal-close" type="button" data-close-rail-detail>Close</button>
        </header>
        <div class="detail-modal-body">
          <div class="detail-modal-summary">
            ${section.facts.map(([label, value]) => `
              <article class="summary-chip">
                <span class="summary-chip-label">${tree.escapeHtml(label)}</span>
                <span class="summary-chip-value">${tree.escapeHtml(value)}</span>
              </article>
            `).join('')}
          </div>
          ${section.list?.length ? `
            <section class="rail-detail-list">
              ${section.list.map((item) => `
                <article class="rail-detail-item">
                  <div class="rail-inline-title">${tree.escapeHtml(item.title)}</div>
                  <div class="rail-inline-meta">${tree.escapeHtml(item.meta)}</div>
                </article>
              `).join('')}
            </section>
          ` : ''}
        </div>
      </section>
    </div>
  `;
}

function asRecord(value: unknown): JsonRecord {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
  return value as JsonRecord;
}

function asRecordArray(value: unknown): JsonRecord[] {
  return Array.isArray(value)
    ? value.filter((item): item is JsonRecord => Boolean(item) && typeof item === 'object' && !Array.isArray(item))
    : [];
}

function asScalarArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.map((item) => scalar(item)).filter((item) => item !== '-')
    : [];
}

function scalar(value: unknown): string {
  if (value === null || value === undefined) return '-';
  if (typeof value === 'string') return value.trim() || '-';
  return JSON.stringify(value);
}

function arrayCount(value: unknown): number {
  return Array.isArray(value) ? value.length : 0;
}

function firstArrayItem(value: unknown): unknown {
  return Array.isArray(value) && value.length ? value[0] : undefined;
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
  return parts.length <= 4 ? text : `…/${parts.slice(-4).join('/')}`;
}
