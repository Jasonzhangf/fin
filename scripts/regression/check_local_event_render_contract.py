#!/usr/bin/env python3
import json, pathlib, datetime
ROOT = pathlib.Path(__file__).resolve().parents[2]
html = ROOT / 'android-client/app/src/main/assets/mobile-shell.html'
text = html.read_text(encoding='utf-8')
checks = {
  'tool_timeline_render': ("function renderToolStatus(records)" in text) or ("function renderToolTimelineFromItems(items,live)" in text and "function visibleTimelineItems(items)" in text),
  'error_timeline_render': ("function renderErrorList(errors)" in text) or ("function normalizeErrorRecord(e)" in text and "error_summary" in text),
  'legacy_event_consumer_removed': "turn.tool_event" not in text and "turn.error_event" not in text,
  'raw_fallback_storage': "(raw)" in text or "raw.event.unhandled" in text,
  'conn_state_healthy': "setConn('healthy')" in text or "S.conn==='healthy'" in text or "state=healthy" in text,
  'conn_state_disconnected': "未连接" in text,
  'readonly_config_boxes': "activeProviderBox" in text and "activeModelBox" in text and "activeEffortBox" in text,
  'update_manifest_single_source': "CONFIG.get('daemon_host'" in text and "CONFIG.get('daemon_port'" in text and "syncUpdateManifestInputFromConfig" in text,
}
ok = all(checks.values())
out = {'ts': datetime.datetime.utcnow().isoformat() + 'Z','ok': ok,'checks': checks,'file': str(html)}
out_path = ROOT / 'reports/regression/event-render-contract-status.json'
out_path.parent.mkdir(parents=True, exist_ok=True)
out_path.write_text(json.dumps(out, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps(out, ensure_ascii=False, indent=2))
raise SystemExit(0 if ok else 1)
