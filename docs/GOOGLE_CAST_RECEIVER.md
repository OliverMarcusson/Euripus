# Google Cast Receiver Registration

Euripus uses a Custom Web Receiver hosted at:

`https://tv.marcusson.dev/receiver?cast=1`

The receiver initializes Google's Cast Application Framework, creates an Euripus receiver session, and receives relay-backed playback through Euripus's existing remote-control APIs. The authenticated sender automatically registers and remembers the selected Cast device; no pairing-code entry is required.

## Register the application

1. Open the [Google Cast SDK Developer Console](https://cast.google.com/publish/) and sign in with the Google account that will own the application.
2. Complete developer-account registration if prompted. Google currently charges a non-refundable USD 5 registration fee.
3. Select **Add New Application**.
4. Select **Custom Receiver**.
5. Enter:
   - **Name:** `Euripus Receiver`
   - **Receiver URL:** `https://tv.marcusson.dev/receiver?cast=1`
6. Save the application and copy the generated application ID.
7. During testing, leave the application unpublished. Publish it only when it should be available to Cast devices not registered to the developer account.

## Register a test device

An unpublished receiver only launches on registered development devices.

1. Connect the Cast device and your computer to the same network.
2. In the Cast Developer Console, open **Devices** and select **Add New Device**.
3. Enter the device's Cast serial number and a description.
   - For Chromecast, cast the Developer Console tab to the device; the serial appears and is read aloud.
   - For Google TV or Android TV, use the software Cast serial—not the hardware serial. It can also be found under the device's Cast settings after enabling developer mode.
4. Save and wait approximately 15 minutes, until the device status becomes **Ready for Testing**.
5. Reboot the Cast device before the first test if Google still reports that the receiver is unavailable.

## Euripus application ID

The registered Cast application ID is configured in `apps/client/src/lib/google-cast.ts`:

```ts
export const EURIPUS_CAST_RECEIVER_APP_ID = "EEC1D3B6";
```

The App ID is public sender configuration, not a secret.

## Test the flow

1. Open Euripus in a supported Chromium browser on the same network as the registered Cast device.
2. Open **Playback device** and choose **Open Euripus receiver...**.
3. Select the Cast device.
4. Euripus automatically registers, remembers, and selects the Cast receiver. The TV briefly displays a connecting screen; no pairing code is shown.
5. Content selection and play, pause, seek, and stop then use the normal Euripus receiver controls.

## Troubleshooting

- `APP_NOT_INSTALLED` usually means App ID `EEC1D3B6` is not available to the device, the unpublished app and test device belong to different developer accounts, or device registration has not propagated.
- Register the canonical `tv.marcusson.dev` URL directly; do not register the redirecting `pb.marcusson.dev` URL.
- The receiver URL must remain publicly reachable over HTTPS.

Official references:

- [Register Cast applications and devices](https://developers.google.com/cast/docs/registration)
- [Build a Custom Web Receiver](https://developers.google.com/cast/docs/web_receiver/basic)
- [Integrate the Web Sender SDK](https://developers.google.com/cast/docs/web_sender/integrate)
