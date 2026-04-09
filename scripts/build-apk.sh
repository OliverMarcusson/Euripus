#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ANDROID_APP_PATH="$(cd "$SCRIPT_DIR/../apps/android-tv-native" && pwd)"
SUPPORTED_ARCHITECTURES=("armeabi-v7a" "arm64-v8a" "x86" "x86_64")
ARCHITECTURE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --architecture|-a)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for $1" >&2
        exit 1
      fi
      ARCHITECTURE="$2"
      shift 2
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

cd "$ANDROID_APP_PATH"

if [[ ! -x "./gradlew" ]]; then
  chmod +x ./gradlew
fi

GRADLE_ARGS=("assembleDebug")

if [[ -n "$ARCHITECTURE" ]]; then
  IFS=',' read -r -a SELECTED_ARCHITECTURES <<< "$ARCHITECTURE"
  NORMALIZED_ARCHITECTURES=()
  for arch in "${SELECTED_ARCHITECTURES[@]}"; do
    arch="${arch//[[:space:]]/}"
    if [[ -z "$arch" ]]; then
      continue
    fi
    if [[ ! " ${SUPPORTED_ARCHITECTURES[*]} " =~ (^|[[:space:]])"$arch"($|[[:space:]]) ]]; then
      echo "Unsupported architecture: $arch. Supported values: ${SUPPORTED_ARCHITECTURES[*]}" >&2
      exit 1
    fi
    NORMALIZED_ARCHITECTURES+=("$arch")
  done

  if [[ ${#NORMALIZED_ARCHITECTURES[@]} -eq 0 ]]; then
    echo "Architecture cannot be empty." >&2
    exit 1
  fi

  echo "Building APK for architecture(s): ${NORMALIZED_ARCHITECTURES[*]}"
  GRADLE_ARGS+=("-Peuripus.targetAbis=$(IFS=,; echo "${NORMALIZED_ARCHITECTURES[*]}")")
else
  echo "Building universal APK (all supported architectures)"
fi

./gradlew "${GRADLE_ARGS[@]}"

APK_PATH="$ANDROID_APP_PATH/app/build/outputs/apk/debug/app-debug.apk"
if [[ ! -f "$APK_PATH" ]]; then
  echo "APK not found at $APK_PATH" >&2
  exit 1
fi

echo "APK built successfully:"
echo "$APK_PATH"
