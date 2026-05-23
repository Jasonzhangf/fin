#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
BUILD_TS=$(date +%Y%m%d%H%M%S)
VERSION_NAME="0.1.0.${BUILD_TS}"
VERSION_CODE=$(date +%s)
APK_OUT="app/build/outputs/apk/debug/app-debug.apk"

if [ -x ./gradlew ]; then
  ./gradlew :app:assembleDebug
elif command -v gradle >/dev/null 2>&1; then
  gradle :app:assembleDebug
else
  echo "gradle/gradlew not found" >&2
  exit 2
fi

if [ ! -f "$APK_OUT" ]; then
  echo "apk not found: $APK_OUT" >&2
  exit 3
fi

APK_NAME="fin-${VERSION_NAME}.apk"
cp "$APK_OUT" "update-dist/$APK_NAME"
cp "$APK_OUT" "update-dist/fin-latest-debug.apk"

SHA=$(shasum -a 256 "update-dist/$APK_NAME" | awk '{print $1}')
SIZE=$(wc -c < "update-dist/$APK_NAME" | tr -d ' ')

cat > update-dist/latest.json <<JSON
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

echo "published: update-dist/$APK_NAME"
echo "manifest: update-dist/latest.json"

echo "update distribution is served by daemon business plane (/updates/* on daemon HTTP port)"
