import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { PpvFavoritesPage } from "@/features/channels/ppv-favorites-page";
import { getPpvFavorites } from "@/lib/api";
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
  startChannelPlayback: vi.fn(),
  startRemoteChannelPlayback: vi.fn(),
}));

const mockedGetPpvFavorites = vi.mocked(getPpvFavorites);

describe("PpvFavoritesPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(Date, "now").mockReturnValue(
      new Date("2026-04-04T12:00:00.000Z").getTime(),
    );
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
    expect(
      screen.getByText(
        formatTimeRange(
          "2026-04-04T11:30:00.000Z",
          "2026-04-04T12:30:00.000Z",
        ),
      ),
    ).toBeInTheDocument();
  });
});
