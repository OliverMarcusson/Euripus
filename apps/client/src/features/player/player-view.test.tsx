import type { ReactElement } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { Mock } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { PlaybackSource } from "@euripus/shared";
import type { PlaybackFailure } from "@/lib/hls";
import { describe, expect, it, beforeEach, afterEach, vi } from "vitest";
import { PlayerView } from "@/features/player/player-view";
import { startChannelPlayback } from "@/lib/api";
import { usePlayerStore } from "@/store/player-store";
import { useRemoteControllerStore } from "@/store/remote-controller-store";

const plyrSurface = vi.fn(({
  ariaLabel,
  className,
  videoClassName,
}: {
  ariaLabel: string;
  className?: string;
  videoClassName?: string;
}) => (
  <div className={className} data-testid="plyr-surface">
    <video aria-label={ariaLabel} className={videoClassName} />
  </div>
)) as Mock<
  (props: {
    ariaLabel: string;
    className?: string;
    onPlaybackFailure?: (failure: PlaybackFailure) => void | Promise<void>;
    onPlaybackHealthy?: () => void;
    source: PlaybackSource,
    uiMode: "local" | "receiver";
    videoClassName?: string;
  }) => ReactElement
>;

vi.mock("@/components/player/plyr-surface", () => ({
  PlyrSurface: (props: {
    ariaLabel: string;
    className?: string;
    onPlaybackFailure?: (failure: PlaybackFailure) => void | Promise<void>;
    onPlaybackHealthy?: () => void;
    source: PlaybackSource;
    uiMode: "local" | "receiver";
    videoClassName?: string;
  }) => plyrSurface(props),
}));

vi.mock("@/lib/api", () => ({
  getRemoteControllerTarget: vi.fn().mockResolvedValue(null),
  startChannelPlayback: vi.fn(),
  startProgramPlayback: vi.fn(),
  pauseRemotePlayback: vi.fn(),
  resumeRemotePlayback: vi.fn(),
  stopRemotePlayback: vi.fn(),
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

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function renderPlayerView() {
  return render(
    <QueryClientProvider client={new QueryClient()}>
      <PlayerView />
    </QueryClientProvider>,
  );
}

describe("PlayerView", () => {
  beforeEach(() => {
    plyrSurface.mockClear();
    vi.mocked(startChannelPlayback).mockReset();
    usePlayerStore.setState({ currentRequest: null, source: null, loading: false });
    useRemoteControllerStore.setState({ target: null, selectedAt: null });
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("renders the empty state when no source is selected", () => {
    renderPlayerView();

    expect(screen.getByText("Choose a channel or program")).toBeInTheDocument();
    expect(plyrSurface).not.toHaveBeenCalled();
  });

  it("renders unsupported playback without creating a Plyr session", () => {
    usePlayerStore.setState({
      currentRequest: { kind: "channel", id: "channel-1" },
      source: {
        ...SOURCE,
        kind: "unsupported",
        unsupportedReason: "Unsupported in browser.",
      },
    });

    renderPlayerView();

    expect(screen.getByText("Unsupported in browser.")).toBeInTheDocument();
    expect(plyrSurface).not.toHaveBeenCalled();
  });

  it("creates a Plyr session for playable sources", () => {
    usePlayerStore.setState({
      currentRequest: { kind: "channel", id: "channel-1" },
      source: SOURCE,
    });

    renderPlayerView();

    expect(plyrSurface).toHaveBeenCalledTimes(1);
    expect(plyrSurface).toHaveBeenCalledWith(
      expect.objectContaining({
        ariaLabel: "Playing Arena Live",
        onPlaybackFailure: expect.any(Function),
        onPlaybackHealthy: expect.any(Function),
        source: SOURCE,
        uiMode: "local",
      }),
    );
    expect(screen.getByLabelText("Playing Arena Live")).toHaveClass(
      "euripus-plyr-media",
    );
  });

  it("shows provider placeholders as terminal failures without retrying", () => {
    usePlayerStore.setState({
      currentRequest: { kind: "channel", id: "channel-1" },
      source: SOURCE,
    });
    renderPlayerView();

    act(() => {
      plyrSurface.mock.lastCall?.[0].onPlaybackFailure?.({
        kind: "provider-unavailable",
        message: "This channel is currently unavailable from the provider.",
      });
    });

    expect(
      screen.getByText("This channel is currently unavailable from the provider."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
    expect(startChannelPlayback).not.toHaveBeenCalled();
  });

  it("deduplicates recovery requests and resets the budget after healthy playback", async () => {
    vi.useFakeTimers();
    vi.mocked(startChannelPlayback).mockResolvedValue(SOURCE);
    usePlayerStore.setState({
      currentRequest: { kind: "channel", id: "channel-1" },
      source: SOURCE,
    });
    renderPlayerView();

    act(() => {
      const reportFailure = plyrSurface.mock.lastCall?.[0].onPlaybackFailure;
      reportFailure?.({ kind: "recoverable", reason: "hls" });
      reportFailure?.({ kind: "recoverable", reason: "video-error" });
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1_000);
    });
    expect(startChannelPlayback).toHaveBeenCalledTimes(1);

    act(() => {
      plyrSurface.mock.lastCall?.[0].onPlaybackHealthy?.();
      plyrSurface.mock.lastCall?.[0].onPlaybackFailure?.({
        kind: "recoverable",
        reason: "hls",
      });
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1_000);
    });
    expect(startChannelPlayback).toHaveBeenCalledTimes(2);
  });

  it("does not let an older recovery clear a newer request's in-flight recovery", async () => {
    vi.useFakeTimers();
    const firstRecovery = deferred<PlaybackSource>();
    const secondRecovery = deferred<PlaybackSource>();
    vi.mocked(startChannelPlayback)
      .mockReturnValueOnce(firstRecovery.promise)
      .mockReturnValueOnce(secondRecovery.promise);
    usePlayerStore.setState({
      currentRequest: { kind: "channel", id: "channel-1" },
      source: SOURCE,
    });
    renderPlayerView();

    act(() => {
      plyrSurface.mock.lastCall?.[0].onPlaybackFailure?.({
        kind: "recoverable",
        reason: "hls",
      });
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1_000);
    });

    const secondSource = { ...SOURCE, title: "Arena Two" };
    act(() => {
      usePlayerStore.setState({
        currentRequest: { kind: "channel", id: "channel-2" },
        source: secondSource,
      });
    });
    act(() => {
      plyrSurface.mock.lastCall?.[0].onPlaybackFailure?.({
        kind: "recoverable",
        reason: "hls",
      });
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1_000);
    });
    expect(startChannelPlayback).toHaveBeenCalledTimes(2);

    await act(async () => {
      firstRecovery.resolve(SOURCE);
      await firstRecovery.promise;
    });
    act(() => {
      plyrSurface.mock.lastCall?.[0].onPlaybackFailure?.({
        kind: "recoverable",
        reason: "video-error",
      });
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3_000);
    });

    expect(startChannelPlayback).toHaveBeenCalledTimes(2);
    await act(async () => {
      secondRecovery.resolve(secondSource);
      await secondRecovery.promise;
    });
  });

  it("caps automatic recovery and allows a manual retry", async () => {
    vi.useFakeTimers();
    vi.mocked(startChannelPlayback).mockResolvedValue(SOURCE);
    usePlayerStore.setState({
      currentRequest: { kind: "channel", id: "channel-1" },
      source: SOURCE,
    });
    renderPlayerView();

    for (const delay of [1_000, 3_000, 10_000]) {
      act(() => {
        plyrSurface.mock.lastCall?.[0].onPlaybackFailure?.({
          kind: "recoverable",
          reason: "hls",
        });
      });
      await act(async () => {
        await vi.advanceTimersByTimeAsync(delay);
      });
    }

    act(() => {
      plyrSurface.mock.lastCall?.[0].onPlaybackFailure?.({
        kind: "recoverable",
        reason: "hls",
      });
    });

    expect(startChannelPlayback).toHaveBeenCalledTimes(3);
    expect(
      screen.getByText("Playback failed after multiple attempts."),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(startChannelPlayback).toHaveBeenCalledTimes(4);
  });

  it("destroys the playback session when minimized", () => {
    usePlayerStore.setState({
      currentRequest: { kind: "channel", id: "channel-1" },
      source: SOURCE,
    });

    renderPlayerView();

    fireEvent.click(screen.getAllByRole("button")[0]);

    expect(plyrSurface).toHaveBeenCalledTimes(1);
    expect(screen.getByText("Arena Live")).toBeInTheDocument();
  });
});
