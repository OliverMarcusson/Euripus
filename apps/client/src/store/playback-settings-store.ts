import { create } from "zustand";
import type { LivePlaybackPreference } from "@/lib/hls";

const LIVE_PLAYBACK_PREFERENCE_STORAGE_KEY =
  "euripus-live-playback-preference";

const DEFAULT_LIVE_PLAYBACK_PREFERENCE: LivePlaybackPreference = "stable";

type PlaybackSettingsState = {
  livePlaybackPreference: LivePlaybackPreference;
  setLivePlaybackPreference: (preference: LivePlaybackPreference) => void;
};

function isLivePlaybackPreference(
  value: string | null,
): value is LivePlaybackPreference {
  return value === "stable" || value === "low-latency";
}

function readStoredPreference(): LivePlaybackPreference {
  if (typeof window === "undefined") {
    return DEFAULT_LIVE_PLAYBACK_PREFERENCE;
  }

  const storedPreference = window.localStorage?.getItem(
    LIVE_PLAYBACK_PREFERENCE_STORAGE_KEY,
  );
  return isLivePlaybackPreference(storedPreference)
    ? storedPreference
    : DEFAULT_LIVE_PLAYBACK_PREFERENCE;
}

export const usePlaybackSettingsStore = create<PlaybackSettingsState>((set) => ({
  livePlaybackPreference: readStoredPreference(),
  setLivePlaybackPreference: (livePlaybackPreference) => {
    if (typeof window !== "undefined") {
      window.localStorage?.setItem(
        LIVE_PLAYBACK_PREFERENCE_STORAGE_KEY,
        livePlaybackPreference,
      );
    }
    set({ livePlaybackPreference });
  },
}));

export {
  DEFAULT_LIVE_PLAYBACK_PREFERENCE,
  LIVE_PLAYBACK_PREFERENCE_STORAGE_KEY,
};
