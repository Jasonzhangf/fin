#!/usr/bin/env bash
set -euo pipefail
ADB=${1:-100.127.23.27:1234}
OUT_DIR=${2:-reports/session-kb-screenshots}
mkdir -p "$OUT_DIR"

is_locked() {
  adb -s "$ADB" shell dumpsys window policy 2>/dev/null | grep -q 'isStatusBarKeyguard=true'
}

echo "[capture] waiting for unlocked screen on $ADB ..."
for i in $(seq 1 180); do
  if ! is_locked; then
    ts=$(date +%F-%H%M%S)
    adb -s "$ADB" shell screencap -p /sdcard/Download/fin-unlocked-${ts}.png >/dev/null
    adb -s "$ADB" pull /sdcard/Download/fin-unlocked-${ts}.png "$OUT_DIR/" >/dev/null
    echo "[capture] unlocked screenshot saved: $OUT_DIR/fin-unlocked-${ts}.png"
    exit 0
  fi
  sleep 1
done

echo "[capture] timeout: device still locked"
exit 2
