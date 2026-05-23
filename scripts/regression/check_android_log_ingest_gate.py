#!/usr/bin/env python3
import json
from pathlib import Path

ROOT=Path('/Volumes/extension/code/fin')
R1=ROOT/'rust/crates/debug-server/src/routes.rs'
R2=ROOT/'rust/crates/debug-server/src/lib.rs'
K=ROOT/'android-client/app/src/main/java/com/fin/client/bridge/MobileBridge.kt'

errs=[]
rs=R1.read_text(encoding='utf-8')
ls=R2.read_text(encoding='utf-8')
ks=K.read_text(encoding='utf-8')

for needle in ['API_LOG_INGEST_PATH','API_LOG_LATEST_PATH','("POST", API_LOG_INGEST_PATH)','("GET", API_LOG_LATEST_PATH)','append_mobile_log_event','read_mobile_log_events']:
    if needle not in (rs+ls):
        errs.append(f'missing_daemon_log_route_or_handler:{needle}')

for needle in ['forwardConnectionEventToDaemon(event)','/api/log/ingest','httpClient.newCall(req).execute()']:
    if needle not in ks:
        errs.append(f'missing_android_forward:{needle}')

out={"status":"PASS" if not errs else "FAIL","errors":errs}
outdir=ROOT/'reports/regression/android-log-ingest'
outdir.mkdir(parents=True,exist_ok=True)
(outdir/'gate.json').write_text(json.dumps(out,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps(out,ensure_ascii=False))
raise SystemExit(0 if not errs else 1)
