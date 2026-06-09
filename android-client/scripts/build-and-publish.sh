#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
BUILD_TS=$(date +%Y%m%d%H%M%S)
VERSION_NAME="0.1.0.${BUILD_TS}"
VERSION_CODE=$(date +%s)
APK_OUT="app/build/outputs/apk/debug/app-debug.apk"
DIST_DIR="${FIN_UPDATE_DIST:-${HOME}/.fin/update-dist}"

if [ -x ./gradlew ]; then
  ./gradlew :app:assembleDebug \
    -PfinVersionName="${VERSION_NAME}" \
    -PfinVersionCode="${VERSION_CODE}"
elif command -v gradle >/dev/null 2>&1; then
  gradle :app:assembleDebug \
    -PfinVersionName="${VERSION_NAME}" \
    -PfinVersionCode="${VERSION_CODE}"
else
  echo "gradle/gradlew not found" >&2
  exit 2
fi

if [ ! -f "$APK_OUT" ]; then
  echo "apk not found: $APK_OUT" >&2
  exit 3
fi

APK_NAME="fin-${VERSION_NAME}.apk"
mkdir -p "$DIST_DIR"
cp "$APK_OUT" "$DIST_DIR/$APK_NAME"
cp "$APK_OUT" "$DIST_DIR/fin-latest-debug.apk"

SHA=$(shasum -a 256 "$DIST_DIR/$APK_NAME" | awk '{print $1}')
SIZE=$(wc -c < "$DIST_DIR/$APK_NAME" | tr -d ' ')

cat > "$DIST_DIR/latest.json" <<JSON
{
  "versionName": "${VERSION_NAME}",
  "versionCode": ${VERSION_CODE},
  "buildNumber": ${BUILD_TS},
  "apkUrl": "${APK_NAME}",
  "sha256": "${SHA}",
  "size": ${SIZE},
  "notes": [],
  "publishedAt": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "channel": "stable",
  "sourceApk": "app-debug.apk"
}
JSON

echo "published: $DIST_DIR/$APK_NAME"
echo "manifest: $DIST_DIR/latest.json"

echo "update distribution is served by daemon business plane (/updates/* on daemon HTTP port)"
