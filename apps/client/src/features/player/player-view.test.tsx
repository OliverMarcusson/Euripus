import type { ReactElement } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { Mock } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { PlaybackSource } from "@euripus/shared";
import { describe, expect, it, beforeEach, afterEach, vi } from "vitest";
import { PlayerView } from "@/features/player/player-view";
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
    source: PlaybackSource,
    uiMode: "local" | "receiver";
    videoClassName?: string;
  }) => ReactElement
>;

vi.mock("@/components/player/plyr-surface", () => ({
  PlyrSurface: (props: {
    ariaLabel: string;
    className?: string;
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
    usePlayerStore.setState({ currentRequest: null, source: null, loading: false });
    useRemoteControllerStore.setState({ target: null, selectedAt: null });
  });

  afterEach(() => {
    cleanup();
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
        onRecoveryNeeded: expect.any(Function),
        source: SOURCE,
        uiMode: "local",
      }),
    );
    expect(screen.getByLabelText("Playing Arena Live")).toHaveClass(
      "euripus-plyr-media",
    );
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
