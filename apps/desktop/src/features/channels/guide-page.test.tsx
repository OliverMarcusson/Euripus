import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { GuidePage } from "@/features/channels/guide-page";
import { getGuide, getGuideCategory } from "@/lib/api";

vi.mock("@/lib/api", () => ({
  addFavorite: vi.fn(),
  getGuide: vi.fn(),
  getGuideCategory: vi.fn(),
  removeFavorite: vi.fn(),
  startChannelPlayback: vi.fn(),
}));

const mockedGetGuide = vi.mocked(getGuide);
const mockedGetGuideCategory = vi.mocked(getGuideCategory);

describe("GuidePage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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

    expect(await screen.findByText("Sports")).toBeInTheDocument();
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
    expect(screen.getByText("No current program metadata synced yet.")).toBeInTheDocument();
  });

  it("filters only by category name without using loaded channel entries", async () => {
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
    expect(await screen.findByText("Arena 1")).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText(/filter guide categories/i), {
      target: { value: "arena" },
    });

    expect(screen.queryByText("Sports")).not.toBeInTheDocument();
    expect(screen.queryByText("Arena 1")).not.toBeInTheDocument();
    expect(screen.queryByText("News")).not.toBeInTheDocument();
    expect(screen.getByText("No guide matches")).toBeInTheDocument();
    expect(mockedGetGuideCategory).toHaveBeenCalledTimes(1);
  });
});
