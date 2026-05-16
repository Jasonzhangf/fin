#!/usr/bin/env bash
set -euo pipefail
ROOT=/Volumes/extension/code/fin
OUT="$ROOT/reports/android-mvp-receipt-bundle"
rm -rf "$OUT"
mkdir -p "$OUT/logs" "$OUT/screenshots"

cp "$ROOT/reports/android-mvp-validation.md" "$OUT/" || true
cp "$ROOT/reports/android-mvp-completion-audit.md" "$OUT/" || true
cp "$ROOT/reports/android-mvp-network-blocker.md" "$OUT/" || true
cp "$ROOT/reports/android-mvp-gate-summary.md" "$OUT/" || true

for f in \
  all-gates-status.json \
  tailscale-live-e2e-status.json \
  tailscale-connection-events.log \
  live-unblock-observation.json \
  gate-connection_matrix.log \
  gate-session_input_matrix.log \
  gate-capture_shell_screenshots.log \
  gate-assemble_debug.log \
  gate-build_publish.log \
  gate-tailscale_live_e2e.log \
  web-debug-live.log \
  adb-install.log \
  adb-start-foreground.log
  do
  cp "$ROOT/reports/android-mvp-logs/$f" "$OUT/logs/" 2>/dev/null || true
 done

cp "$ROOT/reports/android-mvp-screenshots/tailscale-live-e2e.png" "$OUT/screenshots/" 2>/dev/null || true
cp "$ROOT/reports/android-mvp-screenshots/device-foreground-check.png" "$OUT/screenshots/" 2>/dev/null || true

TS=$(date +%Y%m%d-%H%M%S)
TAR="$ROOT/reports/android-mvp-receipt-bundle-$TS.tgz"
tar -czf "$TAR" -C "$ROOT/reports" "android-mvp-receipt-bundle"
echo "$TAR"
