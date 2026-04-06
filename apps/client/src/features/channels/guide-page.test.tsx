import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { GuidePage } from "@/features/channels/guide-page";
import { getGuide, getGuideCategory, getGuidePreferences, saveGuidePreferences } from "@/lib/api";

vi.mock("@/lib/api", () => ({
  addFavorite: vi.fn(),
  getGuide: vi.fn(),
  getGuideCategory: vi.fn(),
  getGuidePreferences: vi.fn(),
  removeFavorite: vi.fn(),
  saveGuidePreferences: vi.fn(),
  startChannelPlayback: vi.fn(),
  startRemoteChannelPlayback: vi.fn(),
}));

const mockedGetGuide = vi.mocked(getGuide);
const mockedGetGuideCategory = vi.mocked(getGuideCategory);
const mockedGetGuidePreferences = vi.mocked(getGuidePreferences);
const mockedSaveGuidePreferences = vi.mocked(saveGuidePreferences);

describe("GuidePage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedGetGuidePreferences.mockResolvedValue({ includedCategoryIds: [] });
    mockedSaveGuidePreferences.mockResolvedValue({ includedCategoryIds: [] });
    mockedGetGuide.mockResolvedValue({
      categories: [
        {
          id: "sports",
          name: "Sports",
          channelCount: 2,
          liveNowCount: 1,
        },
      ],
    });
  });

  function renderGuidePage() {
    const queryClient = new QueryClient({
      defaultOptions: {
        queries: {
          retry: false,
        },
      },
    });

    return render(
      <QueryClientProvider client={queryClient}>
        <GuidePage />
      </QueryClientProvider>,
    );
  }

  it("loads only guide categories on first render", async () => {
    renderGuidePage();

    expect((await screen.findAllByText("Sports")).length).toBeGreaterThan(0);
    expect(mockedGetGuide).toHaveBeenCalledTimes(1);
    expect(mockedGetGuideCategory).not.toHaveBeenCalled();
  });

  it("loads a category only after it is expanded", async () => {
    mockedGetGuideCategory.mockResolvedValue({
      category: {
        id: "sports",
        name: "Sports",
        channelCount: 2,
        liveNowCount: 1,
      },
      entries: [
        {
          channel: {
            id: "channel-1",
            name: "Arena 1",
            logoUrl: null,
            categoryName: "Sports",
            remoteStreamId: 1,
            epgChannelId: null,
            hasEpg: true,
            hasCatchup: true,
            archiveDurationHours: 24,
            streamExtension: "m3u8",
            isFavorite: false,
          },
          program: {
            id: "program-1",
            channelId: "channel-1",
            channelName: "Arena 1",
            title: "Matchday Live",
            description: null,
            startAt: "2026-04-04T10:00:00.000Z",
            endAt: "2026-04-04T12:00:00.000Z",
            canCatchup: true,
          },
        },
      ],
      totalCount: 1,
      nextOffset: null,
    });

    renderGuidePage();
    fireEvent.click((await screen.findAllByRole("button", { name: /show channels/i }))[0]);

    await waitFor(() => expect(mockedGetGuideCategory).toHaveBeenCalledWith("sports", 0, 40));
    expect(await screen.findByText("Matchday Live")).toBeInTheDocument();
  });

  it("appends the next page when load more is pressed", async () => {
    mockedGetGuideCategory
      .mockResolvedValueOnce({
        category: {
          id: "sports",
          name: "Sports",
          channelCount: 2,
          liveNowCount: 1,
        },
        entries: [
          {
            channel: {
              id: "channel-1",
              name: "Arena 1",
              logoUrl: null,
              categoryName: "Sports",
              remoteStreamId: 1,
              epgChannelId: null,
              hasEpg: true,
              hasCatchup: true,
              archiveDurationHours: 24,
              streamExtension: "m3u8",
              isFavorite: false,
            },
            program: {
              id: "program-1",
              channelId: "channel-1",
              channelName: "Arena 1",
              title: "Matchday Live",
              description: null,
              startAt: "2026-04-04T10:00:00.000Z",
              endAt: "2026-04-04T12:00:00.000Z",
              canCatchup: true,
            },
          },
        ],
        totalCount: 2,
        nextOffset: 40,
      })
      .mockResolvedValueOnce({
        category: {
          id: "sports",
          name: "Sports",
          channelCount: 2,
          liveNowCount: 1,
        },
        entries: [
          {
            channel: {
              id: "channel-2",
              name: "Arena 2",
              logoUrl: null,
              categoryName: "Sports",
              remoteStreamId: 2,
              epgChannelId: null,
              hasEpg: true,
              hasCatchup: false,
              archiveDurationHours: null,
              streamExtension: "m3u8",
              isFavorite: false,
            },
            program: {
              id: "program-2",
              channelId: "channel-2",
              channelName: "Arena 2",
              title: "Late Kickoff",
              description: null,
              startAt: "2026-04-04T12:00:00.000Z",
              endAt: "2026-04-04T14:00:00.000Z",
              canCatchup: false,
            },
          },
        ],
        totalCount: 2,
        nextOffset: null,
      });

    renderGuidePage();
    fireEvent.click((await screen.findAllByRole("button", { name: /show channels/i }))[0]);

    expect(await screen.findByText("Matchday Live")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /load more/i }));

    await waitFor(() => expect(mockedGetGuideCategory).toHaveBeenNthCalledWith(2, "sports", 40, 40));
    expect(await screen.findByText("Late Kickoff")).toBeInTheDocument();
    expect(screen.getByText("Matchday Live")).toBeInTheDocument();
  });

  it("shows channels even when a category entry has no program metadata", async () => {
    mockedGetGuideCategory.mockResolvedValue({
      category: {
        id: "sports",
        name: "Sports",
        channelCount: 2,
        liveNowCount: 0,
      },
      entries: [
        {
          channel: {
            id: "channel-3",
            name: "Arena 3",
            logoUrl: null,
            categoryName: "Sports",
            remoteStreamId: 3,
            epgChannelId: null,
            hasEpg: false,
            hasCatchup: false,
            archiveDurationHours: null,
            streamExtension: "m3u8",
            isFavorite: false,
          },
          program: null,
        },
      ],
      totalCount: 1,
      nextOffset: null,
    });

    renderGuidePage();
    fireEvent.click((await screen.findAllByRole("button", { name: /show channels/i }))[0]);

    expect(await screen.findByText("Arena 3")).toBeInTheDocument();
    expect(screen.getByText("No program data")).toBeInTheDocument();
  });

  it("shows only saved included categories", async () => {
    mockedGetGuidePreferences.mockResolvedValue({
      includedCategoryIds: ["sports"],
    });
    mockedGetGuide.mockResolvedValue({
      categories: [
        {
          id: "sports",
          name: "Sports",
          channelCount: 2,
          liveNowCount: 1,
        },
        {
          id: "news",
          name: "News",
          channelCount: 1,
          liveNowCount: 0,
        },
      ],
    });

    renderGuidePage();

    expect(await screen.findByRole("button", { name: /show channels/i })).toBeInTheDocument();
    expect(screen.getAllByText("Sports").length).toBeGreaterThan(0);
    expect(screen.getAllByRole("button", { name: /show channels/i })).toHaveLength(1);
  });

  it("applies a custom filter only after pressing Enter", async () => {
    mockedGetGuide.mockResolvedValue({
      categories: [
        {
          id: "sports",
          name: "Sports",
          channelCount: 2,
          liveNowCount: 1,
        },
        {
          id: "news",
          name: "News",
          channelCount: 1,
          liveNowCount: 0,
        },
      ],
    });

    renderGuidePage();
    fireEvent.click(await screen.findByRole("button", { name: /show filter/i }));

    fireEvent.change(await screen.findByPlaceholderText(/filter categories/i), {
      target: { value: "news" },
    });

    expect(screen.getByRole("button", { name: /sports/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /news/i })).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /show channels/i })).toHaveLength(2);

    fireEvent.keyDown(screen.getByPlaceholderText(/filter categories/i), {
      key: "Enter",
      code: "Enter",
    });

    expect(screen.getByRole("button", { name: /news/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /sports/i })).not.toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /show channels/i })).toHaveLength(1);
    expect(screen.getAllByText("News").length).toBeGreaterThan(0);
    expect(screen.queryByText("Sports")).not.toBeInTheDocument();
    expect(mockedGetGuideCategory).not.toHaveBeenCalled();
  });

  it("applies the custom filter on top of the selected categories", async () => {
    mockedGetGuide.mockResolvedValue({
      categories: [
        {
          id: "sports",
          name: "Sports",
          channelCount: 2,
          liveNowCount: 1,
        },
        {
          id: "news",
          name: "News",
          channelCount: 1,
          liveNowCount: 0,
        },
      ],
    });
    mockedGetGuidePreferences.mockResolvedValue({
      includedCategoryIds: ["sports"],
    });

    renderGuidePage();
    fireEvent.click(await screen.findByRole("button", { name: /show filter/i }));

    fireEvent.change(await screen.findByPlaceholderText(/filter categories/i), {
      target: { value: "news" },
    });
    fireEvent.keyDown(screen.getByPlaceholderText(/filter categories/i), {
      key: "Enter",
      code: "Enter",
    });

    expect(screen.getByRole("button", { name: /news/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /sports/i })).not.toBeInTheDocument();
    expect(screen.queryAllByRole("button", { name: /show channels/i })).toHaveLength(0);
    expect(screen.getByText("No categories match this filter")).toBeInTheDocument();
  });

  it("starts with the filter panel collapsed and can expand it", async () => {
    renderGuidePage();

    expect(await screen.findByRole("button", { name: /show filter/i })).toBeInTheDocument();
    expect(screen.queryByPlaceholderText(/filter categories/i)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /show filter/i }));

    expect(await screen.findByPlaceholderText(/filter categories/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /hide filter/i })).toBeInTheDocument();
  });
});
