import { fireEvent, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { SettingsPage } from "@/features/auth/settings-page";
import { getProvider, getRecents, getSyncStatus } from "@/lib/api";
import { useThemeStore } from "@/store/theme-store";

vi.mock("@/lib/api", () => ({
  addFavorite: vi.fn(),
  getProvider: vi.fn(),
  getRecents: vi.fn(),
  getSyncStatus: vi.fn(),
  removeFavorite: vi.fn(),
  saveProvider: vi.fn(),
  triggerProviderSync: vi.fn(),
  validateProvider: vi.fn(),
}));

const mockedGetRecents = vi.mocked(getRecents);
const mockedGetProvider = vi.mocked(getProvider);
const mockedGetSyncStatus = vi.mocked(getSyncStatus);

describe("SettingsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedGetRecents.mockResolvedValue([]);
    mockedGetProvider.mockResolvedValue(null);
    mockedGetSyncStatus.mockResolvedValue(null);
    useThemeStore.getState().setPreference("system");
  });

  it("updates the theme when a toggle is selected", async () => {
    render(
      <QueryClientProvider client={new QueryClient()}>
        <SettingsPage />
      </QueryClientProvider>,
    );

    fireEvent.click(screen.getByRole("radio", { name: /light/i }));

    expect(useThemeStore.getState().preference).toBe("light");
    expect(document.documentElement.dataset.theme).toBe("light");
  });

  it("shows provider controls inside settings and removes the sessions card", async () => {
    mockedGetRecents.mockResolvedValue([
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
        lastPlayedAt: "2026-04-04T12:00:00.000Z",
      },
    ]);

    render(
      <QueryClientProvider client={new QueryClient()}>
        <SettingsPage />
      </QueryClientProvider>,
    );

    expect(await screen.findByText("Provider")).toBeInTheDocument();
    expect(await screen.findByText("Arena 1")).toBeInTheDocument();
    expect(screen.queryByText("Sessions")).not.toBeInTheDocument();
    expect(screen.getByTestId("recent-channels-scroll-area")).toBeInTheDocument();
  });
});
