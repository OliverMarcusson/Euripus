#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ANDROID_APP_PATH="$(cd "$SCRIPT_DIR/../apps/android-tv-native" && pwd)"

cd "$ANDROID_APP_PATH"

if [[ ! -x "./gradlew" ]]; then
  chmod +x ./gradlew
fi

./gradlew assembleDebug

APK_PATH="$ANDROID_APP_PATH/app/build/outputs/apk/debug/app-debug.apk"
if [[ ! -f "$APK_PATH" ]]; then
  echo "APK not found at $APK_PATH" >&2
  exit 1
fi

echo "APK built successfully:"
echo "$APK_PATH"
