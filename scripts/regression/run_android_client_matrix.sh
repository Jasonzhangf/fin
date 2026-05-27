#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
APP_DIR="$ROOT/android-client"
cd "$APP_DIR"

run_case() {
  local name="$1"; shift
  echo "[android-matrix] case=$name"
  "$@"
}

run_case unit_test ./gradlew test --no-daemon
run_case assemble_debug ./gradlew assembleDebug --no-daemon
run_case ws_event_contract_smoke node scripts/smoke/ws-event-contract-smoke.mjs
run_case layout_focus_contract_smoke node scripts/smoke/layout-focus-contract-smoke.mjs
run_case turn_channel_e2e python3 ../scripts/android-mvp/run_turn_channel_e2e.py
run_case projection_contract_check node scripts/smoke/projection-contract-check.mjs ../reports/android-mvp-logs/turn-channel-e2e.log

echo "[android-matrix] all passed"
