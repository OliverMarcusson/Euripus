import {
  LIVE_PLAYBACK_PREFERENCE_STORAGE_KEY,
  usePlaybackSettingsStore,
} from "@/store/playback-settings-store";

describe("playback settings store", () => {
  beforeEach(() => {
    window.localStorage.clear();
    usePlaybackSettingsStore.setState({ livePlaybackPreference: "stable" });
  });

  it("defaults to stable playback", () => {
    expect(
      usePlaybackSettingsStore.getState().livePlaybackPreference,
    ).toBe("stable");
  });

  it("persists the low-latency preference", () => {
    usePlaybackSettingsStore
      .getState()
      .setLivePlaybackPreference("low-latency");

    expect(
      usePlaybackSettingsStore.getState().livePlaybackPreference,
    ).toBe("low-latency");
    expect(
      window.localStorage.getItem(LIVE_PLAYBACK_PREFERENCE_STORAGE_KEY),
    ).toBe("low-latency");
  });
});
