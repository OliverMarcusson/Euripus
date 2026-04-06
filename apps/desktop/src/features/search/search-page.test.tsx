import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { SearchPage } from "@/features/search/search-page";
import { searchChannels, searchPrograms, startRemoteChannelPlayback } from "@/lib/api";
import { useRemoteControllerStore } from "@/store/remote-controller-store";

vi.mock("@/hooks/use-debounce", () => ({
  useDebounce: (value: string) => value,
}));

vi.mock("@/lib/api", () => ({
  addFavorite: vi.fn(),
  removeFavorite: vi.fn(),
  searchChannels: vi.fn(),
  searchPrograms: vi.fn(),
  startChannelPlayback: vi.fn(),
  startProgramPlayback: vi.fn(),
  startRemoteChannelPlayback: vi.fn(),
  startRemoteProgramPlayback: vi.fn(),
}));

const mockedSearchChannels = vi.mocked(searchChannels);
const mockedSearchPrograms = vi.mocked(searchPrograms);
const mockedStartRemoteChannelPlayback = vi.mocked(startRemoteChannelPlayback);

describe("SearchPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(Date, "now").mockReturnValue(new Date("2026-04-04T12:00:00.000Z").getTime());
    useRemoteControllerStore.getState().clearTarget();
    mockedStartRemoteChannelPlayback.mockResolvedValue({
      id: "remote-command-1",
      targetDeviceId: "tv-1",
      targetDeviceName: "Living room TV",
      commandType: "playback_source",
      status: "delivered",
      sourceTitle: "Arena 1",
      createdAt: "2026-04-05T10:00:00.000Z",
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  function renderSearchPage() {
    const queryClient = new QueryClient({
      defaultOptions: {
        queries: {
          retry: false,
        },
      },
    });

    return render(
      <QueryClientProvider client={queryClient}>
        <SearchPage />
      </QueryClientProvider>,
    );
  }

  it("renders EPG program states and play buttons only for playable results", async () => {
    mockedSearchChannels.mockResolvedValue({
      query: "hammarby",
      items: [],
      totalCount: 0,
      nextOffset: null,
    });
    mockedSearchPrograms.mockResolvedValue({
      query: "hammarby",
      items: [
        {
          id: "live-program",
          channelId: "channel-live",
          channelName: "Arena 1",
          title: "Live derby",
          description: null,
          startAt: "2026-04-04T11:30:00.000Z",
          endAt: "2026-04-04T12:30:00.000Z",
          canCatchup: false,
        },
        {
          id: "catchup-program",
          channelId: "channel-archive",
          channelName: "Arena 2",
          title: "Morning recap",
          description: null,
          startAt: "2026-04-04T09:00:00.000Z",
          endAt: "2026-04-04T10:00:00.000Z",
          canCatchup: true,
        },
        {
          id: "upcoming-program",
          channelId: "channel-upcoming",
          channelName: "Arena 3",
          title: "Evening match",
          description: null,
          startAt: "2026-04-04T13:00:00.000Z",
          endAt: "2026-04-04T14:00:00.000Z",
          canCatchup: false,
        },
        {
          id: "info-program",
          channelId: "channel-info",
          channelName: "Arena 4",
          title: "Expired listing",
          description: null,
          startAt: "2026-04-04T08:00:00.000Z",
          endAt: "2026-04-04T09:00:00.000Z",
          canCatchup: false,
        },
      ],
      totalCount: 4,
      nextOffset: null,
    });

    renderSearchPage();
    fireEvent.change(screen.getByPlaceholderText(/^search$/i), {
      target: { value: "hammarby" },
    });

    await waitFor(() => expect(mockedSearchPrograms).toHaveBeenCalledWith("hammarby", 0, 30));
    expect(await screen.findByText("Live now")).toBeInTheDocument();
    expect(screen.getByText("Catch-up")).toBeInTheDocument();
    expect(screen.getByText("Upcoming")).toBeInTheDocument();
    expect(screen.getAllByText("Info only")).toHaveLength(2);
    expect(screen.getAllByRole("button", { name: /^play$/i })).toHaveLength(2);
    expect(screen.getByText("Upcoming only")).toBeInTheDocument();
  });

  it("renders favorite controls for channel matches", async () => {
    mockedSearchChannels.mockResolvedValue({
      query: "arena",
      items: [
        {
          id: "channel-1",
          name: "Arena 1",
          logoUrl: null,
          categoryName: "Sports",
          remoteStreamId: 1,
          epgChannelId: null,
          hasCatchup: true,
          archiveDurationHours: 24,
          streamExtension: "m3u8",
          isFavorite: false,
        },
      ],
      totalCount: 1,
      nextOffset: null,
    });
    mockedSearchPrograms.mockResolvedValue({
      query: "arena",
      items: [],
      totalCount: 0,
      nextOffset: null,
    });

    renderSearchPage();
    fireEvent.change(screen.getByPlaceholderText(/^search$/i), {
      target: { value: "arena" },
    });

    await waitFor(() => expect(mockedSearchChannels).toHaveBeenCalledWith("arena", 0, 30));
    expect(await screen.findByRole("button", { name: /favorite/i })).toBeInTheDocument();
    expect(screen.getByText("Channel matches")).toBeInTheDocument();
  });

  it("redirects play actions to the selected remote target", async () => {
    useRemoteControllerStore.getState().setTargetSelection({
      device: {
        id: "tv-1",
        name: "Living room TV",
        platform: "web",
        formFactorHint: "large-screen",
        appKind: "receiver-web",
        remembered: true,
        online: true,
        currentController: true,
        lastSeenAt: "2026-04-05T10:00:00.000Z",
        updatedAt: "2026-04-05T10:00:00.000Z",
        currentPlayback: null,
      },
      selectedAt: "2026-04-05T10:00:00.000Z",
    });
    mockedSearchChannels.mockResolvedValue({
      query: "arena",
      items: [
        {
          id: "channel-1",
          name: "Arena 1",
          logoUrl: null,
          categoryName: "Sports",
          remoteStreamId: 1,
          epgChannelId: null,
          hasCatchup: true,
          archiveDurationHours: 24,
          streamExtension: "m3u8",
          isFavorite: false,
        },
      ],
      totalCount: 1,
      nextOffset: null,
    });
    mockedSearchPrograms.mockResolvedValue({
      query: "arena",
      items: [],
      totalCount: 0,
      nextOffset: null,
    });

    renderSearchPage();
    fireEvent.change(screen.getByPlaceholderText(/^search$/i), {
      target: { value: "arena" },
    });

    fireEvent.click(await screen.findByRole("button", { name: /^play$/i }));

    await waitFor(() =>
      expect(mockedStartRemoteChannelPlayback).toHaveBeenCalledWith("channel-1"),
    );
  });
});
