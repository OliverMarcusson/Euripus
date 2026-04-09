import Hls, { type ErrorData } from "hls.js";
import { describe, expect, it, vi } from "vitest";
import {
  IPTV_HLS_CONFIG,
  getIptvHlsQualityLabel,
  getIptvHlsQualityOptions,
  handleIptvHlsError,
  syncLivePlaybackPosition,
  updateLivePlaybackRate,
} from "@/lib/hls";

describe("IPTV HLS helpers", () => {
  it("uses the tuned IPTV buffering configuration", () => {
    expect(IPTV_HLS_CONFIG).toMatchObject({
      lowLatencyMode: false,
      liveSyncDurationCount: 10,
      liveMaxLatencyDurationCount: 20,
      maxBufferLength: 60,
      backBufferLength: 90,
      nudgeOnVideoHole: true,
      manifestLoadingTimeOut: 15_000,
      fragLoadingTimeOut: 25_000,
    });
  });

  it("restarts loading on non-fatal network errors", () => {
    const controller = {
      destroy: vi.fn(),
      recoverMediaError: vi.fn(),
      startLoad: vi.fn(),
    };

    handleIptvHlsError(
      controller,
      { type: Hls.ErrorTypes.NETWORK_ERROR, fatal: false } as ErrorData,
      { mediaRecoveryAttempts: 0 },
    );

    expect(controller.startLoad).toHaveBeenCalledTimes(1);
    expect(controller.recoverMediaError).not.toHaveBeenCalled();
    expect(controller.destroy).not.toHaveBeenCalled();
  });

  it("recovers a media error once before destroying the instance", () => {
    const controller = {
      destroy: vi.fn(),
      recoverMediaError: vi.fn(),
      startLoad: vi.fn(),
    };
    const recoveryState = { mediaRecoveryAttempts: 0 };

    handleIptvHlsError(
      controller,
      { type: Hls.ErrorTypes.MEDIA_ERROR, fatal: false } as ErrorData,
      recoveryState,
    );
    handleIptvHlsError(
      controller,
      { type: Hls.ErrorTypes.MEDIA_ERROR, fatal: false } as ErrorData,
      recoveryState,
    );

    expect(controller.recoverMediaError).toHaveBeenCalledTimes(1);
    expect(controller.destroy).toHaveBeenCalledTimes(1);
  });

  it("requests external recovery on fatal errors", () => {
    const controller = {
      destroy: vi.fn(),
      recoverMediaError: vi.fn(),
      startLoad: vi.fn(),
    };
    const onFatalRecoveryNeeded = vi.fn();

    handleIptvHlsError(
      controller,
      { type: Hls.ErrorTypes.NETWORK_ERROR, fatal: true } as ErrorData,
      { mediaRecoveryAttempts: 0 },
      { onFatalRecoveryNeeded },
    );

    expect(onFatalRecoveryNeeded).toHaveBeenCalledTimes(1);
    expect(controller.destroy).not.toHaveBeenCalled();
    expect(controller.startLoad).not.toHaveBeenCalled();
    expect(controller.recoverMediaError).not.toHaveBeenCalled();
  });

  it("does not force-seek live playback toward the live sync position", () => {
    const video = {
      currentTime: 100,
    } as unknown as HTMLVideoElement;

    syncLivePlaybackPosition(video, { liveSyncPosition: 106 });

    expect(video.currentTime).toBe(100);
  });

  it("does not speed up live playback to chase the live edge", () => {
    const video = {
      currentTime: 100,
      paused: false,
      playbackRate: 1,
      buffered: {
        length: 1,
        end: () => 103,
      },
    } as unknown as HTMLVideoElement;

    updateLivePlaybackRate(video, { liveSyncPosition: 102.5 });

    expect(video.playbackRate).toBe(1);
  });

  it("formats quality labels from resolution or bitrate", () => {
    expect(
      getIptvHlsQualityLabel({ height: 1080, bitrate: 4_200_000 }),
    ).toBe("1080p");
    expect(
      getIptvHlsQualityLabel({ height: 0, bitrate: 768_000 }),
    ).toBe("768 kbps");
  });

  it("deduplicates HLS quality options by visible label value", () => {
    expect(
      getIptvHlsQualityOptions([
        { height: 720, bitrate: 2_500_000 },
        { height: 1080, bitrate: 4_500_000 },
        { height: 720, bitrate: 3_100_000 },
      ]),
    ).toEqual([
      { value: 1080, label: "1080p", level: 1, bitrate: 4_500_000 },
      { value: 720, label: "720p", level: 2, bitrate: 3_100_000 },
    ]);
  });
});
