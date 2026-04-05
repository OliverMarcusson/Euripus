import { useEffect } from "react";
import type { RemoteDeviceEventPayload } from "@/lib/remote-events";
import {
  API_BASE_URL,
  acknowledgeRemoteCommand,
  heartbeatPlaybackDevice,
  registerPlaybackDevice,
} from "@/lib/api";
import { useAuthStore } from "@/store/auth-store";
import { usePlaybackDeviceStore } from "@/store/playback-device-store";
import { usePlayerStore } from "@/store/player-store";

const REMOTE_DEVICE_HEARTBEAT_MS = 15_000;

function buildEventsUrl(deviceId: string, accessToken: string) {
  const baseUrl =
    typeof window === "undefined" ? API_BASE_URL : new URL(API_BASE_URL, window.location.origin).toString();
  const url = new URL(`${baseUrl}/remote/devices/${deviceId}/events`);
  url.searchParams.set("accessToken", accessToken);
  return url.toString();
}

export function usePlaybackDeviceRegistration() {
  const user = useAuthStore((state) => state.user);
  const hydrated = useAuthStore((state) => state.hydrated);
  const accessToken = useAuthStore((state) => state.accessToken);
  const setSource = usePlayerStore((state) => state.setSource);
  const deviceKey = usePlaybackDeviceStore((state) => state.deviceKey);
  const name = usePlaybackDeviceStore((state) => state.name);
  const platform = usePlaybackDeviceStore((state) => state.platform);
  const formFactorHint = usePlaybackDeviceStore((state) => state.formFactorHint);
  const remoteTargetEnabled = usePlaybackDeviceStore((state) => state.remoteTargetEnabled);
  const setActiveDeviceId = usePlaybackDeviceStore((state) => state.setActiveDeviceId);

  useEffect(() => {
    if (!hydrated) {
      return;
    }

    if (!user || !accessToken) {
      setActiveDeviceId(null);
      return;
    }
    const token = accessToken;

    let active = true;
    let heartbeatTimer: number | null = null;
    let events: EventSource | null = null;

    async function register() {
      try {
        const device = await registerPlaybackDevice({
          deviceKey,
          name: name.trim() || "Playback device",
          platform,
          formFactorHint,
          remoteTargetEnabled,
        });
        if (!active) {
          return;
        }

        setActiveDeviceId(device.id);

        if (!remoteTargetEnabled) {
          return;
        }

        const sendHeartbeat = () => {
          void heartbeatPlaybackDevice(device.id).catch(() => undefined);
        };

        sendHeartbeat();
        heartbeatTimer = window.setInterval(sendHeartbeat, REMOTE_DEVICE_HEARTBEAT_MS);

        events = new EventSource(buildEventsUrl(device.id, token), { withCredentials: true });
        events.addEventListener("playback_command", (event) => {
          const payload = JSON.parse((event as MessageEvent<string>).data) as RemoteDeviceEventPayload;
          setSource(payload.source);
          void acknowledgeRemoteCommand(device.id, payload.command.id, { status: "acknowledged" }).catch(
            () => undefined,
          );
        });
      } catch {
        if (active) {
          setActiveDeviceId(null);
        }
      }
    }

    void register();

    return () => {
      active = false;
      if (heartbeatTimer !== null) {
        window.clearInterval(heartbeatTimer);
      }
      events?.close();
    };
  }, [
    accessToken,
    deviceKey,
    formFactorHint,
    hydrated,
    name,
    platform,
    remoteTargetEnabled,
    setActiveDeviceId,
    setSource,
    user,
  ]);
}
