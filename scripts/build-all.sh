#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$SCRIPT_DIR"

echo "=== Step 1: Build and install fin daemon globally ==="
"$SCRIPT_DIR/install-fin-global.sh"

echo ""
echo "=== Step 2: Build Android APK ==="
cd "$ROOT_DIR/android-client"
./gradlew :app:assembleDebug
mkdir -p update-dist
cp app/build/outputs/apk/debug/app-debug.apk update-dist/fin-latest-debug.apk
echo "APK built: $ROOT_DIR/android-client/update-dist/fin-latest-debug.apk"

echo ""
echo "=== Build complete! ==="
echo "Daemon: ~/.fin/bin/fin (global)"
echo "Global CLI: ${FIN_GLOBAL_BIN_DIR:-$HOME/.local/bin}/fin"
echo "APK: $ROOT_DIR/android-client/update-dist/fin-latest-debug.apk"
