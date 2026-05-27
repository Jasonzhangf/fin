import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), '../..');
const distDir = path.join(repoRoot, 'rust/crates/debug-server/webui/dist');
const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fin-chat-render-smoke-'));

fs.writeFileSync(path.join(tempDir, 'package.json'), JSON.stringify({ type: 'module' }));

for (const file of ['chat.js', 'chat_cards.js', 'activity_cards_ui.js', 'time.js', 'tree.js']) {
  fs.copyFileSync(path.join(distDir, file), path.join(tempDir, file));
}

fs.writeFileSync(
  path.join(tempDir, 'stub.mjs'),
  `
class HTMLElementStub {
  constructor() {
    this.innerHTML = '';
    this.textContent = '';
    this.scrollTop = 0;
    this.scrollHeight = 0;
    this.clientHeight = 0;
    this.attrs = {};
  }
  setAttribute(key, value) {
    this.attrs[key] = value;
  }
}
globalThis.HTMLElement = HTMLElementStub;
`,
);

fs.writeFileSync(
  path.join(tempDir, 'run.mjs'),
  `
import './stub.mjs';
import { ChatPane } from './chat.js';
import { StructuredTreeRenderer } from './tree.js';

const makeEl = () => new globalThis.HTMLElement();
const chat = new ChatPane(
  makeEl(),
  makeEl(),
  makeEl(),
  makeEl(),
  makeEl(),
  makeEl(),
  makeEl(),
  makeEl(),
  makeEl(),
  makeEl(),
  makeEl(),
  makeEl(),
  new StructuredTreeRenderer(),
);

const messages = [
  {
    message_id: 'user-op-1',
    role: 'user',
    content: '请研究 fin',
    created_at: '2026-05-24T10:00:00Z',
    operation_id: 'op-1',
  },
  {
    message_id: 'assistant-closure-op-1',
    role: 'assistant',
    content: '我先调度 project agent。',
    created_at: '2026-05-24T10:00:05Z',
    operation_id: 'op-1',
  },
];

const focusTurns = [{
  operationId: 'op-1',
  userMessage: messages[0],
  assistantMessage: messages[1],
  toolRecords: [],
  events: [],
}];

const activityCards = {
  generated_at: '2026-05-24T10:00:20Z',
  user_card: {
    header: 'system frontstage · busy',
    state: 'running',
    focus_source_id: 'local.project-fin',
    recent_items: ['已派发 fin 研究任务'],
    updated_at: '2026-05-24T10:00:20Z',
  },
  source_cards: [{
    source_id: 'local.project-fin',
    source_kind: 'project_agent',
    title: 'Atlas',
    state: 'running',
    summary: '正在研究 ~/code/fin',
    current_activity: '执行 codex 对比与优化建议整理',
    updated_at: '2026-05-24T10:00:20Z',
    recent_actions: [
      {
        operation_id: 'project-op-1',
        tool_name: 'peer.list',
        verb: 'Explored',
        summary: '检查 peers',
        status: 'completed',
        started_at: '2026-05-24T10:00:10Z',
      },
      {
        operation_id: 'project-op-1',
        tool_name: 'provider.call',
        verb: 'Ran',
        summary: '调用模型做分析',
        status: 'running',
        started_at: '2026-05-24T10:00:18Z',
      },
    ],
  }],
  tool_semantics: [],
};

chat.render(
  {
    project_id: 'fin',
    project_label: 'fin',
    runtime_home: '/tmp',
    session_id: 'system-agent',
    task_id: 'task-1',
  },
  messages,
  focusTurns,
  'op-1',
  'rich',
  null,
  { provider: 'minimax', model: 'MiniMax-M2.7' },
  { context: { project: { project_root: '/Volumes/extension/code/fin' } } },
  activityCards,
  null,
);

const html = chat.messagesEl.innerHTML;
if (!html.includes('我先调度 project agent')) throw new Error('missing assistant bubble');
if (!html.includes('执行 codex 对比与优化建议整理')) throw new Error('missing delegated progress in thread');
if (!html.includes('已派发 fin 研究任务')) throw new Error('missing recent progress item');
if (!html.includes('Atlas')) throw new Error('missing project agent title');
console.log('chat-render-smoke:ok');
`,
);

const { spawnSync } = await import('node:child_process');
const result = spawnSync(process.execPath, [path.join(tempDir, 'run.mjs')], {
  stdio: 'inherit',
});
process.exit(result.status ?? 1);
