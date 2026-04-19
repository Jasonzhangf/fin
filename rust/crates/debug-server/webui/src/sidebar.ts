import type { JsonRecord, RefreshState, TurnRecord } from './types.js';
import { StructuredTreeRenderer } from './tree.js';

export type SidebarSectionId = 'project' | 'session' | 'tasks' | 'execution' | 'skills' | 'plugins';

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
  expandedSectionId: SidebarSectionId | null,
  openedSectionId: SidebarSectionId | null,
): void {
  const sections = buildSidebarSections(state);
  const openedSection = sections.find((section) => section.id === openedSectionId) ?? null;

  rootEl.innerHTML = `
    ${sections.map((section) => renderSidebarSection(section, tree, section.id === expandedSectionId)).join('')}
    ${openedSection ? renderSidebarModal(openedSection, tree) : ''}
  `;
}

function buildSidebarSections(state: RefreshState): SidebarSection[] {
  const context = asRecord(state.currentContext?.context);
  const project = asRecord(context.project);
  const control = asRecord(context.control);
  const rolePrompt = asRecord(context.role_prompt);
  const tools = asRecord(context.tools);
  const execution = asRecord(state.currentExecutionState);
  const pauseCheckpoint = asRecord(state.currentPauseCheckpoint);
  const interruptedSegment = asRecord(state.currentInterruptedSegment);
  const segmentMerge = asRecord(state.currentSegmentMerge);
  const routingDecision = asRecord(state.currentRoutingDecision);
  const promptModules = asRecordArray(rolePrompt.prompt_modules);
  const loadedSkillSummary = scalar(
    promptModules.find((module) => scalar(module.module_id) === 'stable_core.loaded_global_skill_index')?.summary,
  );
  const loadedSkillIds = parseLoadedSkillIds(loadedSkillSummary);
  const loadedSkillCount = parseLoadedSkillCount(loadedSkillSummary);
  const promptLayers = asRecordArray(rolePrompt.prompt_layers);
  const activeLayerSummary = promptLayers.length
    ? promptLayers.map((layer) => `${scalar(layer.layer_id)}:${arrayCount(layer.module_ids)}`).join(' · ')
    : '-';
  const modelTools = asRecordArray(tools.model_tools);
  const frameworkTools = asRecordArray(tools.framework_tools);
  const disabledTools = asScalarArray(tools.disabled_tools);
  const hardGuards = asScalarArray(tools.hard_guards);
  const selectionPolicies = asScalarArray(tools.tool_selection_policy);
  const recentFocusTurns = [...state.focusTurns].slice(-5).reverse();
  const recentTurnRecords = [...state.recentTurns].slice(-5).reverse();
  const currentTaskId = scalar(control.task_id ?? state.binding?.task_id);
  const candidateTaskId = scalar(routingDecision.candidate_task_id);
  const topicThreadId = scalar(control.topic_thread_id ?? routingDecision.candidate_topic_thread_id);
  const pendingInputs = Array.isArray(state.currentPendingInputs) ? state.currentPendingInputs : [];
  const projectRoot = scalar(project.project_root ?? project.cwd ?? state.binding?.runtime_home);
  const selectedOperation = state.selectedOperationId ?? recentFocusTurns[0]?.operationId ?? '-';
  const executionStatus = scalar(execution.status);
  const activeStep = scalar(execution.active_step_id);
  const pendingCount = pendingInputs.length || Number(execution.pending_input_count ?? 0);
  const routingDisposition = scalar(routingDecision.disposition);
  const roleId = scalar(rolePrompt.role_id);
  const taskTurns = turnsForTask(recentTurnRecords, currentTaskId);
  const taskSummary = buildTaskSummary(currentTaskId, candidateTaskId, topicThreadId, routingDisposition);
  const promptSourceItems = [
    ...loadedSkillIds.map((skillId) => ({
      title: shortText(skillId, 54),
      meta: 'loaded global skill',
    })),
    ...promptLayers.slice(0, 3).map((layer) => ({
      title: shortText(scalar(layer.title), 54),
      meta: shortText(
        `${scalar(layer.layer_id)} · source=${scalar(layer.source)} · modules=${arrayCount(layer.module_ids)}`,
        60,
      ),
    })),
    ...asScalarArray(rolePrompt.behavior_rules).slice(0, loadedSkillIds.length ? 2 : 4).map((rule) => ({
      title: shortText(rule, 54),
      meta: 'behavior rule',
    })),
  ];
  const capabilityItems = [
    ...modelTools.slice(0, 4).map((tool) => capabilityItemFromTool(tool, 'model tool')),
    ...frameworkTools.slice(0, 2).map((tool) => capabilityItemFromTool(tool, 'framework tool')),
    ...selectionPolicies.slice(0, 2).map((policy) => ({
      title: shortText(policy, 54),
      meta: 'selection policy',
    })),
    ...disabledTools.slice(0, 2).map((toolName) => ({
      title: shortText(toolName, 54),
      meta: 'disabled',
    })),
    ...hardGuards.slice(0, 2).map((guard) => ({
      title: shortText(guard, 54),
      meta: 'guard',
    })),
  ];

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
        `task=${scalar(state.binding?.task_id)} · state=${executionStatus} · turns=${state.recentTurns.length || state.focusTurns.length}`,
        92,
      ),
      facts: [
        ['active turn', shortText(selectedOperation, 24)],
        ['routing', shortText(routingDisposition, 28)],
        ['recent digests', String(state.recentDigests.length)],
        ['reasoning', String(state.recentReasoningViews.length)],
        ['closures', String(state.recentClosures.length)],
      ],
      list: recentTurnRecords.length
        ? recentTurnRecords.map((turn) => ({
          title: shortText(scalar(turn.user_input ?? turn.assistant_visible_output ?? turn.operation_id), 54),
          meta: shortText(`${scalar(turn.operation_id)} · ${scalar(turn.status)}`, 48),
        }))
        : recentFocusTurns.map((turn) => ({
          title: shortText(turn.userMessage?.content ?? turn.assistantMessage?.content ?? turn.operationId, 54),
          meta: shortText(turn.operationId, 36),
        })),
      detailTitle: 'Session Detail',
      detailSubtitle: '当前 session、routing 与最近 turn 摘要',
    },
    {
      id: 'tasks',
      kicker: 'Tasks',
      title: currentTaskId !== '-' ? currentTaskId : 'tentative / no bound task',
      summary: shortText(taskSummary, 92),
      facts: [
        ['current', shortText(currentTaskId, 28)],
        ['candidate', shortText(candidateTaskId, 28)],
        ['topic', shortText(topicThreadId, 28)],
        ['confirm', routingDecision.requires_user_confirmation === true ? 'required' : 'no'],
      ],
      list: [
        ...(candidateTaskId !== '-' && candidateTaskId !== currentTaskId
          ? [{
            title: shortText(`candidate ${candidateTaskId}`, 54),
            meta: shortText(`routing · ${routingDisposition} · confirm=${routingDecision.requires_user_confirmation === true ? 'yes' : 'no'}`, 60),
          }]
          : []),
        ...[
          scalar(routingDecision.current_topic_summary) !== '-' ? {
            title: shortText(scalar(routingDecision.current_topic_summary), 54),
            meta: 'current topic summary',
          } : null,
          scalar(routingDecision.previous_topic_summary) !== '-' ? {
            title: shortText(scalar(routingDecision.previous_topic_summary), 54),
            meta: 'previous topic summary',
          } : null,
        ].filter((item): item is { title: string; meta: string } => Boolean(item)),
        ...taskTurns.slice(0, 3).map((turn) => ({
          title: shortText(scalar(turn.progress_summary ?? turn.user_input ?? turn.assistant_visible_output), 54),
          meta: shortText(`${scalar(turn.operation_id)} · ${scalar(turn.status)}`, 54),
        })),
      ],
      detailTitle: 'Task Detail',
      detailSubtitle: 'current task / candidate task / topic continuity 摘要',
    },
    {
      id: 'execution',
      kicker: 'Execution',
      title: executionStatus !== '-' ? executionStatus : 'idle',
      summary: shortText(
        `pending=${pendingCount} · active_step=${activeStep} · resume=${scalar(execution.resume_from_step_id)}`,
        92,
      ),
      facts: [
        ['active step', shortText(activeStep, 28)],
        ['resume from', shortText(scalar(execution.resume_from_step_id), 28)],
        ['pending', String(pendingCount)],
        ['interrupt', shortText(scalar(interruptedSegment.status ?? segmentMerge.strategy), 24)],
      ],
      list: [
        ...pendingInputs.slice(0, 3).map((item) => {
          const pending = asRecord(item);
          return {
            title: shortText(scalar(pending.message), 54),
            meta: shortText(`pending · ${scalar(pending.input_kind)} · ${scalar(pending.enqueue_reason)}`, 54),
          };
        }),
        ...[
          scalar(pauseCheckpoint.checkpoint_id) !== '-' ? {
            title: shortText(`checkpoint ${scalar(pauseCheckpoint.checkpoint_id)}`, 54),
            meta: shortText(`pause · ${scalar(pauseCheckpoint.reason)} · ${scalar(pauseCheckpoint.paused_at)}`, 60),
          } : null,
          scalar(segmentMerge.merge_id) !== '-' ? {
            title: shortText(`merge ${scalar(segmentMerge.merge_id)}`, 54),
            meta: shortText(`${scalar(segmentMerge.strategy)} · resumed=${scalar(segmentMerge.resumed_operation_id)}`, 60),
          } : null,
        ].filter((item): item is { title: string; meta: string } => Boolean(item)),
      ],
      detailTitle: 'Execution Detail',
      detailSubtitle: '运行状态、pending queue、pause / interrupt / merge 摘要',
    },
    {
      id: 'skills',
      kicker: 'Skills',
      title: loadedSkillCount > 0 ? `${loadedSkillCount} loaded skills` : 'Prompt stack / no loaded skills',
      summary: shortText(
        `role=${roleId} · layers=${activeLayerSummary} · behavior-rules=${arrayCount(rolePrompt.behavior_rules)} · modules=${promptModules.length}`,
        92,
      ),
      facts: [
        ['role', shortText(roleId, 20)],
        ['loaded', String(loadedSkillCount)],
        ['prompt layers', String(promptLayers.length)],
        ['modules', String(promptModules.length)],
        ['contract', shortText(scalar(firstArrayItem(rolePrompt.output_contract)), 24)],
      ],
      list: promptSourceItems,
      detailTitle: 'Skills Detail',
      detailSubtitle: 'loaded skills / prompt layers / behavior rules / output contract 摘要',
    },
    {
      id: 'plugins',
      kicker: 'Capabilities',
      title: `${modelTools.length} model / ${frameworkTools.length} framework`,
      summary: shortText(
        `selection=${selectionPolicies.length} · disabled=${disabledTools.length} · guards=${hardGuards.length} · surface=runtime tools`,
        92,
      ),
      facts: [
        ['model tools', String(modelTools.length)],
        ['framework', String(frameworkTools.length)],
        ['disabled', String(disabledTools.length)],
        ['guards', String(hardGuards.length)],
      ],
      list: capabilityItems,
      detailTitle: 'Capabilities & Tools',
      detailSubtitle: 'model tools / framework tools / selection policy / disabled / hard guards 摘要',
    },
  ];
}

function renderSidebarSection(
  section: SidebarSection,
  tree: StructuredTreeRenderer,
  expanded: boolean,
): string {
  return `
    <section class="rail-section">
      <div class="rail-section-kicker">${tree.escapeHtml(section.kicker)}</div>
      <button
        class="rail-digest-card"
        type="button"
        data-toggle-rail-section="${tree.escapeHtml(section.id)}"
        aria-expanded="${expanded ? 'true' : 'false'}"
      >
        <div class="rail-digest-header">
          <div class="rail-card-title">${tree.escapeHtml(section.title)}</div>
          <span class="rail-digest-open">${expanded ? 'Collapse' : 'Expand'}</span>
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
      ${expanded ? renderSidebarExpanded(section, tree) : ''}
    </section>
  `;
}

function renderSidebarExpanded(section: SidebarSection, tree: StructuredTreeRenderer): string {
  return `
    <section class="rail-inline-panel">
      <div class="rail-inline-panel-header">
        <div>
          <div class="rail-inline-kicker">Quick View</div>
          <div class="rail-inline-title">${tree.escapeHtml(section.detailTitle)}</div>
        </div>
        <button
          class="rail-inline-detail-btn"
          type="button"
          data-open-rail-detail="${tree.escapeHtml(section.id)}"
        >
          View detail
        </button>
      </div>
      <div class="rail-inline-summary">${tree.escapeHtml(section.detailSubtitle)}</div>
      <div class="rail-inline-facts">
        ${section.facts.map(([label, value]) => `
          <article class="rail-fact-pill">
            <span class="rail-fact-label">${tree.escapeHtml(label)}</span>
            <span class="rail-fact-value">${tree.escapeHtml(value)}</span>
          </article>
        `).join('')}
      </div>
      ${section.list?.length ? `
        <section class="rail-inline-list">
          ${section.list.slice(0, 3).map((item) => `
            <article class="rail-inline-item">
              <div class="rail-inline-title">${tree.escapeHtml(item.title)}</div>
              <div class="rail-inline-meta">${tree.escapeHtml(item.meta)}</div>
            </article>
          `).join('')}
        </section>
      ` : ''}
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

function parseLoadedSkillCount(summary: string): number {
  const match = summary.match(/^(\d+)\s+global skills loaded/);
  return match ? Number(match[1]) : 0;
}

function parseLoadedSkillIds(summary: string): string[] {
  const parts = summary.split(':');
  if (parts.length < 2) return [];
  return parts[1]
    .split(',')
    .map((part) => part.trim())
    .filter((part) => part && !part.startsWith('+'));
}

function buildTaskSummary(
  currentTaskId: string,
  candidateTaskId: string,
  topicThreadId: string,
  routingDisposition: string,
): string {
  return `current=${currentTaskId} · candidate=${candidateTaskId} · topic=${topicThreadId} · routing=${routingDisposition}`;
}

function turnsForTask(turns: TurnRecord[], taskId: string): TurnRecord[] {
  if (taskId === '-') return [];
  return turns.filter((turn) => scalar(asRecord(asRecord(turn).refs).task_id) === taskId);
}

function capabilityItemFromTool(tool: JsonRecord, kindLabel: string): { title: string; meta: string } {
  const toolName = scalar(tool.tool_name);
  const purpose = scalar(tool.purpose);
  const whenToUse = asScalarArray(tool.when_to_use);
  const sideEffects = asScalarArray(tool.side_effects);
  const metaParts = [
    kindLabel,
    purpose !== '-' ? shortText(purpose, 34) : null,
    whenToUse.length ? `use=${shortText(whenToUse[0], 28)}` : null,
    sideEffects.length ? `effects=${shortText(sideEffects[0], 24)}` : null,
  ].filter((part): part is string => Boolean(part));
  return {
    title: shortText(toolName, 54),
    meta: shortText(metaParts.join(' · '), 80),
  };
}
