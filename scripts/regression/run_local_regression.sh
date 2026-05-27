#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"
RUN_ID="test-local-reg-$(date +%Y%m%d-%H%M%S)"
RUN_ROOT="$HOME/.fin/harness/runs/$RUN_ID"
RUNTIME_HOME="$RUN_ROOT/runtime-home"
USER_TOML="$RUN_ROOT/user.test.toml"
mkdir -p "$RUN_ROOT" reports/regression
scripts/generate-test-user-toml.py --output "$USER_TOML" --scope-label local-regression >/dev/null
export FIN_RUNTIME_HOME_OVERRIDE="$RUNTIME_HOME"
export FIN_SESSION_NAMESPACE="$RUN_ID"
STATUS_JSON="reports/regression/local-regression-status.json"
SUMMARY_MD="reports/regression/local-regression-summary.md"

cat > "$STATUS_JSON" <<JSON
{"run_id":"$RUN_ID","gates":[]}
JSON

run_gate() {
  local name="$1"; shift
  local cmd="$*"
  local log="reports/regression/${name}.log"
  local blocking="true"
  set +e
  bash -lc "$cmd" >"$log" 2>&1
  local code=$?
  set -e
  python3 - "$STATUS_JSON" "$name" "$cmd" "$code" "$log" "$blocking" <<'PY'
import json, sys
p,name,cmd,code,log,blocking = sys.argv[1:7]
obj=json.load(open(p))
obj['gates'].append({
  'name':name,
  'cmd':cmd,
  'ok':int(code)==0,
  'code':int(code),
  'log':log,
  'blocking': blocking == 'true'
})
json.dump(obj, open(p,'w'), ensure_ascii=False, indent=2)
PY
  return $code
}

ALL_OK=1
run_gate g0_config_check "cargo run -p fin-cli --manifest-path rust/Cargo.toml -- config-check '$USER_TOML'" || ALL_OK=0
run_gate g1_transcript_fixture_json "python3 -c \"import json; d=json.load(open('docs/samples/transcript-fixture.json')); assert 'turns' in d and len(d['turns'])>=1\"" || ALL_OK=0
run_gate g1_offline_static_provider_tests "cargo test -p fin-cli transcript_session_persists_recent_context_history --manifest-path rust/Cargo.toml -- --nocapture" || ALL_OK=0
run_gate g1_context_cache_assembly_tests "cargo test -p fin-runtime --manifest-path rust/Cargo.toml assembler_tests -- --nocapture" || ALL_OK=0
run_gate g1_context_cache_round_loop_tests "cargo test -p fin-runtime --manifest-path rust/Cargo.toml round_loop_runtime_tests -- --nocapture" || ALL_OK=0
run_gate g1_provider_cache_usage_tests "cargo test -p fin-provider --manifest-path rust/Cargo.toml -- --nocapture" || ALL_OK=0
run_gate g1_ws_turn_channel_contract "python3 scripts/android-mvp/run_turn_channel_contract.py" || ALL_OK=0
run_gate g1_ws_event_replay_dynamic "python3 scripts/regression/run_ws_event_replay_gate.py" || ALL_OK=0
run_gate g2_event_render_contract "python3 scripts/regression/check_local_event_render_contract.py" || ALL_OK=0
run_gate g2_android_layout_focus_contract "node android-client/scripts/smoke/layout-focus-contract-smoke.mjs" || ALL_OK=0
run_gate g2_android_log_ingest "python3 scripts/regression/check_android_log_ingest_gate.py" || ALL_OK=0
run_gate g3_live_provider_smoke "scripts/run-real-provider-smoke.sh '$RUN_ID-live'" || ALL_OK=0
run_gate g3_local_multi_agent_live_e2e "scripts/run-local-multi-agent-e2e.sh live '$RUN_ID-live-multi-agent'" || ALL_OK=0
run_gate g3_local_multi_agent_live_webui_render "node scripts/webui/live-runtime-chat-smoke.mjs '$RUN_ID-live-multi-agent'" || ALL_OK=0

python3 - "$STATUS_JSON" "$SUMMARY_MD" "$ALL_OK" <<'PY'
import json, datetime, sys
status, summary, all_ok = sys.argv[1], sys.argv[2], int(sys.argv[3])
obj=json.load(open(status))
obj['ts']=datetime.datetime.now(datetime.UTC).isoformat().replace('+00:00','Z')
obj['overall_ok']=bool(all_ok)
json.dump(obj, open(status,'w'), ensure_ascii=False, indent=2)
lines=['# Local Regression Summary','',f"- run_id: {obj['run_id']}",f"- overall: {'PASS' if obj['overall_ok'] else 'FAIL'}",'', '| gate | status | blocking | log |','|---|---|---|---|']
for g in obj['gates']:
    status = '✅' if g['ok'] else ('⚠️' if not g.get('blocking', True) else '❌')
    lines.append(f"| {g['name']} | {status} | {'yes' if g.get('blocking', True) else 'no'} | `{g['log']}` |")
open(summary,'w').write('\n'.join(lines)+'\n')
PY

if [[ "$ALL_OK" == "1" ]]; then echo PASS; else echo FAIL; fi
exit $((ALL_OK==1?0:1))
