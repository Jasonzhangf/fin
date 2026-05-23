#!/usr/bin/env node
import fs from 'node:fs';
import os from 'node:os';
import pathModule from 'node:path';
const path = process.argv[2] || pathModule.join(os.homedir(), '.fin', 'logs', 'turn-channel-e2e.log');
function fail(msg){ throw new Error(msg); }
const data = JSON.parse(fs.readFileSync(path, 'utf8'));
const events = Array.isArray(data.events) ? data.events : [];
const itemEvents = events.filter(e => String(e.type||'').startsWith('turn.item.'));
const started = itemEvents.filter(e => e.type === 'turn.item.started');
const terminal = itemEvents.filter(e => e.type === 'turn.item.completed' || e.type === 'turn.item.failed');
if (started.length === 0) fail('no turn.item.started events');
const byId = new Map();
for (const e of itemEvents) {
  if (!e.item_id) fail(`missing item_id in ${JSON.stringify(e)}`);
  if (!byId.has(e.item_id)) byId.set(e.item_id, []);
  byId.get(e.item_id).push(e);
  for (const f of ['label','title','purpose','status']) {
    const v = String(e[f] ?? '').trim();
    if (!v) fail(`missing ${f} for item ${e.item_id}`);
    if (v === 'tool' || v === 'unknown') fail(`non-informative ${f}=${v} for item ${e.item_id}`);
  }
}
for (const e of started) {
  const terms = (byId.get(e.item_id) || []).filter(x => x.type === 'turn.item.completed' || x.type === 'turn.item.failed');
  if (terms.length === 0) fail(`item ${e.item_id} has no terminal event`);
}
const failed = terminal.filter(e => e.type === 'turn.item.failed' || e.status === 'failed');
if (failed.length > 0 && failed.some(e => !String(e.error_summary || '').trim())) fail('failed item missing error_summary');
if (!events.some(e => e.type === 'turn.started')) fail('missing turn.started');
if (!events.some(e => e.type === 'turn.completed')) fail('missing turn.completed');
if (!events.some(e => e.type === 'runtime.health') && !events.some(e => e.type === 'turn.progress')) fail('missing runtime observability');
const rendered = events.filter(e => e.type === 'turn.rendered');
if (rendered.length === 0) fail('missing turn.rendered');
if (rendered.some(e => !String(e.assistant_response || '').trim() && e.closure_stop_source !== 'error')) fail('rendered turn missing assistant_response');
console.log(JSON.stringify({ok:true,path,item_started:started.length,item_terminal:terminal.length,failed:failed.length,rendered:rendered.length}, null, 2));
