#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$SCRIPT_DIR"

if [[ -z "${JAVA_HOME:-}" && -x "/Applications/Android Studio.app/Contents/jbr/Contents/Home/bin/java" ]]; then
  export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
  export PATH="$JAVA_HOME/bin:$PATH"
fi

echo "=== Step 1: Build and install fin daemon globally ==="
"$SCRIPT_DIR/install-fin-global.sh"

echo ""
echo "=== Step 2: Run local regression gates ==="
cd "$ROOT_DIR"
scripts/regression/run_local_regression.sh

echo ""
echo "=== Step 3: Build Android APK ==="
cd "$ROOT_DIR/android-client"
node scripts/smoke/ws-event-contract-smoke.mjs
scripts/build-and-publish.sh
RUNTIME_UPDATE_DIST="${FIN_HOME:-$HOME/.fin}/update-dist"
if [[ -L "$RUNTIME_UPDATE_DIST" ]]; then
  rm "$RUNTIME_UPDATE_DIST"
fi
mkdir -p "$RUNTIME_UPDATE_DIST"
cp update-dist/latest.json update-dist/*.apk "$RUNTIME_UPDATE_DIST/"
echo "APK built: $ROOT_DIR/android-client/update-dist/fin-latest-debug.apk"
echo "Manifest built: $ROOT_DIR/android-client/update-dist/latest.json"
echo "Runtime update dist: $RUNTIME_UPDATE_DIST"

echo ""
echo "=== Build complete! ==="
echo "Daemon: ~/.fin/bin/fin (global)"
echo "Global CLI: ${FIN_GLOBAL_BIN_DIR:-$HOME/.local/bin}/fin"
echo "APK: $ROOT_DIR/android-client/update-dist/fin-latest-debug.apk"
echo "Manifest: $ROOT_DIR/android-client/update-dist/latest.json"
echo "Runtime update dist: ${FIN_HOME:-$HOME/.fin}/update-dist"
