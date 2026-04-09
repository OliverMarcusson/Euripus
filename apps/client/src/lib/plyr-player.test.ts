import type { PlaybackSource } from "@euripus/shared";
import Plyr from "plyr";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { createIptvHls, isIptvHlsSupported } from "@/lib/hls";
import { bindPlaybackSource } from "@/lib/plyr-player";

const plyrDestroy = vi.fn();
const hlsDestroy = vi.fn();
const onQualitiesChanged = vi.fn(() => vi.fn());
const getCurrentQuality = vi.fn(() => 0);
const setQuality = vi.fn();

vi.mock("plyr", () => ({
  default: vi.fn().mockImplementation(() => ({
    destroy: plyrDestroy,
  })),
}));

vi.mock("@/lib/hls", () => ({
  createIptvHls: vi.fn(() => ({
    destroy: hlsDestroy,
    getCurrentQuality,
    onQualitiesChanged,
    qualityOptions: [],
    setQuality,
  })),
  isIptvHlsSupported: vi.fn(),
  AUTO_HLS_QUALITY: 0,
}));

const HLS_SOURCE: PlaybackSource = {
  kind: "hls",
  url: "https://example.com/live.m3u8",
  headers: {},
  live: true,
  catchup: false,
  expiresAt: null,
  unsupportedReason: null,
  title: "Arena Live",
};

describe("bindPlaybackSource", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(isIptvHlsSupported).mockReturnValue(true);
    vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => {});
    vi.spyOn(HTMLMediaElement.prototype, "play").mockResolvedValue();
  });

  it("creates Plyr and Hls.js sessions for HLS playback", () => {
    const video = document.createElement("video");

    const session = bindPlaybackSource(video, HLS_SOURCE, { uiMode: "local" });

    expect(Plyr).toHaveBeenCalledTimes(1);
    expect(createIptvHls).toHaveBeenCalledWith(video, HLS_SOURCE.url, {
      live: true,
    });
    expect(onQualitiesChanged).toHaveBeenCalledTimes(1);
    expect(session.plyr).toBeDefined();
  });

  it("uses a direct video src when Hls.js is not available", () => {
    const video = document.createElement("video");
    vi.mocked(isIptvHlsSupported).mockReturnValue(false);

    bindPlaybackSource(video, { ...HLS_SOURCE, kind: "mpegts", live: false });

    expect(createIptvHls).not.toHaveBeenCalled();
    expect(video.src).toBe("https://example.com/live.m3u8");
  });

  it("uses receiver-specific Plyr controls for the receiver surface", () => {
    const video = document.createElement("video");

    bindPlaybackSource(video, HLS_SOURCE, { uiMode: "receiver" });

    expect(Plyr).toHaveBeenCalledWith(
      video,
      expect.objectContaining({
        clickToPlay: false,
        controls: [],
        fullscreen: expect.objectContaining({ enabled: false }),
      }),
    );
  });

  it("tears down Hls.js and Plyr and clears the media element", () => {
    const video = document.createElement("video");
    video.src = HLS_SOURCE.url;

    const session = bindPlaybackSource(video, HLS_SOURCE, { uiMode: "local" });
    session.destroy();

    expect(hlsDestroy).toHaveBeenCalledTimes(1);
    expect(plyrDestroy).toHaveBeenCalledTimes(1);
    expect(video.getAttribute("src")).toBeNull();
  });

  it("adds quality settings and PiP controls when multiple HLS levels exist", () => {
    const video = document.createElement("video");
    vi.mocked(createIptvHls).mockReturnValue({
      destroy: hlsDestroy,
      getCurrentQuality,
      onQualitiesChanged(listener) {
        listener([
          { value: 1080, label: "1080p", level: 0, bitrate: 4_500_000 },
          { value: 720, label: "720p", level: 1, bitrate: 3_100_000 },
        ]);
        return vi.fn();
      },
      qualityOptions: [
        { value: 1080, label: "1080p", level: 0, bitrate: 4_500_000 },
        { value: 720, label: "720p", level: 1, bitrate: 3_100_000 },
      ],
      setQuality,
    });

    bindPlaybackSource(video, HLS_SOURCE, { uiMode: "local" });

    expect(Plyr).toHaveBeenLastCalledWith(
      video,
      expect.objectContaining({
        controls: expect.arrayContaining(["settings", "pip"]),
        quality: expect.objectContaining({
          options: [0, 1080, 720],
        }),
        settings: ["quality"],
      }),
    );
  });

  it("can bind a second session after destroying the first one", () => {
    const video = document.createElement("video");

    const first = bindPlaybackSource(video, HLS_SOURCE, { uiMode: "local" });
    first.destroy();
    const second = bindPlaybackSource(video, { ...HLS_SOURCE, url: "https://example.com/other.m3u8" }, { uiMode: "local" });

    expect(Plyr).toHaveBeenCalledTimes(2);
    expect(plyrDestroy).toHaveBeenCalledTimes(1);
    expect(second.plyr).toBeDefined();
  });
});
