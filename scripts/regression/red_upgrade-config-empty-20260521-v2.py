#!/usr/bin/env python3
import json, os, sys
ROOT = "/Volumes/extension/code/fin"
HTML = f"{ROOT}/android-client/app/src/main/assets/mobile-shell.html"
BUG_ID = "upgrade-config-empty-20260521-v2"
OUT = f"{ROOT}/reports/regression/bug-repro/{BUG_ID}"
os.makedirs(OUT, exist_ok=True)
src = open(HTML, "r", encoding="utf-8").read()
errors = []; notes = []
# RED gate: init MUST restore config from provider cache
checks = [
    ("(function initFromCache(){" in src,
     "init_missing_cache_restore: no IIFE initFromCache() found"),
    ("loadProviderCache();" in src,
     "init_missing_cache_restore: no loadProviderCache() call in init"),
    ("renderActiveConfig(cached);" in src,
     "init_missing_cache_restore: no renderActiveConfig(cached) call"),
    ("activeProviderBox" in src and "activeModelBox" in src and "activeEffortBox" in src,
     "missing_dom_readonly_boxes"),
    ("syncUpdateManifestInputFromConfig" in src,
     "missing_syncUpdateManifestInputFromConfig"),
    ("diagBox" in src and "diagState" in src,
     "missing_diag_elements"),
    ("S.connTarget" in src,
     "missing_connTarget_for_consistent_diag"),
]
for ok, msg in checks:
    if not ok: errors.append(msg)
    else: notes.append(msg.split(":")[0] + "_present")
result = {
    "bug_id": BUG_ID,
    "status": "PASS" if not errors else "FAIL",
    "errors": errors,
    "notes": notes,
}
open(f"{OUT}/red.json","w",encoding="utf-8").write(json.dumps(result,ensure_ascii=False,indent=2))
open(f"{OUT}/red.log","w",encoding="utf-8").write(json.dumps(result,ensure_ascii=False,indent=2)+"\n")
print(json.dumps(result,ensure_ascii=False))
sys.exit(0 if not errors else 1)
