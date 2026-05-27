import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), '../..');
const runId = process.argv[2] || 'test-goal-live-20260524-130105';
const runtimeHome =
  process.argv[3] || path.join(os.homedir(), '.fin/harness/runs', runId, 'runtime-home');
const userToml =
  process.argv[4] || path.join(os.homedir(), '.fin/harness/runs', runId, 'user.test.toml');
const bindPort = Number(process.argv[5] || 4147);
const bindHost = '127.0.0.1';

if (!fs.existsSync(runtimeHome)) {
  throw new Error(`runtime_home missing: ${runtimeHome}`);
}
if (!fs.existsSync(userToml)) {
  throw new Error(`user_toml missing: ${userToml}`);
}

let serverReady = false;
let stdoutBuf = '';
let stderrBuf = '';
let debugProc = null;
let actualPort = bindPort;
let baseUrl = `http://${bindHost}:${actualPort}`;

function startDebugProcess(port) {
  actualPort = port;
  baseUrl = `http://${bindHost}:${actualPort}`;
  stdoutBuf = '';
  stderrBuf = '';
  debugProc = spawn(
    'cargo',
    ['run', '-p', 'fin-cli', '--manifest-path', 'rust/Cargo.toml', '--', 'web-debug', userToml, bindHost, String(actualPort)],
    {
      cwd: repoRoot,
      env: {
        ...process.env,
        FIN_RUNTIME_HOME_OVERRIDE: runtimeHome,
      },
      stdio: ['ignore', 'pipe', 'pipe'],
    },
  );
  debugProc.stdout.on('data', (chunk) => {
    stdoutBuf += chunk.toString();
  });
  debugProc.stderr.on('data', (chunk) => {
    stderrBuf += chunk.toString();
  });
}

async function waitForServer(timeoutMs = 20000) {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    if (debugProc.exitCode !== null) {
      throw new Error(`web-debug exited early: code=${debugProc.exitCode}\n${stderrBuf}`);
    }
    try {
      const response = await fetch(`${baseUrl}/api/binding.json`);
      if (response.ok) {
        serverReady = true;
        return;
      }
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`web-debug did not start in time\nstdout:\n${stdoutBuf}\nstderr:\n${stderrBuf}`);
}

async function startDebugWithRetry() {
  const candidates = [bindPort, bindPort + 1, bindPort + 11, bindPort + 111];
  let lastError = null;
  for (const candidate of candidates) {
    startDebugProcess(candidate);
    try {
      await waitForServer();
      return;
    } catch (error) {
      lastError = error;
      const message = String(error?.message || error);
      if (!message.includes('Address already in use')) {
        throw error;
      }
    } finally {
      if (!serverReady && debugProc && debugProc.exitCode === null) {
        debugProc.kill('SIGTERM');
        await new Promise((resolve) => setTimeout(resolve, 200));
      }
    }
  }
  throw lastError ?? new Error('failed to start web-debug');
}

function makeEl() {
  return {
    innerHTML: '',
    textContent: '',
    scrollTop: 0,
    scrollHeight: 0,
    clientHeight: 0,
    attrs: {},
    setAttribute(key, value) {
      this.attrs[key] = value;
    },
    querySelector() {
      return null;
    },
  };
}

try {
  await startDebugWithRetry();

  const [binding, messages, activityCards, lastRun, currentContext] = await Promise.all([
    fetch(`${baseUrl}/api/binding.json`).then((r) => r.json()),
    fetch(`${baseUrl}/api/session_messages.json`).then((r) => r.json()),
    fetch(`${baseUrl}/api/activity_cards.json`).then((r) => r.json()),
    fetch(`${baseUrl}/api/last_run.json`).then((r) => r.json()),
    fetch(`${baseUrl}/api/current_context.json`).then(async (r) => (r.ok ? r.json() : null)).catch(() => null),
  ]);

  if (!Array.isArray(messages) || messages.length < 2) {
    throw new Error(`expected real session messages, got: ${JSON.stringify(messages)}`);
  }
  if (!activityCards || !Array.isArray(activityCards.source_cards)) {
    throw new Error(`expected activity cards snapshot, got: ${JSON.stringify(activityCards)}`);
  }

  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fin-live-chat-render-'));
  fs.writeFileSync(path.join(tempDir, 'package.json'), JSON.stringify({ type: 'module' }));
  const distDir = path.join(repoRoot, 'rust/crates/debug-server/webui/dist');
  for (const file of ['chat.js', 'chat_cards.js', 'activity_cards_ui.js', 'time.js', 'tree.js']) {
    fs.copyFileSync(path.join(distDir, file), path.join(tempDir, file));
  }
  fs.writeFileSync(
    path.join(tempDir, 'run.mjs'),
    `
import { ChatPane } from './chat.js';
import { StructuredTreeRenderer } from './tree.js';

globalThis.HTMLElement = class {
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
};

const binding = ${JSON.stringify(binding)};
const messages = ${JSON.stringify(messages)};
const activityCards = ${JSON.stringify(activityCards)};
const lastRun = ${JSON.stringify(lastRun)};
const currentContext = ${JSON.stringify(currentContext)};

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

chat.render(
  binding,
  messages,
  [],
  null,
  'rich',
  null,
  lastRun,
  currentContext,
  activityCards,
  null,
);

const html = chat.messagesEl.innerHTML;
if (!html.includes('project result:')) throw new Error('missing real assistant session message');
if (!html.includes('System Progress')) throw new Error('missing system progress thread');
if (!html.includes('project agent completed real LLM execution against configured cwd')) {
  throw new Error('missing delegated completion summary from live runtime');
}
if (!html.includes('project-fin')) throw new Error('missing project agent card/title in live render');
console.log('live-runtime-chat-smoke:ok');
`,
  );

  const { status, stdout, stderr } = await new Promise((resolve) => {
    const child = spawn(process.execPath, [path.join(tempDir, 'run.mjs')], {
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let out = '';
    let err = '';
    child.stdout.on('data', (chunk) => {
      out += chunk.toString();
    });
    child.stderr.on('data', (chunk) => {
      err += chunk.toString();
    });
    child.on('exit', (code) => resolve({ status: code ?? 1, stdout: out, stderr: err }));
  });

  if (status !== 0) {
    throw new Error(`live render child failed\nstdout:\n${stdout}\nstderr:\n${stderr}`);
  }
  process.stdout.write(stdout);
} finally {
  if (serverReady && debugProc.pid) {
    debugProc.kill('SIGTERM');
  }
}
