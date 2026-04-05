import { useEffect } from "react";
import { updateRemotePlaybackDeviceState } from "@/lib/api";
import { useAuthStore } from "@/store/auth-store";
import { usePlaybackDeviceStore } from "@/store/playback-device-store";
import { usePlayerStore } from "@/store/player-store";

export function useRemotePlaybackStateSync() {
  const user = useAuthStore((state) => state.user);
  const activeDeviceId = usePlaybackDeviceStore((state) => state.activeDeviceId);
  const remoteTargetEnabled = usePlaybackDeviceStore((state) => state.remoteTargetEnabled);
  const source = usePlayerStore((state) => state.source);

  useEffect(() => {
    if (!user || !activeDeviceId || !remoteTargetEnabled) {
      return;
    }

    void updateRemotePlaybackDeviceState(activeDeviceId, {
      title: source?.title ?? null,
      sourceKind: source?.kind ?? null,
      live: source?.live ?? null,
      catchup: source?.catchup ?? null,
    }).catch(() => undefined);
  }, [
    activeDeviceId,
    remoteTargetEnabled,
    source?.catchup,
    source?.kind,
    source?.live,
    source?.title,
    user,
  ]);
}
