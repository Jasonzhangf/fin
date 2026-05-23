#!/usr/bin/env python3
"""RED: 升级后配置清空 bug - 行为验证版"""
import json, os, sys
ROOT = "/Volumes/extension/code/fin"
HTML = f"{ROOT}/android-client/app/src/main/assets/mobile-shell.html"
BUG_ID = "upgrade-config-empty-20260521-v3"
OUT = f"{ROOT}/reports/regression/bug-repro/{BUG_ID}"
os.makedirs(OUT, exist_ok=True)
src = open(HTML, "r", encoding="utf-8").read()
errors = []; notes = []

# ── RED Gate 1: localStorage upgrade persistence ──────────────────────────
# Bug: 升级时 localStorage fin_state 被清空，无迁移逻辑
# Fix: provider cache 必须存在 localStorage 之外的持久化路径（Android SharedPreferences via bridge）
if "readProviderConfigCacheJson" not in src:
    errors.append("no_android_bridge_readProviderConfigCacheJson: bridge method not in HTML")
if "saveProviderConfigCache" not in src:
    errors.append("no_android_bridge_saveProviderConfigCache: bridge method not in HTML")

# ── RED Gate 2: CONFIG.update_manifest_url needs default ──────────────────
# Bug: update_manifest_url 空字符串无默认值，升级后 UI 无 upgrade URL
if "update_manifest_url" not in src or ("CONFIG.get('update_manifest_url','')" in src and "set('update_manifest_url'," not in src):
    # check if there's a default being set somewhere
    lines_with_url = [l.strip() for l in src.split('\n') if 'update_manifest_url' in l]
    has_default_set = any("set('update_manifest_url'" in l or 'set("update_manifest_url"' in l for l in lines_with_url)
    if not has_default_set:
        errors.append("no_update_manifest_url_default: update_manifest_url has no default value set in CONFIG")

# ── RED Gate 3: diag click must be truly non-blocking ─────────────────────
# Bug: click handler calls bridge() twice synchronously before setTimeout
lines = src.split('\n')
in_diag_handler = False
diag_block_lines = []
for i, l in enumerate(lines):
    if "diagBox" in l and "addEventListener" in l:
        in_diag_handler = True
        diag_block_lines = [l]
    elif in_diag_handler:
        diag_block_lines.append(l)
        if l.strip().startswith("});") or l.strip().startswith("}"):
            in_diag_handler = False

diag_code = '\n'.join(diag_block_lines)
sync_bridge_calls_before_setTimeout = 0
for l in diag_block_lines:
    if 'bridge()' in l and 'setTimeout' not in l:
        sync_bridge_calls_before_setTimeout += 1
if sync_bridge_calls_before_setTimeout > 1:
    errors.append(f"diag_blocking_calls: {sync_bridge_calls_before_setTimeout} sync bridge() calls before setTimeout in diag handler")

# ── RED Gate 4: renderActiveConfig must handle empty/null cache gracefully ─
# If cache is null, readonly boxes should show (not connected) state
if "if(cached)" in src and "renderActiveConfig(cached)" in src:
    notes.append("cache_null_guard_present")
else:
    errors.append("missing_cache_null_guard: renderActiveConfig needs null check before call")

result = {
    "bug_id": BUG_ID,
    "status": "FAIL" if errors else "PASS",
    "errors": errors,
    "notes": notes,
}
open(f"{OUT}/red.json","w",encoding="utf-8").write(json.dumps(result,ensure_ascii=False,indent=2))
open(f"{OUT}/red.log","w",encoding="utf-8").write(json.dumps(result,ensure_ascii=False,indent=2)+"\n")
print(json.dumps(result,ensure_ascii=False))
sys.exit(0 if not errors else 1)
