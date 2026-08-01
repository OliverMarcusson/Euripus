import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { DiscoverTitle } from "@euripus/shared";
import { DiscoverPage } from "@/features/discover/discover-page";
import { getDiscoverCharts, getDiscoverTitles } from "@/lib/api";

vi.mock("@/lib/api", () => ({
  getDiscoverCharts: vi.fn(),
  getDiscoverTitles: vi.fn(),
  getOnDemandTitle: vi.fn(),
  getSeriesEpisodes: vi.fn(),
  startEpisodePlayback: vi.fn(),
  startOnDemandPlayback: vi.fn(),
  startRemoteEpisodePlayback: vi.fn(),
  startRemoteOnDemandPlayback: vi.fn(),
  seekRemotePlayback: vi.fn(),
  updateOnDemandProgress: vi.fn(),
}));

const mockedCharts = vi.mocked(getDiscoverCharts);
const mockedTitles = vi.mocked(getDiscoverTitles);

function title(overrides: Partial<DiscoverTitle> & { rank: number; tmdbId: number; title: string }): DiscoverTitle {
  return {
    mediaType: "movie",
    originalTitle: overrides.title,
    originCountries: [],
    overview: null,
    posterUrl: null,
    backdropUrl: null,
    releaseDate: null,
    releaseYear: 2024,
    voteAverage: 7.5,
    voteCount: 100,
    onDemandTitleId: null,
    providerLabel: null,
    ...overrides,
  };
}

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={client}><DiscoverPage /></QueryClientProvider>);
}

describe("DiscoverPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedCharts.mockResolvedValue({
      enabled: true,
      countries: ["SE", "US"],
      charts: [
        { chart: "trending", countryMode: "global" },
        { chart: "popular", countryMode: "global" },
        { chart: "popular", countryMode: "available_in" },
        { chart: "popular", countryMode: "from" },
        { chart: "top_rated", countryMode: "from" },
      ],
      lastRefreshedAt: null,
    });
    mockedTitles.mockResolvedValue({
      items: [
        title({ rank: 1, tmdbId: 1, title: "Owned Movie", onDemandTitleId: "od-1", providerLabel: "Main TV" }),
        title({ rank: 2, tmdbId: 2, title: "Missing Movie" }),
      ],
      totalCount: 2,
      nextOffset: null,
      availableCount: 1,
    });
  });

  it("marks titles the providers do not carry as unavailable without dropping them", async () => {
    renderPage();

    // Both keep their chart position; only the unmatched one is disabled.
    expect(await screen.findByText("Owned Movie")).toBeInTheDocument();
    expect(screen.getByText("Missing Movie")).toBeInTheDocument();
    expect(screen.getByText("#1")).toBeInTheDocument();
    expect(screen.getByText("#2")).toBeInTheDocument();

    expect(screen.getByLabelText("Open Owned Movie")).toBeEnabled();
    expect(
      screen.getByLabelText("Missing Movie is not available from your providers"),
    ).toBeDisabled();
    expect(screen.getByText("Not in your providers")).toBeInTheDocument();
    expect(screen.getByText("1 of 2 on this page are in your providers.")).toBeInTheDocument();
  });

  it("requests the default worldwide trending chart without a country", async () => {
    renderPage();
    await waitFor(() => expect(mockedTitles).toHaveBeenCalled());

    expect(mockedTitles).toHaveBeenCalledWith("movie", expect.objectContaining({
      chart: "trending",
      countryMode: "global",
      country: undefined,
    }));
  });

  it("selects a country and falls back to a chart the mode supports", async () => {
    renderPage();
    await waitFor(() => expect(mockedTitles).toHaveBeenCalled());
    mockedTitles.mockClear();

    // "Available in" has no trending chart, so the active chart must move to one it does
    // have rather than requesting a combination the server never refreshes.
    fireEvent.click(screen.getByRole("button", { name: "Available in" }));

    await waitFor(() => {
      expect(mockedTitles).toHaveBeenLastCalledWith("movie", expect.objectContaining({
        chart: "popular",
        countryMode: "available_in",
        country: "SE",
      }));
    });
    expect(screen.queryByRole("button", { name: "Trending" })).not.toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Country"), { target: { value: "US" } });
    await waitFor(() => {
      expect(mockedTitles).toHaveBeenLastCalledWith("movie", expect.objectContaining({
        countryMode: "available_in",
        country: "US",
      }));
    });
  });

  it("explains the country scope instead of implying a regional popularity chart", async () => {
    renderPage();
    await waitFor(() => expect(mockedTitles).toHaveBeenCalled());

    fireEvent.click(screen.getByRole("button", { name: "From" }));
    expect(await screen.findByText("Titles produced in Sweden.")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Available in" }));
    expect(
      await screen.findByText("Globally popular titles licensed to stream in Sweden."),
    ).toBeInTheDocument();
  });

  it("filters unavailable titles on the server so paging stays consistent", async () => {
    renderPage();
    await waitFor(() => expect(mockedTitles).toHaveBeenCalled());
    mockedTitles.mockClear();
    mockedTitles.mockResolvedValue({
      items: [title({ rank: 1, tmdbId: 1, title: "Owned Movie", onDemandTitleId: "od-1", providerLabel: "Main TV" })],
      totalCount: 1,
      nextOffset: null,
      availableCount: 1,
    });

    fireEvent.click(screen.getByRole("button", { name: /Only in my providers/ }));

    // Filtering has to happen server-side, otherwise totalCount would still say 2 while
    // the grid shows one card.
    await waitFor(() => {
      expect(mockedTitles).toHaveBeenLastCalledWith("movie", expect.objectContaining({
        availableOnly: true,
        offset: 0,
      }));
    });
    expect(await screen.findByText("Owned Movie")).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByText("Missing Movie")).not.toBeInTheDocument());
    // The ratio line is noise once unmatched rows are gone.
    expect(screen.queryByText(/on this page are in your providers/)).not.toBeInTheDocument();
  });

  it("returns to the first page when the availability filter changes", async () => {
    mockedTitles.mockResolvedValue({
      items: [title({ rank: 41, tmdbId: 41, title: "Page Two Movie" })],
      totalCount: 200,
      nextOffset: 80,
      availableCount: 0,
    });
    renderPage();
    expect(await screen.findByText("Page Two Movie")).toBeInTheDocument();

    // Paging follows the server's nextOffset rather than adding a page size locally.
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    await waitFor(() => {
      expect(mockedTitles).toHaveBeenLastCalledWith("movie", expect.objectContaining({ offset: 80 }));
    });

    // An offset into the unfiltered chart is meaningless once the chart is filtered.
    fireEvent.click(screen.getByRole("button", { name: /Only in my providers/ }));
    await waitFor(() => {
      expect(mockedTitles).toHaveBeenLastCalledWith("movie", expect.objectContaining({
        availableOnly: true,
        offset: 0,
      }));
    });
  });

  it("explains what to configure when the server has no TMDB key", async () => {
    mockedCharts.mockResolvedValue({ enabled: false, countries: [], charts: [], lastRefreshedAt: null });
    renderPage();

    expect(await screen.findByText("Discover is not configured")).toBeInTheDocument();
    expect(mockedTitles).not.toHaveBeenCalled();
  });
});
