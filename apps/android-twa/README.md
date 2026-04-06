# Euripus Android TWA

This folder contains the Trusted Web Activity scaffolding for shipping the hosted Euripus PWA on Android phones, tablets, and Android TV devices.

## Build an Android TV APK

From the workspace root:

```powershell
bun run build:android-tv
```

That script will:

- regenerate the PWA icons from `Euripus-icon.png` when that source file is present
- regenerate the Android TV launcher icon from `Euripus-atv-icon.png` when that source file is present
- create the Bubblewrap Android wrapper under `apps/android-twa/android`
- patch the generated Android manifest with Android TV launcher support
- build a debuggable APK at `apps/android-twa/android/app/build/outputs/apk/debug/app-debug.apk`
- build a signed release APK at `apps/android-twa/android/app/build/outputs/apk/release/app-release-signed.apk`
- keep the local release keystore and signing settings under `apps/android-twa/.generated/signing/`

## Expected setup

1. Host the web build over HTTPS.
2. Publish `/.well-known/assetlinks.json` from the deployed origin.
3. Generate the Android wrapper with Bubblewrap using `bubblewrap.config.json`.
4. Keep the generated keystore and signing properties safe if you want future updates to install over the same app on your Android TV.

## Notes

- Android TV now launches directly into the `/receiver` experience so pairing is the supported TV flow.
- Keep the launcher origin and package name aligned with the deployed app and Digital Asset Links file.
