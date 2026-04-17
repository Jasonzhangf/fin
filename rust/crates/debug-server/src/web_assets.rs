pub const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>fin Debug MVP</title>
    <link rel="stylesheet" href="/styles.css" />
  </head>
  <body>
    <header class="page-header">
      <div>
        <h1>fin Debug MVP</h1>
        <p class="muted">Rust runtime is truth. Web only observes current projection and event stream.</p>
      </div>
      <div class="meta">
        <span id="status-pill" class="pill">loading</span>
        <span id="last-updated" class="muted">not loaded</span>
      </div>
    </header>

    <main class="layout">
      <section class="panel">
        <div class="panel-title-row">
          <h2>Current Projection</h2>
          <button id="refresh-btn">Refresh</button>
        </div>
        <div id="projection-grid" class="kv-grid"></div>
      </section>

      <section class="panel">
        <h2>Last Run</h2>
        <pre id="last-run" class="code-block">loading...</pre>
      </section>

      <section class="panel">
        <div class="panel-title-row">
          <h2>Warnings / Errors</h2>
          <span id="alert-count" class="muted">0</span>
        </div>
        <div id="alerts" class="stack-list empty-state">No warnings yet.</div>
      </section>

      <section class="panel panel-wide">
        <div class="panel-title-row">
          <h2>Live Event Stream</h2>
          <input id="event-filter" type="text" placeholder="filter by event / trace / task / session" />
        </div>
        <div id="event-list" class="event-list"></div>
      </section>
    </main>

    <script src="/app.js"></script>
  </body>
</html>
"#;

pub const STYLES_CSS: &str = r#":root {
  color-scheme: dark;
  --bg: #0b1020;
  --panel: #141b2d;
  --border: #26324c;
  --text: #d7def0;
  --muted: #95a3bf;
  --accent: #6ea8fe;
  --warn: #ffcc66;
  --error: #ff7b72;
  --ok: #3fb950;
}

* { box-sizing: border-box; }
body {
  margin: 0;
  font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  background: var(--bg);
  color: var(--text);
}

.page-header,
.layout {
  width: min(1200px, calc(100vw - 32px));
  margin: 0 auto;
}

.page-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding: 24px 0 16px;
}

.page-header h1 { margin: 0 0 8px; }
.muted { color: var(--muted); }
.pill {
  display: inline-flex;
  align-items: center;
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 6px 10px;
  font-size: 12px;
}

.layout {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
  padding-bottom: 24px;
}

.panel {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 16px;
  min-height: 120px;
}

.panel-wide { grid-column: 1 / -1; }
.panel h2 { margin: 0 0 16px; font-size: 18px; }

.panel-title-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}

button,
input {
  background: #0f1729;
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: 8px;
}

button {
  cursor: pointer;
  padding: 8px 12px;
}

input {
  padding: 9px 12px;
  min-width: 320px;
}

.kv-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

.kv-card {
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 12px;
  background: rgba(255,255,255,0.01);
}

.kv-card .key {
  display: block;
  color: var(--muted);
  font-size: 12px;
  margin-bottom: 6px;
}

.kv-card .value {
  display: block;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  word-break: break-word;
}

.code-block {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  padding: 12px;
  border-radius: 10px;
  background: #0f1729;
  border: 1px solid var(--border);
  font-size: 12px;
}

.stack-list,
.event-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.empty-state {
  color: var(--muted);
}

.alert-card,
.event-card {
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 12px;
  background: #0f1729;
}

.alert-card.warn { border-color: rgba(255, 204, 102, 0.45); }
.alert-card.error { border-color: rgba(255, 123, 114, 0.45); }

.event-card-head {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: 12px;
  margin-bottom: 8px;
}

.event-name {
  font-weight: 600;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}

.event-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  color: var(--muted);
  font-size: 12px;
}

.event-payload {
  margin: 0;
  font-size: 12px;
  color: #c4d0ea;
  white-space: pre-wrap;
  word-break: break-word;
}

@media (max-width: 960px) {
  .layout { grid-template-columns: 1fr; }
  .panel-wide { grid-column: auto; }
  .panel-title-row { flex-direction: column; align-items: stretch; }
  input { min-width: 0; width: 100%; }
  .kv-grid { grid-template-columns: 1fr; }
}
"#;

pub const APP_JS: &str = r#"const projectionGrid = document.getElementById("projection-grid");
const eventList = document.getElementById("event-list");
const alerts = document.getElementById("alerts");
const alertCount = document.getElementById("alert-count");
const lastRunEl = document.getElementById("last-run");
const lastUpdatedEl = document.getElementById("last-updated");
const statusPill = document.getElementById("status-pill");
const eventFilter = document.getElementById("event-filter");
const refreshBtn = document.getElementById("refresh-btn");

let latestState = { projection: null, events: [], lastRun: null };

function safeJson(value) {
  return JSON.stringify(value, null, 2);
}

function setStatus(text, ok = true) {
  statusPill.textContent = text;
  statusPill.style.borderColor = ok ? "rgba(63,185,80,0.45)" : "rgba(255,123,114,0.45)";
  statusPill.style.color = ok ? "var(--ok)" : "var(--error)";
}

function renderProjection(projection) {
  const entries = Object.entries(projection || {});
  if (!entries.length) {
    projectionGrid.innerHTML = '<div class="empty-state">No projection loaded.</div>';
    return;
  }
  projectionGrid.innerHTML = entries.map(([key, value]) => `
    <div class="kv-card">
      <span class="key">${key}</span>
      <span class="value">${value === null ? "-" : String(value)}</span>
    </div>
  `).join("");
}

function eventMatchesFilter(event, filter) {
  if (!filter) return true;
  const haystack = [
    event.event_type,
    event.trace_id,
    event.refs?.task_id,
    event.refs?.session_id,
    event.refs?.dispatch_id,
    event.refs?.worker_id
  ].filter(Boolean).join(" ").toLowerCase();
  return haystack.includes(filter.toLowerCase());
}

function renderEvents(events) {
  const filter = eventFilter.value.trim();
  const filtered = (events || []).filter((event) => eventMatchesFilter(event, filter));
  if (!filtered.length) {
    eventList.innerHTML = '<div class="empty-state">No events match the current filter.</div>';
    return;
  }
  eventList.innerHTML = filtered.slice().reverse().map((event) => `
    <article class="event-card">
      <div class="event-card-head">
        <span class="event-name">${event.sequence}. ${event.event_type}</span>
        <span class="muted">${event.timestamp}</span>
      </div>
      <div class="event-meta">
        <span>trace=${event.trace_id}</span>
        <span>task=${event.refs?.task_id || "-"}</span>
        <span>session=${event.refs?.session_id || "-"}</span>
        <span>source=${event.source}</span>
      </div>
      <pre class="event-payload">${safeJson(event.payload)}</pre>
    </article>
  `).join("");
}

function renderAlerts(projection, events) {
  const warningRows = [];
  for (const warning of projection?.warnings || []) {
    warningRows.push({ level: "warn", title: warning, details: "projection warning" });
  }
  for (const event of events || []) {
    if (event.event_type.includes("failed") || event.event_type.includes("timeout")) {
      warningRows.push({
        level: "error",
        title: event.event_type,
        details: `${event.timestamp} · trace=${event.trace_id}`
      });
    }
  }

  alertCount.textContent = String(warningRows.length);
  if (!warningRows.length) {
    alerts.className = "stack-list empty-state";
    alerts.textContent = "No warnings yet.";
    return;
  }

  alerts.className = "stack-list";
  alerts.innerHTML = warningRows.map((row) => `
    <article class="alert-card ${row.level}">
      <strong>${row.title}</strong>
      <div class="muted">${row.details}</div>
    </article>
  `).join("");
}

async function fetchJson(url) {
  const response = await fetch(url, { cache: "no-store" });
  if (!response.ok) {
    throw new Error(`${url} -> ${response.status}`);
  }
  return response.json();
}

async function fetchJsonLines(url) {
  const response = await fetch(url, { cache: "no-store" });
  if (!response.ok) {
    throw new Error(`${url} -> ${response.status}`);
  }
  const text = await response.text();
  return text.split("\n").map((line) => line.trim()).filter(Boolean).map((line) => JSON.parse(line));
}

async function refresh() {
  try {
    const [snapshot, lastRun] = await Promise.all([
      fetchJson("/api/current_snapshot.json"),
      fetchJson("/api/last_run.json").catch(() => null)
    ]);
    const events = Array.isArray(snapshot.events) ? snapshot.events : await fetchJsonLines("/api/latest_events.jsonl");
    latestState = { projection: snapshot.projection || {}, events, lastRun };
    renderProjection(latestState.projection);
    renderEvents(latestState.events);
    renderAlerts(latestState.projection, latestState.events);
    lastRunEl.textContent = lastRun ? safeJson(lastRun) : "No last_run.json yet.";
    lastUpdatedEl.textContent = `updated ${new Date().toLocaleTimeString()}`;
    setStatus("connected", true);
  } catch (error) {
    setStatus("waiting for runtime artifacts", false);
    lastUpdatedEl.textContent = String(error);
  }
}

eventFilter.addEventListener("input", () => renderEvents(latestState.events));
refreshBtn.addEventListener("click", refresh);

refresh();
setInterval(refresh, 2000);
"#;
