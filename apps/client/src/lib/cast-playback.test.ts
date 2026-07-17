import {
  startChannelPlayback,
  startEpisodePlayback,
  startOnDemandPlayback,
  startProgramPlayback,
} from "@/lib/api";
import { castPlaybackRequest } from "@/lib/cast-playback";
import { loadGoogleCastMedia } from "@/lib/google-cast";

vi.mock("@/lib/api", () => ({
  startChannelPlayback: vi.fn(),
  startEpisodePlayback: vi.fn(),
  startOnDemandPlayback: vi.fn(),
  startProgramPlayback: vi.fn(),
}));

vi.mock("@/lib/google-cast", () => ({
  loadGoogleCastMedia: vi.fn(),
}));

const source = {
  kind: "hls" as const,
  url: "https://euripus.example/api/relay/hls?token=signed",
  headers: {},
  live: true,
  catchup: false,
  expiresAt: null,
  unsupportedReason: null,
  title: "Arena 1",
};

describe("castPlaybackRequest", () => {
  beforeEach(() => {
    vi.mocked(startChannelPlayback).mockResolvedValue(source);
    vi.mocked(startProgramPlayback).mockResolvedValue(source);
    vi.mocked(startOnDemandPlayback).mockResolvedValue(source);
    vi.mocked(startEpisodePlayback).mockResolvedValue(source);
  });

  it.each([
    ["channel", startChannelPlayback],
    ["program", startProgramPlayback],
    ["onDemand", startOnDemandPlayback],
    ["episode", startEpisodePlayback],
  ] as const)("requests a relay-backed cast source for %s playback", async (kind, resolver) => {
    await castPlaybackRequest({ kind, id: "media-1" });

    expect(resolver).toHaveBeenCalledWith("media-1", "cast");
    expect(loadGoogleCastMedia).toHaveBeenCalledWith(source);
  });
});
