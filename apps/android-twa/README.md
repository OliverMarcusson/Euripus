# Euripus Android TWA

This folder contains the Trusted Web Activity scaffolding for shipping the hosted Euripus PWA on Android phones, tablets, and Android TV devices.

## Build an Android TV APK

From the workspace root:

```powershell
bun run build:android-tv
```

That script will:

- generate the missing PNG launcher icons if needed
- create the Bubblewrap Android wrapper under `apps/android-twa/android`
- patch the generated Android manifest with Android TV launcher support
- build a debuggable APK at `apps/android-twa/android/app/build/outputs/apk/debug/app-debug.apk`

## Expected setup

1. Host the web build over HTTPS.
2. Publish `/.well-known/assetlinks.json` from the deployed origin.
3. Generate the Android wrapper with Bubblewrap using `bubblewrap.config.json`.
4. Replace the signing placeholders before building a release APK or AAB.

## Notes

- Android TV navigation depends on the web app's `TV mode`; the wrapper does not provide a separate TV UI.
- Keep the launcher origin and package name aligned with the deployed app and Digital Asset Links file.
