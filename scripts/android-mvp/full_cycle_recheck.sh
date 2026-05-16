#!/usr/bin/env bash
set -euo pipefail
ROOT=/Volumes/extension/code/fin
LOG=$ROOT/reports/android-mvp-logs
mkdir -p "$LOG"

# 1) ensure daemon listener first
nohup "$ROOT/rust/target/debug/fin-cli" web-debug ~/.fin/config/user.toml 0.0.0.0 4040 > "$LOG/web-debug-live.log" 2>&1 &
echo $! > "$LOG/web-debug.pid"
sleep 1

# 2) rerun full chain
cd "$ROOT"
python3 scripts/android-mvp/preflight_network_diagnose.py
python3 scripts/android-mvp/run_tailscale_live_e2e.py || true
python3 scripts/android-mvp/run_all_gates.py || true
python3 scripts/android-mvp/update_validation_from_gates.py
scripts/android-mvp/build_receipt_bundle.sh
