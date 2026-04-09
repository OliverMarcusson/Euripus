# Euripus Android TV Receiver

This module now contains the native Android TV receiver app for Euripus.

Implemented in this pass:

- full Android app project structure with Gradle Kotlin DSL
- first-run server URL setup that normalizes public origins to `/api`
- persisted receiver identity and remembered-device credential storage
- receiver session bootstrap, heartbeat, SSE event handling, and command acknowledgement
- Media3 / ExoPlayer playback for relay-backed HLS and MPEG-TS streams
- Compose-based TV UI for setup, pairing, idle, playback, unsupported streams, and recovery

## Expected Flow

1. Launch the app on Android TV.
2. Enter the public Euripus server URL on first run.
3. The app creates a receiver session and shows a pairing code if the device is not remembered.
4. Pair from the Euripus web client.
5. Start playback from the web client and control it with the normal remote receiver flow.

## Build Notes

- The app expects Java 17.
- The project includes Gradle wrapper scripts and wrapper properties.
- If `gradle/wrapper/gradle-wrapper.jar` is missing in your checkout, regenerate or restore it before building.
- For local-network testing, the receiver accepts LAN and localhost-style dev URLs such as `http://192.168.1.42:5173` and automatically targets the same `/api` receiver endpoints on that host.
- You can build a universal APK or restrict native dependencies to specific ABI targets with `-Peuripus.targetAbis=<abi[,abi...]>`.
- Supported ABI values are `armeabi-v7a`, `arm64-v8a`, `x86`, and `x86_64`.
- Helper scripts support this directly:
  - PowerShell: `.\scripts\build-apk.ps1 -Architecture x86_64`
  - Bash: `./scripts/build-apk.sh --architecture x86_64`
- Root workspace command:
  - `bun run build:apk` opens an interactive radio picker for `x86 (x86_64)` or `arm64`
  - `bun run build:apk --architecture x86_64` skips the picker
- Direct Gradle examples:
  - PowerShell: `& .\gradlew.bat 'assembleDebug' '-Peuripus.targetAbis=x86_64'`
  - Bash: `./gradlew assembleDebug -Peuripus.targetAbis=x86_64`
