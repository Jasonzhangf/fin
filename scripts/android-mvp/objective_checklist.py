#!/usr/bin/env python3
import json, pathlib
ROOT=pathlib.Path('/Volumes/extension/code/fin')
logs=ROOT/'reports/android-mvp-logs'
report=ROOT/'reports/android-mvp-objective-checklist.md'
all_status=json.loads((logs/'all-gates-status.json').read_text()) if (logs/'all-gates-status.json').exists() else {}
live_status=json.loads((logs/'tailscale-live-e2e-status.json').read_text()) if (logs/'tailscale-live-e2e-status.json').exists() else {}
preflight=json.loads((logs/'preflight-network-diagnose.json').read_text()) if (logs/'preflight-network-diagnose.json').exists() else {}

checks=[
("F1 live subscribed/healthy", bool(live_status.get('ok'))),
("Gate commands all green", bool(all_status.get('all_ok'))),
("APK exists", (ROOT/'android-client/update-dist/fin-latest-debug.apk').exists()),
("latest.json exists", (ROOT/'android-client/update-dist/latest.json').exists()),
("validation index exists", (ROOT/'reports/android-mvp-validation.md').exists()),
("receipt bundle exists", any((ROOT/'reports').glob('android-mvp-receipt-bundle-*.tgz'))),
]
lines=['# Android MVP Objective Checklist (Auto)','']
for name,ok in checks:
    lines.append(f"- {'✅' if ok else '❌'} {name}")
lines += [
'',
f"- preflight_blocker: `{preflight.get('blocker','unknown')}`",
f"- gate_overall: `{all_status.get('all_ok','unknown')}`",
f"- live_ok: `{live_status.get('ok','unknown')}`",
'',
'## Conclusion',
('ACHIEVED' if all(ok for _,ok in checks) else 'NOT ACHIEVED')
]
report.write_text('\n'.join(lines)+'\n')
print(report)
