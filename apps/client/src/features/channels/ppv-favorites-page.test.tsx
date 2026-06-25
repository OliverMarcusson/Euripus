import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { PpvFavoritesPage } from "@/features/channels/ppv-favorites-page";
import {
  addPpvFavorite,
  getPpvFavorites,
  searchAiPpv,
  startChannelPlayback,
} from "@/lib/api";
import { formatTimeRange } from "@/lib/utils";

vi.mock("@tanstack/react-router", async () => {
  const actual = await vi.importActual<typeof import("@tanstack/react-router")>("@tanstack/react-router");
  return {
    ...actual,
    useNavigate: () => vi.fn(),
  };
});

vi.mock("@/lib/api", () => ({
  getPpvFavorites: vi.fn(),
  addPpvFavorite: vi.fn(),
  removePpvFavorite: vi.fn(),
  reorderPpvFavorites: vi.fn(),
  searchAiPpv: vi.fn(),
  startChannelPlayback: vi.fn(),
  startRemoteChannelPlayback: vi.fn(),
}));

const mockedGetPpvFavorites = vi.mocked(getPpvFavorites);
const mockedSearchAiPpv = vi.mocked(searchAiPpv);
const mockedStartChannelPlayback = vi.mocked(startChannelPlayback);
const mockedAddPpvFavorite = vi.mocked(addPpvFavorite);

describe("PpvFavoritesPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(Date, "now").mockReturnValue(
      new Date("2026-04-04T12:00:00.000Z").getTime(),
    );
    mockedSearchAiPpv.mockResolvedValue({
      query: "sweden japan",
      backend: "local_fallback",
      items: [],
      message: null,
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  function renderPpvFavoritesPage() {
    const queryClient = new QueryClient({
      defaultOptions: {
        queries: {
          retry: false,
        },
      },
    });

    return render(
      <QueryClientProvider client={queryClient}>
        <PpvFavoritesPage />
      </QueryClientProvider>,
    );
  }

  it("shows an explanatory empty state", async () => {
    mockedGetPpvFavorites.mockResolvedValue([]);

    renderPpvFavoritesPage();

    expect(await screen.findByText("No PPV favorites yet")).toBeInTheDocument();
    expect(
      screen.getByText(/Save temporary PPV event channels here/i),
    ).toBeInTheDocument();
  });

  it("renders ppv favorites with program metadata", async () => {
    mockedGetPpvFavorites.mockResolvedValue([
      {
        kind: "channel",
        order: 0,
        channel: {
          id: "ppv-channel-1",
          name: "LIVE | Main Event | SE: VIAPLAY PPV 5",
          logoUrl: null,
          categoryName: "Sports",
          remoteStreamId: 5,
          epgChannelId: "ppv-5",
          hasEpg: true,
          hasCatchup: true,
          archiveDurationHours: 24,
          streamExtension: "m3u8",
          isFavorite: false,
          isPpv: true,
          isPpvFavorite: true,
        },
        program: {
          id: "program-1",
          channelId: "ppv-channel-1",
          channelName: "LIVE | Main Event | SE: VIAPLAY PPV 5",
          title: "Championship Fight",
          description: "Main card coverage",
          startAt: "2026-04-04T11:30:00.000Z",
          endAt: "2026-04-04T12:30:00.000Z",
          canCatchup: true,
        },
      },
    ]);

    renderPpvFavoritesPage();

    expect(await screen.findByText("PPV")).toBeInTheDocument();
    expect(screen.getByText("Championship Fight")).toBeInTheDocument();
    expect(screen.getByText("Main card coverage")).toBeInTheDocument();
    expect(screen.getAllByText("Live now").length).toBeGreaterThan(0);
    expect(
      screen.getByText(
        formatTimeRange(
          "2026-04-04T11:30:00.000Z",
          "2026-04-04T12:30:00.000Z",
        ),
      ),
    ).toBeInTheDocument();
  });

  it("searches AI PPV matches and renders confidence metadata", async () => {
    mockedGetPpvFavorites.mockResolvedValue([]);
    mockedSearchAiPpv.mockResolvedValue({
      query: "sweden japan world cup",
      backend: "openrouter",
      message: null,
      items: [
        {
          confidence: 0.91,
          reason: "Teams and competition match the event title.",
          matchedTerms: ["sweden", "japan"],
          channel: {
            id: "ppv-channel-ai",
            name: "LIVE | FIFA WWC Sweden v Japan | SE: VIAPLAY PPV 3",
            logoUrl: null,
            categoryName: "SE| VIAPLAY PPV",
            remoteStreamId: 3,
            epgChannelId: "ai-3",
            hasEpg: true,
            hasCatchup: false,
            archiveDurationHours: null,
            streamExtension: "m3u8",
            isFavorite: false,
            isPpv: true,
            isPpvFavorite: false,
          },
          program: {
            id: "program-ai",
            channelId: "ppv-channel-ai",
            channelName: "LIVE | FIFA WWC Sweden v Japan | SE: VIAPLAY PPV 3",
            title: "Sweden v Japan",
            description: "World Cup quarter-final",
            startAt: "2026-04-04T11:30:00.000Z",
            endAt: "2026-04-04T13:30:00.000Z",
            canCatchup: false,
          },
        },
      ],
    });

    renderPpvFavoritesPage();
    fireEvent.change(screen.getByLabelText(/describe a ppv event/i), {
      target: { value: "sweden japan world cup" },
    });
    fireEvent.click(screen.getByRole("button", { name: /find ppv matches/i }));

    await waitFor(() => expect(mockedSearchAiPpv).toHaveBeenCalled());
    expect(mockedSearchAiPpv.mock.calls[0]?.[0]).toEqual({
      query: "sweden japan world cup",
      limit: 12,
    });
    expect(await screen.findByText("OpenRouter")).toBeInTheDocument();
    expect(screen.getByText("91% match")).toBeInTheDocument();
    expect(screen.getByText("Teams and competition match the event title.")).toBeInTheDocument();
    expect(screen.getByText("Sweden v Japan")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /save ppv/i }));
    await waitFor(() =>
      expect(mockedAddPpvFavorite).toHaveBeenCalledWith("ppv-channel-ai"),
    );
    expect(screen.getByRole("button", { name: /remove ppv/i })).toBeInTheDocument();
    expect(screen.getByText("PPV saved")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^play$/i }));
    await waitFor(() =>
      expect(mockedStartChannelPlayback).toHaveBeenCalledWith("ppv-channel-ai"),
    );
  });

  it("pins title-derived live ppv favorites above non-live entries when epg is missing", async () => {
    vi.mocked(Date.now).mockReturnValue(
      new Date("2026-04-17T17:00:00.000Z").getTime(),
    );
    mockedGetPpvFavorites.mockResolvedValue([
      {
        kind: "channel",
        order: 0,
        channel: {
          id: "ppv-channel-upcoming",
          name: "US (ESPN+ 034) | RBC Heritage: Spieth Featured Group (Second Round) Apr 17 2:00PM ET",
          logoUrl: null,
          categoryName: "US| ESPN+ PPV",
          remoteStreamId: 34,
          epgChannelId: null,
          hasEpg: false,
          hasCatchup: false,
          archiveDurationHours: null,
          streamExtension: "m3u8",
          isFavorite: false,
          isPpv: true,
          isPpvFavorite: true,
        },
        program: null,
      },
      {
        kind: "channel",
        order: 1,
        channel: {
          id: "ppv-channel-live",
          name: "US (ESPN+ 002) | RBC Heritage: Main Feed (Second Round) Apr 17 7:00AM ET",
          logoUrl: null,
          categoryName: "US| ESPN+ PPV",
          remoteStreamId: 2,
          epgChannelId: null,
          hasEpg: false,
          hasCatchup: false,
          archiveDurationHours: null,
          streamExtension: "m3u8",
          isFavorite: false,
          isPpv: true,
          isPpvFavorite: true,
        },
        program: null,
      },
      {
        kind: "channel",
        order: 2,
        channel: {
          id: "ppv-channel-later",
          name: "US (ESPN+ 006) | RBC Heritage: Featured Groups (Second Round) Apr 17 9:30AM ET",
          logoUrl: null,
          categoryName: "US| ESPN+ PPV",
          remoteStreamId: 6,
          epgChannelId: null,
          hasEpg: false,
          hasCatchup: false,
          archiveDurationHours: null,
          streamExtension: "m3u8",
          isFavorite: false,
          isPpv: true,
          isPpvFavorite: true,
        },
        program: null,
      },
    ]);

    renderPpvFavoritesPage();

    const headings = await screen.findAllByRole("heading", { level: 2 });
    expect(headings.map((heading) => heading.textContent)).toEqual([
      expect.stringContaining("Main Feed"),
      expect.stringContaining("Featured Groups"),
      expect.stringContaining("Spieth Featured Group"),
    ]);
    expect(screen.getByText("2 live now")).toBeInTheDocument();
    expect(screen.getAllByText("Live now").length).toBeGreaterThan(0);
  });
});
