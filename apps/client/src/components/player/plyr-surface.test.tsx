import { render } from "@testing-library/react";
import type { PlaybackSource } from "@euripus/shared";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PlyrSurface } from "@/components/player/plyr-surface";
import { bindPlaybackSource } from "@/lib/plyr-player";

const { destroy, attachPlaybackSeekDebugging } = vi.hoisted(() => ({
  destroy: vi.fn(),
  attachPlaybackSeekDebugging: vi.fn(() => vi.fn()),
}));

vi.mock("@/lib/plyr-player", () => ({
  bindPlaybackSource: vi.fn(() => ({
    plyr: null,
    destroy,
  })),
}));

vi.mock("@/lib/playback-diagnostics", () => ({
  attachPlaybackSeekDebugging,
}));

const SOURCE: PlaybackSource = {
  kind: "hls",
  url: "https://example.com/live.m3u8",
  headers: {},
  live: true,
  catchup: false,
  expiresAt: null,
  unsupportedReason: null,
  title: "Arena Live",
};

describe("PlyrSurface", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => {});
    vi.spyOn(HTMLMediaElement.prototype, "play").mockResolvedValue();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("does not recreate the playback binding when the recovery callback changes", () => {
    const firstRecovery = vi.fn();
    const secondRecovery = vi.fn();
    const { rerender } = render(
      <PlyrSurface
        ariaLabel="Playing Arena Live"
        onRecoveryNeeded={firstRecovery}
        source={SOURCE}
        uiMode="local"
      />,
    );

    rerender(
      <PlyrSurface
        ariaLabel="Playing Arena Live"
        onRecoveryNeeded={secondRecovery}
        source={SOURCE}
        uiMode="local"
      />,
    );

    expect(bindPlaybackSource).toHaveBeenCalledTimes(1);
    expect(destroy).not.toHaveBeenCalled();
  });
});
