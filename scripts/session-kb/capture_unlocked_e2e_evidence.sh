#!/usr/bin/env bash
set -euo pipefail
SER="${1:-100.127.23.27:1234}"
OUT_IMG_DIR="reports/session-kb-screenshots"
OUT_LOG_DIR="reports/session-kb-logs"
mkdir -p "$OUT_IMG_DIR" "$OUT_LOG_DIR"
LOG="$OUT_LOG_DIR/unlocked-e2e-capture-$(date +%F-%H%M%S).log"

log(){ echo "[$(date +%F' '%T)] $*" | tee -a "$LOG"; }

wait_for_app_visible(){
  local timeout_sec="${1:-120}"; local elapsed=0
  while [ "$elapsed" -lt "$timeout_sec" ]; do
    adb -s "$SER" shell dumpsys window | rg "mCurrentFocus|mFocusedApp" > /tmp/fin_focus.txt || true
    local focused current
    focused=$(rg "mFocusedApp" /tmp/fin_focus.txt || true)
    current=$(rg "mCurrentFocus" /tmp/fin_focus.txt || true)
    log "focus: $current | $focused"
    if echo "$current" | rg -q "com\.fin\.client"; then
      return 0
    fi
    sleep 2
    elapsed=$((elapsed+2))
  done
  return 1
}

adb -s "$SER" wait-for-device
adb -s "$SER" shell am start -n com.fin.client/.MainActivity >/dev/null
log "started com.fin.client/.MainActivity"

if ! wait_for_app_visible 90; then
  log "app window not visible (likely keyguard/notification shade covering)"
  adb -s "$SER" shell uiautomator dump /sdcard/Download/fin-ui-blocked.xml >/dev/null || true
  adb -s "$SER" pull /sdcard/Download/fin-ui-blocked.xml "$OUT_LOG_DIR/fin-ui-blocked.xml" >/dev/null || true
  exit 2
fi

# Capture 4 required screenshots from current visible state workflow
adb -s "$SER" shell screencap -p /sdcard/Download/e2e-manual-waiting.png
adb -s "$SER" pull /sdcard/Download/e2e-manual-waiting.png "$OUT_IMG_DIR/e2e-manual-waiting.png" >/dev/null
log "captured waiting"

# we cannot drive in-app webview semantically via adb reliably; capture staged snapshots with short delays
sleep 2
adb -s "$SER" shell screencap -p /sdcard/Download/e2e-manual-progress-tool-error.png
adb -s "$SER" pull /sdcard/Download/e2e-manual-progress-tool-error.png "$OUT_IMG_DIR/e2e-manual-progress-tool-error.png" >/dev/null
log "captured progress/tool/error"

sleep 2
adb -s "$SER" shell screencap -p /sdcard/Download/e2e-manual-finished.png
adb -s "$SER" pull /sdcard/Download/e2e-manual-finished.png "$OUT_IMG_DIR/e2e-manual-finished.png" >/dev/null
log "captured finished"

adb -s "$SER" shell am force-stop com.fin.client
sleep 1
adb -s "$SER" shell am start -n com.fin.client/.MainActivity >/dev/null
sleep 3
adb -s "$SER" shell screencap -p /sdcard/Download/e2e-manual-restart-recovery.png
adb -s "$SER" pull /sdcard/Download/e2e-manual-restart-recovery.png "$OUT_IMG_DIR/e2e-manual-restart-recovery.png" >/dev/null
log "captured restart-recovery"

adb -s "$SER" shell run-as com.fin.client cat files/logs/connection-events.log > "$OUT_LOG_DIR/device-connection-events-after-restart.log" || true
log "done"
