import Hls, { type ErrorData } from "hls.js";
import { describe, expect, it, vi } from "vitest";
import {
  IPTV_HLS_CONFIG,
  handleIptvHlsError,
  syncLivePlaybackPosition,
  updateLivePlaybackRate,
} from "@/lib/hls";

describe("IPTV HLS helpers", () => {
  it("uses the tuned IPTV buffering configuration", () => {
    expect(IPTV_HLS_CONFIG).toMatchObject({
      lowLatencyMode: false,
      liveSyncDurationCount: 1,
      liveMaxLatencyDurationCount: 2,
      maxBufferLength: 6,
      backBufferLength: 12,
      manifestLoadingTimeOut: 10_000,
      fragLoadingTimeOut: 15_000,
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

  it("destroys the instance on fatal errors", () => {
    const controller = {
      destroy: vi.fn(),
      recoverMediaError: vi.fn(),
      startLoad: vi.fn(),
    };

    handleIptvHlsError(
      controller,
      { type: Hls.ErrorTypes.NETWORK_ERROR, fatal: true } as ErrorData,
      { mediaRecoveryAttempts: 0 },
    );

    expect(controller.destroy).toHaveBeenCalledTimes(1);
    expect(controller.startLoad).not.toHaveBeenCalled();
    expect(controller.recoverMediaError).not.toHaveBeenCalled();
  });

  it("snaps live playback closer to the live sync position when drift is large", () => {
    const video = {
      currentTime: 100,
    } as unknown as HTMLVideoElement;

    syncLivePlaybackPosition(video, { liveSyncPosition: 106 });

    expect(video.currentTime).toBeCloseTo(105.5);
  });

  it("increases playback rate slightly when live playback drifts behind", () => {
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

    expect(video.playbackRate).toBe(1.05);
  });
});
