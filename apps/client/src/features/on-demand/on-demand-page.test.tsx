import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { OnDemandPage } from "@/features/on-demand/on-demand-page";
import { getOnDemandCategories, getOnDemandHistory, getOnDemandTitles } from "@/lib/api";

vi.mock("@/lib/api", () => ({
  addOnDemandCategoryFavorite: vi.fn(),
  addOnDemandTitleFavorite: vi.fn(),
  removeOnDemandCategoryFavorite: vi.fn(),
  removeOnDemandTitleFavorite: vi.fn(),
  getOnDemandCategories: vi.fn(),
  getOnDemandHistory: vi.fn(),
  getOnDemandTitles: vi.fn(),
  getOnDemandTitle: vi.fn(),
  getSeriesEpisodes: vi.fn(),
  startEpisodePlayback: vi.fn(),
  startOnDemandPlayback: vi.fn(),
  startRemoteEpisodePlayback: vi.fn(),
  startRemoteOnDemandPlayback: vi.fn(),
  seekRemotePlayback: vi.fn(),
  updateOnDemandProgress: vi.fn(),
}));

const mockedCategories = vi.mocked(getOnDemandCategories);
const mockedHistory = vi.mocked(getOnDemandHistory);
const mockedTitles = vi.mocked(getOnDemandTitles);

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={client}><OnDemandPage /></QueryClientProvider>);
}

describe("OnDemandPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedCategories.mockResolvedValue([{ id: "movies", mediaType: "movie", name: "Movies", titleCount: 1, isFavorite: false }]);
    mockedHistory.mockResolvedValue([]);
    mockedTitles.mockResolvedValue({
      items: [{ id: "title-1", mediaType: "movie", name: "Example Movie", categoryId: "movies", categoryName: "Movies", posterUrl: null, backdropUrl: null, plot: "A movie.", genre: "Drama", castNames: null, director: null, releaseDate: "2026", rating: 8, durationMinutes: 90, containerExtension: "mp4", isFavorite: false }],
      totalCount: 1,
      nextOffset: null,
    });
  });

  it("renders and filters the movie catalog", async () => {
    renderPage();
    expect(await screen.findByText("Example Movie")).toBeInTheDocument();
    fireEvent.change(screen.getByPlaceholderText("Search movies"), { target: { value: "example" } });
    await waitFor(() => expect(mockedTitles).toHaveBeenLastCalledWith("movie", expect.objectContaining({ query: "example" })));
  });

  it("switches to the series catalog", async () => {
    renderPage();
    const seriesTab = screen.getByRole("tab", { name: "Series" });
    fireEvent.mouseDown(seriesTab, { button: 0 });
    fireEvent.click(seriesTab);
    await waitFor(() => expect(mockedCategories).toHaveBeenCalledWith("series"));
  });

  it("shows history with start-over and continue actions", async () => {
    mockedHistory.mockResolvedValue([{
      ...(await mockedTitles("movie", {})).items[0],
      episodeId: null,
      episodeName: null,
      seasonNumber: null,
      episodeNumber: null,
      positionSeconds: 600,
      durationSeconds: 5400,
      lastPlayedAt: "2026-07-17T12:00:00Z",
    }]);
    renderPage();
    const historyTab = screen.getByRole("tab", { name: "History" });
    fireEvent.mouseDown(historyTab, { button: 0 });
    fireEvent.click(historyTab);
    expect(await screen.findByRole("button", { name: "Start over" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Continue" })).toBeInTheDocument();
  });
});
