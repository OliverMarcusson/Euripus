import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { SportsPage } from "@/features/sports/sports-page";
import {
  getSportsEvent,
  getSportsLiveEvents,
  getSportsProviders,
  getSportsTodayEvents,
  getSportsUpcomingEvents,
} from "@/lib/api";

vi.mock("@/lib/api", () => ({
  getSportsEvent: vi.fn(),
  getSportsLiveEvents: vi.fn(),
  getSportsProviders: vi.fn(),
  getSportsTodayEvents: vi.fn(),
  getSportsUpcomingEvents: vi.fn(),
}));

const mockedGetSportsEvent = vi.mocked(getSportsEvent);
const mockedGetSportsLiveEvents = vi.mocked(getSportsLiveEvents);
const mockedGetSportsProviders = vi.mocked(getSportsProviders);
const mockedGetSportsTodayEvents = vi.mocked(getSportsTodayEvents);
const mockedGetSportsUpcomingEvents = vi.mocked(getSportsUpcomingEvents);

const sampleEvent = {
  id: "allsvenskan-1",
  sport: "soccer",
  competition: "allsvenskan",
  title: "Halmstads BK vs IFK Göteborg",
  startTime: "2026-04-18T11:00:00.000Z",
  endTime: "2026-04-18T13:00:00.000Z",
  status: "live",
  venue: "Örjans Vall",
  roundLabel: "Round 3",
  participants: {
    home: "Halmstads BK",
    away: "IFK Göteborg",
  },
  source: "allsvenskan-fixture",
  sourceUrl: "https://example.com/event",
  watch: {
    recommendedMarket: "se",
    recommendedProvider: "TV4 Play",
    availabilities: [
      {
        market: "se",
        providerFamily: "tv4",
        providerLabel: "TV4 Play",
        channelName: "TV4 Fotboll",
        watchType: "streaming+linear",
        confidence: 0.93,
        source: "overlay",
        searchHints: ["Halmstads BK vs IFK Göteborg TV4 Play"],
      },
    ],
  },
  searchMetadata: {
    queries: ["Halmstads BK vs IFK Göteborg"],
    keywords: ["allsvenskan"],
  },
};

describe("SportsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedGetSportsLiveEvents.mockResolvedValue({
      count: 1,
      events: [sampleEvent],
    });
    mockedGetSportsTodayEvents.mockResolvedValue({
      count: 1,
      events: [sampleEvent],
    });
    mockedGetSportsUpcomingEvents.mockResolvedValue({
      count: 1,
      events: [
        {
          ...sampleEvent,
          id: "premier-1",
          competition: "premier_league",
          title: "Arsenal vs Liverpool",
          participants: { home: "Arsenal", away: "Liverpool" },
          status: "upcoming",
        },
      ],
    });
    mockedGetSportsProviders.mockResolvedValue({
      count: 2,
      providers: [
        { family: "tv4", market: "se", aliases: ["TV4 Play"] },
        { family: "viaplay", market: "se", aliases: ["Viaplay"] },
      ],
    });
    mockedGetSportsEvent.mockResolvedValue(sampleEvent);
  });

  function renderSportsPage() {
    const queryClient = new QueryClient({
      defaultOptions: {
        queries: {
          retry: false,
        },
      },
    });

    return render(
      <QueryClientProvider client={queryClient}>
        <SportsPage />
      </QueryClientProvider>,
    );
  }

  it("renders sports views and opens event detail", async () => {
    renderSportsPage();

    expect(await screen.findByRole("heading", { name: "Sports" })).toBeInTheDocument();
    expect((await screen.findAllByText("Live Now")).length).toBeGreaterThan(0);
    expect(await screen.findByRole("heading", { name: /Halmstads BK vs IFK Göteborg/i })).toBeInTheDocument();
    expect(screen.getAllByText("Round 3").length).toBeGreaterThan(0);

    expect(screen.getByRole("tab", { name: /Later/i })).toBeInTheDocument();

    fireEvent.click(screen.getAllByRole("button", { name: /view details/i })[0]);

    await waitFor(() => expect(mockedGetSportsEvent).toHaveBeenCalledWith("allsvenskan-1"));
    expect(await screen.findByText("Watch guidance")).toBeInTheDocument();
    expect(screen.getAllByText("TV4 Fotboll").length).toBeGreaterThan(0);
  });

  it("filters sports views by competition chip", async () => {
    renderSportsPage();

    fireEvent.click(await screen.findByRole("radio", { name: /Allsvenskan/i }));

    await waitFor(() => {
      expect(screen.getByText("Showing Allsvenskan")).toBeInTheDocument();
    });
    expect(screen.getByRole("tab", { name: /Later/i })).toHaveTextContent("0 events");
    expect(screen.getByRole("heading", { name: /Halmstads BK vs IFK Göteborg/i })).toBeInTheDocument();
  });
});
