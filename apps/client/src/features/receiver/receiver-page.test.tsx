import type { MutableRefObject } from "react";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { PlaybackSource, ReceiverSession } from "@euripus/shared";
import {
  acknowledgeReceiverCommand,
  startReceiverCastTranscode,
  stopReceiverCastTranscode,
  updateReceiverPlaybackState,
} from "@/lib/api";
import { ReceiverPage } from "@/features/receiver/receiver-page";
import { useReceiverSession } from "@/features/receiver/use-receiver-session";

const { completePairing, renewSession } = vi.hoisted(() => ({
  completePairing: vi.fn(),
  renewSession: vi.fn(),
}));

vi.mock("@/features/receiver/use-receiver-session", () => ({
  useReceiverSession: vi.fn(),
}));

vi.mock("@/lib/google-cast-receiver", () => ({
  initializeGoogleCastReceiver: vi.fn().mockResolvedValue(true),
  isGoogleCastReceiver: vi.fn(() => true),
  publishGoogleCastReceiverStatus: vi.fn(),
}));

vi.mock("@/lib/api", () => ({
  API_BASE_URL: "/api",
  acknowledgeReceiverCommand: vi.fn().mockResolvedValue(undefined),
  startReceiverCastTranscode: vi.fn(),
  stopReceiverCastTranscode: vi.fn().mockResolvedValue(undefined),
  updateReceiverPlaybackState: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@/components/player/plyr-surface", () => ({
  PlyrSurface: ({
    onPlaybackFailure,
    source,
    videoRef,
  }: {
    onPlaybackFailure: (failure: {
      kind: "recoverable";
      reason: "codec";
    }) => void | Promise<void>;
    source: PlaybackSource;
    videoRef: MutableRefObject<HTMLVideoElement | null>;
  }) => (
    <div data-testid="receiver-player" data-source-url={source.url}>
      <video ref={videoRef} />
      <button
        onClick={() => {
          void onPlaybackFailure({ kind: "recoverable", reason: "codec" });
        }}
        type="button"
      >
        Simulate codec failure
      </button>
    </div>
  ),
}));

type EventListener = (event: MessageEvent<string> | Event) => void;

class EventSourceMock {
  static instances: EventSourceMock[] = [];
  listeners = new Map<string, EventListener[]>();

  constructor() {
    EventSourceMock.instances.push(this);
  }

  addEventListener(type: string, listener: EventListener) {
    const listeners = this.listeners.get(type) ?? [];
    listeners.push(listener);
    this.listeners.set(type, listeners);
  }

  close() {}

  emit(type: string, payload?: unknown) {
    const event =
      payload === undefined
        ? new Event(type)
        : new MessageEvent(type, { data: JSON.stringify(payload) });
    this.listeners.get(type)?.forEach((listener) => listener(event));
  }
}

const SESSION: ReceiverSession = {
  sessionToken: "session-1",
  expiresAt: "2026-08-02T00:00:00.000Z",
  receiverCredential: "credential-1",
  pairingCode: null,
  paired: true,
  device: {
    id: "device-1",
    name: "Living room",
    platform: "google-cast",
    formFactorHint: "tv",
    appKind: "receiver-google-cast",
    remembered: true,
    online: true,
    currentController: true,
    lastSeenAt: "2026-08-01T00:00:00.000Z",
    updatedAt: "2026-08-01T00:00:00.000Z",
    currentPlayback: null,
    playbackStateStale: false,
  },
};

const SOURCE: PlaybackSource = {
  kind: "hls",
  url: "https://example.com/original.m3u8",
  headers: {},
  live: true,
  catchup: false,
  expiresAt: null,
  unsupportedReason: null,
  title: "Arena Live",
};

function playbackCommand(source: PlaybackSource) {
  return {
    command: {
      id: "command-1",
      targetDeviceId: "device-1",
      targetDeviceName: "Living room",
      commandType: "play",
      status: "delivered",
      sourceTitle: source.title,
      createdAt: "2026-08-01T00:00:00.000Z",
    },
    source,
    positionSeconds: null,
    receiverCredential: null,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

describe("ReceiverPage", () => {
  beforeEach(() => {
    EventSourceMock.instances = [];
    Object.defineProperty(globalThis, "EventSource", {
      configurable: true,
      value: EventSourceMock,
    });
    vi.mocked(useReceiverSession).mockReturnValue({
      completePairing,
      error: null,
      pairingCode: null,
      renewSession,
      session: SESSION,
    });
    vi.mocked(acknowledgeReceiverCommand).mockClear();
    vi.mocked(startReceiverCastTranscode).mockReset();
    vi.mocked(stopReceiverCastTranscode).mockClear();
    vi.mocked(updateReceiverPlaybackState).mockClear();
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("keeps the preparation overlay visible until the transcoded video can play", async () => {
    const transcode = deferred<PlaybackSource>();
    const transcodedSource = {
      ...SOURCE,
      url: "https://example.com/api/transcode/session/index.m3u8",
    };
    vi.mocked(startReceiverCastTranscode).mockReturnValue(transcode.promise);
    render(<ReceiverPage />);

    act(() => {
      EventSourceMock.instances[0]?.emit(
        "playback_command",
        playbackCommand(SOURCE),
      );
    });
    expect(screen.getByText("Loading stream")).toBeVisible();
    expect(screen.getByTestId("receiver-player")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Simulate codec failure" }));

    expect(screen.getByText("Preparing a compatible stream")).toBeVisible();
    expect(screen.getByTestId("receiver-player")).toBeInTheDocument();

    await act(async () => {
      transcode.resolve(transcodedSource);
      await transcode.promise;
    });

    expect(screen.getByText("Preparing a compatible stream")).toBeVisible();
    expect(screen.getByTestId("receiver-player")).toHaveAttribute(
      "data-source-url",
      transcodedSource.url,
    );

    const video = screen.getByTestId("receiver-player").querySelector("video")!;
    fireEvent.canPlay(video);
    expect(screen.getByText("Preparing a compatible stream")).toBeVisible();

    fireEvent.playing(video);

    expect(
      screen.queryByText("Preparing a compatible stream"),
    ).not.toBeInTheDocument();
    expect(screen.getByTestId("receiver-player")).toBeInTheDocument();
  });

  it("shows unsupported streams in the receiver layout", () => {
    render(<ReceiverPage />);
    act(() => {
      EventSourceMock.instances[0]?.emit(
        "playback_command",
        playbackCommand({
          ...SOURCE,
          kind: "unsupported",
          unsupportedReason: "This provider format is unavailable.",
        }),
      );
    });

    expect(screen.getByRole("alert")).toHaveClass("euripus-receiver__panel");
    expect(screen.getByText("Playback unavailable")).toBeVisible();
    expect(
      screen.getByText("This provider format is unavailable."),
    ).toHaveClass("euripus-receiver__copy");
    expect(screen.getByRole("alert").closest(".euripus-receiver")).toHaveAttribute(
      "data-tone",
      "error",
    );
  });
});
