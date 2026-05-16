#!/usr/bin/env python3
import json, pathlib, datetime

ROOT = pathlib.Path('/Volumes/extension/code/fin')
LOG = ROOT/'reports/android-mvp-logs'
LOG.mkdir(parents=True, exist_ok=True)

STATUS={"ts":datetime.datetime.now(datetime.UTC).isoformat().replace('+00:00','Z'),"ok":True,"checks":[]}

p = ROOT/'rust/crates/debug-server/src/mobile_ws.rs'
text = p.read_text()

required = [
    '"type":"turn.rendered"',
    '"user_input"',
    '"assistant_response"',
    '"control_feedback_summary"',
    '"tool_execution_summary"',
    '"tool_execution_records"',
    '"error_records"',
    '"closure_stop_source"',
]

for k in required:
    ok = k in text
    STATUS['checks'].append({'name':f'contract_has_{k}','ok':ok})
    STATUS['ok'] = STATUS['ok'] and ok

out = LOG/'turn-channel-contract.log'
out.write_text(json.dumps(STATUS, ensure_ascii=False, indent=2))
print(json.dumps(STATUS, ensure_ascii=False, indent=2))
raise SystemExit(0 if STATUS['ok'] else 1)
