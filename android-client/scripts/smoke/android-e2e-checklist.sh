#!/usr/bin/env bash
set -euo pipefail
OUT_DIR="${1:-reports/android-e2e-$(date +%Y%m%d-%H%M%S)}"
PKG="com.fin.client"
ACT="com.fin.client/.MainActivity"
mkdir -p "$OUT_DIR"

echo "[1] adb devices" | tee "$OUT_DIR/steps.log"
adb devices | tee "$OUT_DIR/adb-devices.txt"

echo "[2] install debug apk" | tee -a "$OUT_DIR/steps.log"
adb install -r app/build/outputs/apk/debug/app-debug.apk | tee "$OUT_DIR/install.txt"

echo "[3] clear logs + launch" | tee -a "$OUT_DIR/steps.log"
adb logcat -c
adb shell am start -n "$ACT" | tee "$OUT_DIR/launch.txt"
sleep 5

echo "[4] capture app logs" | tee -a "$OUT_DIR/steps.log"
adb logcat -d | grep -E "FinMobileBridge|fin-main|update\.selftest|state=" > "$OUT_DIR/app-logcat.txt" || true

echo "[5] ws config snapshot presence" | tee -a "$OUT_DIR/steps.log"
grep -n "config.snapshot\|protocol_unknown_event\|state=healthy\|state=reconnecting" "$OUT_DIR/app-logcat.txt" > "$OUT_DIR/ws-check.txt" || true

echo "[6] upgrade chain selftest trigger" | tee -a "$OUT_DIR/steps.log"
adb shell am start -n "$ACT" --ez updateSelfTest true --es manifestUrl internal://latest --es baseUrl internal://files/ | tee "$OUT_DIR/upgrade-selftest-launch.txt"
sleep 6
adb logcat -d | grep -E "update\.selftest\.(start|check|download|install|done)" > "$OUT_DIR/upgrade-selftest-log.txt" || true

echo "DONE: $OUT_DIR"
