#!/usr/bin/env python3
import json, pathlib
ROOT=pathlib.Path('/Volumes/extension/code/fin')
status_path=ROOT/'reports/android-mvp-logs/all-gates-status.json'
live_path=ROOT/'reports/android-mvp-logs/tailscale-live-e2e-status.json'
out=ROOT/'reports/android-mvp-validation.md'
if not status_path.exists():
    raise SystemExit('missing all-gates-status.json, run run_all_gates.py first')
status=json.loads(status_path.read_text())
live=json.loads(live_path.read_text()) if live_path.exists() else {'ok':False,'required':[]}

rows=[]
for r in status['results']:
    rows.append(f"| {r['name']} | {'✅' if r['ok'] else '❌'} | `{r['log']}` |")

text=f'''# Android MVP Validation Matrix (Auto, Current Truth)\n\n- overall gate: {'PASS' if status['all_ok'] else 'FAIL'}\n- gate timestamp: {status['ts']}\n\n## Gate Results\n| gate | status | log |\n|---|---|---|\n'''+'\n'.join(rows)+'''\n\n## Live Tailscale E2E\n- status: '''+('PASS' if live.get('ok') else 'FAIL')+'''\n- required markers: '''+', '.join(live.get('required',[]))+'''\n- evidence:\n  - reports/android-mvp-logs/tailscale-live-e2e-status.json\n  - reports/android-mvp-logs/tailscale-connection-events.log\n  - reports/android-mvp-screenshots/tailscale-live-e2e.png\n\n## Deliverables\n- APK: `android-client/update-dist/fin-latest-debug.apk`\n- Manifest: `android-client/update-dist/latest.json`\n- Gate summary: `reports/android-mvp-gate-summary.md`\n- Completion audit: `reports/android-mvp-completion-audit.md`\n\n## Verdict\n'''+('ACHIEVED' if status['all_ok'] and live.get('ok') else 'NOT ACHIEVED')+'''\n'''
out.write_text(text)
print(out)
