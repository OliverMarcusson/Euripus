import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { SearchPage } from "@/features/search/search-page";
import {
  getSearchFilterOptions,
  searchChannels,
  searchPrograms,
  startRemoteChannelPlayback,
} from "@/lib/api";
import { useRemoteControllerStore } from "@/store/remote-controller-store";

vi.mock("@/hooks/use-debounce", () => ({
  useDebounce: (value: string) => value,
}));

vi.mock("@/lib/api", () => ({
  addFavorite: vi.fn(),
  getSearchFilterOptions: vi.fn(),
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
const mockedGetSearchFilterOptions = vi.mocked(getSearchFilterOptions);
const mockedStartRemoteChannelPlayback = vi.mocked(startRemoteChannelPlayback);

describe("SearchPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(Date, "now").mockReturnValue(new Date("2026-04-04T12:00:00.000Z").getTime());
    useRemoteControllerStore.getState().clearTarget();
    mockedGetSearchFilterOptions.mockResolvedValue({
      countries: ["se", "us"],
      providers: [
        { value: "tv4play", countryCodes: ["se"] },
        { value: "viaplay", countryCodes: ["se"] },
        { value: "sky", countryCodes: ["uk"] },
      ],
    });
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
      backend: "meilisearch",
      items: [],
      totalCount: 0,
      nextOffset: null,
    });
    mockedSearchPrograms.mockResolvedValue({
      query: "hammarby",
      backend: "meilisearch",
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
      backend: "meilisearch",
      items: [
        {
          id: "channel-1",
          name: "Arena 1",
          logoUrl: null,
          categoryName: "Sports",
          remoteStreamId: 1,
          epgChannelId: "arena-1",
          hasEpg: true,
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
      backend: "postgres",
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
    expect(screen.getByText("EPG")).toBeInTheDocument();
  });

  it("does not render the EPG badge when a channel only has an epg mapping id", async () => {
    mockedSearchChannels.mockResolvedValue({
      query: "film",
      backend: "postgres",
      items: [
        {
          id: "channel-film",
          name: "TV4 Film",
          logoUrl: null,
          categoryName: "Movies",
          remoteStreamId: 2,
          epgChannelId: "tv4-film",
          hasEpg: false,
          hasCatchup: false,
          archiveDurationHours: null,
          streamExtension: "m3u8",
          isFavorite: false,
        },
      ],
      totalCount: 1,
      nextOffset: null,
    });
    mockedSearchPrograms.mockResolvedValue({
      query: "film",
      backend: "postgres",
      items: [],
      totalCount: 0,
      nextOffset: null,
    });

    renderSearchPage();
    fireEvent.change(screen.getByPlaceholderText(/^search$/i), {
      target: { value: "film" },
    });

    await waitFor(() =>
      expect(mockedSearchChannels).toHaveBeenCalledWith("film", 0, 30),
    );
    expect(await screen.findByText("TV4 Film")).toBeInTheDocument();
    expect(screen.queryByText("EPG")).not.toBeInTheDocument();
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
        playbackStateStale: false,
      },
      selectedAt: "2026-04-05T10:00:00.000Z",
    });
    mockedSearchChannels.mockResolvedValue({
      query: "arena",
      backend: "meilisearch",
      items: [
        {
          id: "channel-1",
          name: "Arena 1",
          logoUrl: null,
          categoryName: "Sports",
          remoteStreamId: 1,
          epgChannelId: null,
          hasEpg: false,
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
      backend: "meilisearch",
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

  it("shows which backend answered each search section", async () => {
    mockedSearchChannels.mockResolvedValue({
      query: "arena",
      backend: "meilisearch",
      items: [],
      totalCount: 0,
      nextOffset: null,
    });
    mockedSearchPrograms.mockResolvedValue({
      query: "arena",
      backend: "postgres",
      items: [],
      totalCount: 0,
      nextOffset: null,
    });

    renderSearchPage();
    fireEvent.change(screen.getByPlaceholderText(/^search$/i), {
      target: { value: "arena" },
    });

    expect(await screen.findByText("Meilisearch")).toBeInTheDocument();
    expect(await screen.findByText("Postgres")).toBeInTheDocument();
  });

  it("shows a lightweight search guide and applies filter suggestions", async () => {
    mockedSearchChannels.mockResolvedValue({
      query: "golf country:se",
      backend: "meilisearch",
      items: [],
      totalCount: 0,
      nextOffset: null,
    });
    mockedSearchPrograms.mockResolvedValue({
      query: "golf country:se",
      backend: "meilisearch",
      items: [],
      totalCount: 0,
      nextOffset: null,
    });

    renderSearchPage();
    fireEvent.change(screen.getByPlaceholderText(/^search$/i), {
      target: { value: "golf country:se" },
    });

    expect(await screen.findByText(/search guide:/i)).toBeInTheDocument();
    expect(screen.getByText(/searching for "golf"/i)).toBeInTheDocument();
    expect(screen.getByText(/country se/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "epg" }));

    await waitFor(() =>
      expect(mockedSearchChannels).toHaveBeenCalledWith("golf country:se epg", 0, 30),
    );
  });

  it("shows country autocomplete options and inserts the selected token", async () => {
    mockedSearchChannels.mockResolvedValue({
      query: "country:se ",
      backend: "meilisearch",
      items: [],
      totalCount: 0,
      nextOffset: null,
    });
    mockedSearchPrograms.mockResolvedValue({
      query: "country:se ",
      backend: "meilisearch",
      items: [],
      totalCount: 0,
      nextOffset: null,
    });

    renderSearchPage();
    const input = screen.getByPlaceholderText(/^search$/i) as HTMLInputElement;
    fireEvent.focus(input);
    fireEvent.change(input, {
      target: { value: "country:" },
    });
    input.setSelectionRange(input.value.length, input.value.length);
    fireEvent.keyUp(input);

    expect(await screen.findByText("Countries")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("option", { name: /country:se/i }));

    await waitFor(() =>
      expect(mockedSearchChannels).toHaveBeenCalledWith("country:se ", 0, 30),
    );
    expect(input.value).toBe("country:se ");
  });

  it("supports provider autocomplete from the keyboard, scoped to the chosen country", async () => {
    mockedSearchChannels.mockResolvedValue({
      query: "golf country:se provider:viaplay ",
      backend: "meilisearch",
      items: [],
      totalCount: 0,
      nextOffset: null,
    });
    mockedSearchPrograms.mockResolvedValue({
      query: "golf country:se provider:viaplay ",
      backend: "meilisearch",
      items: [],
      totalCount: 0,
      nextOffset: null,
    });

    renderSearchPage();
    const input = screen.getByPlaceholderText(/^search$/i) as HTMLInputElement;
    fireEvent.focus(input);
    fireEvent.change(input, {
      target: { value: "ppv" },
    });
    input.setSelectionRange(input.value.length, input.value.length);
    fireEvent.keyUp(input);
    expect(screen.queryByText("Countries")).not.toBeInTheDocument();
    expect(screen.queryByText("Providers")).not.toBeInTheDocument();

    fireEvent.change(input, {
      target: { value: "golf country:se provider:via" },
    });
    input.setSelectionRange(input.value.length, input.value.length);
    fireEvent.keyUp(input);

    expect(await screen.findByText("Providers")).toBeInTheDocument();
    expect(screen.getByRole("option", { name: /provider:viaplay/i })).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /provider:sky/i })).not.toBeInTheDocument();
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() =>
      expect(mockedSearchChannels).toHaveBeenCalledWith(
        "golf country:se provider:viaplay ",
        0,
        30,
      ),
    );
    expect(input.value).toBe("golf country:se provider:viaplay ");
  });
});
