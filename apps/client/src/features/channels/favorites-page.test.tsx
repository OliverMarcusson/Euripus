import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { FavoritesPage } from "@/features/channels/favorites-page";
import { getFavorites } from "@/lib/api";
import { formatTimeRange } from "@/lib/utils";

vi.mock("@/lib/api", () => ({
  addFavorite: vi.fn(),
  getFavorites: vi.fn(),
  removeFavorite: vi.fn(),
  startChannelPlayback: vi.fn(),
  startRemoteChannelPlayback: vi.fn(),
}));

const mockedGetFavorites = vi.mocked(getFavorites);

describe("FavoritesPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(Date, "now").mockReturnValue(
      new Date("2026-04-04T12:00:00.000Z").getTime(),
    );
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  function renderFavoritesPage() {
    const queryClient = new QueryClient({
      defaultOptions: {
        queries: {
          retry: false,
        },
      },
    });

    return render(
      <QueryClientProvider client={queryClient}>
        <FavoritesPage />
      </QueryClientProvider>,
    );
  }

  it("shows epg metadata when a favorite has a current program", async () => {
    mockedGetFavorites.mockResolvedValue([
      {
        channel: {
          id: "channel-1",
          name: "Arena 1",
          logoUrl: null,
          categoryName: "Sports",
          remoteStreamId: 1,
          epgChannelId: "arena-1",
          hasCatchup: true,
          archiveDurationHours: 24,
          streamExtension: "m3u8",
          isFavorite: true,
        },
        program: {
          id: "program-1",
          channelId: "channel-1",
          channelName: "Arena 1",
          title: "Matchday Live",
          description: "League coverage",
          startAt: "2026-04-04T11:30:00.000Z",
          endAt: "2026-04-04T12:30:00.000Z",
          canCatchup: true,
        },
      },
    ]);

    renderFavoritesPage();

    expect(await screen.findByText("Matchday Live")).toBeInTheDocument();
    expect(screen.getByText("Live now")).toBeInTheDocument();
    expect(screen.getByText("League coverage")).toBeInTheDocument();
    expect(
      screen.getByText(
        formatTimeRange(
          "2026-04-04T11:30:00.000Z",
          "2026-04-04T12:30:00.000Z",
        ),
      ),
    ).toBeInTheDocument();
    expect(screen.getAllByText("Catch-up").length).toBeGreaterThan(0);
  });

  it("keeps the row clean when no epg is available", async () => {
    mockedGetFavorites.mockResolvedValue([
      {
        channel: {
          id: "channel-2",
          name: "Arena 2",
          logoUrl: null,
          categoryName: "Sports",
          remoteStreamId: 2,
          epgChannelId: null,
          hasCatchup: false,
          archiveDurationHours: null,
          streamExtension: "m3u8",
          isFavorite: true,
        },
        program: null,
      },
    ]);

    renderFavoritesPage();

    expect(await screen.findByText("Arena 2")).toBeInTheDocument();
    expect(screen.queryByText("Live now")).not.toBeInTheDocument();
    expect(screen.queryByText("Upcoming")).not.toBeInTheDocument();
    expect(screen.queryByText("Info only")).not.toBeInTheDocument();
  });
});
