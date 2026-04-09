import { describe, expect, it } from "vitest";
import {
  formatReceiverPlaybackSummary,
  receiverPlaybackBadgeLabel,
} from "@/lib/receiver-playback";

describe("receiver playback helpers", () => {
  it("prioritizes playback errors in summaries", () => {
    expect(
      formatReceiverPlaybackSummary({
        currentPlayback: {
          title: "Arena 1",
          sourceKind: "hls",
          live: true,
          catchup: false,
          updatedAt: "2026-04-09T12:00:00.000Z",
          paused: false,
          buffering: false,
          positionSeconds: null,
          durationSeconds: null,
          errorMessage: "The receiver could not decode this stream.",
        },
        online: true,
        platform: "android-tv",
        lastSeenAt: "2026-04-09T12:00:00.000Z",
        playbackStateStale: false,
      }),
    ).toContain("Playback issue");
  });

  it("describes buffering receivers", () => {
    expect(
      receiverPlaybackBadgeLabel({
        title: "Arena 1",
        sourceKind: "hls",
        live: true,
        catchup: false,
        updatedAt: "2026-04-09T12:00:00.000Z",
        paused: false,
        buffering: true,
        positionSeconds: null,
        durationSeconds: null,
        errorMessage: null,
      }),
    ).toBe("Buffering");
  });
});
