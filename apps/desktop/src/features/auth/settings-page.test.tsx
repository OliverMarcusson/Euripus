import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { SettingsPage } from "@/features/auth/settings-page";
import {
  getProvider,
  getRecents,
  getRemoteDevices,
  getServerNetworkStatus,
  getSyncStatus,
  saveProvider,
  startChannelPlayback,
  startRemoteChannelPlayback,
  triggerProviderSync,
} from "@/lib/api";
import { usePlaybackDeviceStore } from "@/store/playback-device-store";
import { useThemeStore } from "@/store/theme-store";
import { useTvModeStore } from "@/store/tv-mode-store";

vi.mock("@/lib/api", () => ({
  addFavorite: vi.fn(),
  getProvider: vi.fn(),
  getRecents: vi.fn(),
  getRemoteDevices: vi.fn(),
  getServerNetworkStatus: vi.fn(),
  getSyncStatus: vi.fn(),
  removeFavorite: vi.fn(),
  saveProvider: vi.fn(),
  startChannelPlayback: vi.fn(),
  startRemoteChannelPlayback: vi.fn(),
  triggerProviderSync: vi.fn(),
  validateProvider: vi.fn(),
}));

const mockedGetRecents = vi.mocked(getRecents);
const mockedGetProvider = vi.mocked(getProvider);
const mockedGetRemoteDevices = vi.mocked(getRemoteDevices);
const mockedGetServerNetworkStatus = vi.mocked(getServerNetworkStatus);
const mockedGetSyncStatus = vi.mocked(getSyncStatus);
const mockedSaveProvider = vi.mocked(saveProvider);
const mockedStartChannelPlayback = vi.mocked(startChannelPlayback);
const mockedStartRemoteChannelPlayback = vi.mocked(startRemoteChannelPlayback);
const mockedTriggerProviderSync = vi.mocked(triggerProviderSync);

describe("SettingsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedGetRecents.mockResolvedValue([]);
    mockedGetProvider.mockResolvedValue(null);
    mockedGetRemoteDevices.mockResolvedValue([]);
    mockedGetServerNetworkStatus.mockResolvedValue({
      serverStatus: "online",
      vpnActive: false,
      vpnProvider: null,
      publicIp: "203.0.113.42",
      publicIpCheckedAt: "2026-04-05T10:00:00.000Z",
      publicIpError: null,
    });
    mockedGetSyncStatus.mockResolvedValue(null);
    mockedTriggerProviderSync.mockResolvedValue({
      id: "sync-job-1",
      status: "queued",
      jobType: "full",
      trigger: "manual",
      createdAt: "2026-04-05T10:00:00.000Z",
      startedAt: null,
      finishedAt: null,
      currentPhase: "queued",
      completedPhases: 0,
      totalPhases: 7,
      phaseMessage: "Waiting to start",
      errorMessage: null,
    });
    mockedSaveProvider.mockImplementation(async (payload) => ({
      id: "provider-1",
      providerType: "xtreme",
      baseUrl: payload.baseUrl,
      username: payload.username,
      outputFormat: payload.outputFormat,
      playbackMode: payload.playbackMode,
      status: "valid",
      lastValidatedAt: "2026-04-04T12:00:00.000Z",
      lastSyncAt: null,
      lastSyncError: null,
      createdAt: "2026-04-04T10:00:00.000Z",
      updatedAt: "2026-04-04T12:00:00.000Z",
      epgSources: payload.epgSources.map((source, index) => ({
        id: source.id ?? `epg-source-${index + 1}`,
        url: source.url,
        priority: source.priority,
        enabled: source.enabled,
        sourceKind: "external",
        lastSyncAt: null,
        lastSyncError: null,
        lastProgramCount: null,
        lastMatchedCount: null,
        createdAt: "2026-04-04T10:00:00.000Z",
        updatedAt: "2026-04-04T12:00:00.000Z",
      })),
    }));
    mockedStartChannelPlayback.mockResolvedValue({
      kind: "hls",
      url: "https://stream.example.com/channel.m3u8",
      headers: {},
      live: true,
      catchup: false,
      expiresAt: null,
      unsupportedReason: null,
      title: "Arena 1",
    });
    mockedStartRemoteChannelPlayback.mockResolvedValue({
      id: "remote-command-1",
      targetDeviceId: "tv-1",
      targetDeviceName: "Living room TV",
      status: "delivered",
      sourceTitle: "Arena 1",
      createdAt: "2026-04-05T10:00:00.000Z",
    });
    useThemeStore.getState().setPreference("system");
    useTvModeStore.getState().setPreference("auto");
    usePlaybackDeviceStore.setState({
      activeDeviceId: null,
      deviceKey: "device-1",
      name: "Browser device",
      remoteTargetEnabled: false,
      platform: "web",
      formFactorHint: "desktop",
    });
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

  it("lets the user enable remote playback target mode locally", async () => {
    render(
      <QueryClientProvider client={new QueryClient()}>
        <SettingsPage />
      </QueryClientProvider>,
    );

    fireEvent.change(screen.getByLabelText(/device name/i), {
      target: { value: "Living room TV" },
    });
    fireEvent.click(
      screen.getByRole("button", { name: /use this device as a playback target/i }),
    );

    expect(usePlaybackDeviceStore.getState().name).toBe("Living room TV");
    expect(usePlaybackDeviceStore.getState().remoteTargetEnabled).toBe(true);
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
    mockedGetProvider.mockResolvedValue({
      id: "provider-1",
      providerType: "xtreme",
      baseUrl: "https://provider.example.com",
      username: "demo",
      outputFormat: "m3u8",
      playbackMode: "direct",
      status: "valid",
      lastValidatedAt: "2026-04-04T12:00:00.000Z",
      lastSyncAt: "2026-04-04T12:30:00.000Z",
      lastSyncError: null,
      createdAt: "2026-04-04T10:00:00.000Z",
      updatedAt: "2026-04-04T12:30:00.000Z",
      epgSources: [
        {
          id: "epg-source-1",
          url: "https://open-epg.com/files/sweden4.xml.gz",
          priority: 0,
          enabled: true,
          sourceKind: "external",
          lastSyncAt: "2026-04-04T12:30:00.000Z",
          lastSyncError: null,
          lastProgramCount: 500,
          lastMatchedCount: 291,
          createdAt: "2026-04-04T10:00:00.000Z",
          updatedAt: "2026-04-04T12:30:00.000Z",
        },
      ],
    });

    render(
      <QueryClientProvider client={new QueryClient()}>
        <SettingsPage />
      </QueryClientProvider>,
    );

    expect(await screen.findByText("Provider")).toBeInTheDocument();
    expect(await screen.findByText("Arena 1")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^play$/i })).toBeInTheDocument();
    expect(
      await screen.findByDisplayValue(
        "https://open-epg.com/files/sweden4.xml.gz",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText("Sessions")).not.toBeInTheDocument();
    expect(
      screen.getByTestId("recent-channels-scroll-area"),
    ).toBeInTheDocument();
  });

  it("saves a newly added external epg source", async () => {
    mockedGetProvider.mockResolvedValue({
      id: "provider-1",
      providerType: "xtreme",
      baseUrl: "https://provider.example.com",
      username: "demo",
      outputFormat: "m3u8",
      playbackMode: "direct",
      status: "valid",
      lastValidatedAt: "2026-04-04T12:00:00.000Z",
      lastSyncAt: null,
      lastSyncError: null,
      createdAt: "2026-04-04T10:00:00.000Z",
      updatedAt: "2026-04-04T12:00:00.000Z",
      epgSources: [],
    });

    render(
      <QueryClientProvider client={new QueryClient()}>
        <SettingsPage />
      </QueryClientProvider>,
    );

    expect(await screen.findByDisplayValue("https://provider.example.com")).toBeInTheDocument();
    fireEvent.click(await screen.findByRole("button", { name: /add source/i }));
    fireEvent.change(screen.getByPlaceholderText(/guide\.xml\.gz/i), {
      target: { value: "https://www.open-epg.com/files/sweden4.xml.gz" },
    });
    fireEvent.click(screen.getByRole("button", { name: /save profile/i }));

    await waitFor(() => expect(mockedSaveProvider).toHaveBeenCalled());
    expect(mockedSaveProvider.mock.calls[0]?.[0]).toEqual(
      expect.objectContaining({
        epgSources: [
          expect.objectContaining({
            url: "https://www.open-epg.com/files/sweden4.xml.gz",
            enabled: true,
            priority: 0,
          }),
        ],
      }),
    );
  });

  it("shows sync errors from failed manual sync attempts", async () => {
    mockedGetProvider.mockResolvedValue({
      id: "provider-1",
      providerType: "xtreme",
      baseUrl: "https://provider.example.com",
      username: "demo",
      outputFormat: "m3u8",
      playbackMode: "direct",
      status: "error",
      lastValidatedAt: "2026-04-04T12:00:00.000Z",
      lastSyncAt: null,
      lastSyncError: "Previous sync failed.",
      createdAt: "2026-04-04T10:00:00.000Z",
      updatedAt: "2026-04-04T12:00:00.000Z",
      epgSources: [],
    });
    mockedTriggerProviderSync.mockRejectedValue(
      new Error("A sync is already queued or running for this provider."),
    );

    render(
      <QueryClientProvider client={new QueryClient()}>
        <SettingsPage />
      </QueryClientProvider>,
    );

    const syncButton = await screen.findByRole("button", { name: /trigger full sync/i });
    await waitFor(() => expect(syncButton).toBeEnabled());
    fireEvent.click(syncButton);
    await waitFor(() => expect(mockedTriggerProviderSync).toHaveBeenCalled());

    expect(
      await screen.findByText(/queued or running for this provider/i),
    ).toBeInTheDocument();
  });

  it("disables the sync trigger while a sync job is already active", async () => {
    mockedGetProvider.mockResolvedValue({
      id: "provider-1",
      providerType: "xtreme",
      baseUrl: "https://provider.example.com",
      username: "demo",
      outputFormat: "m3u8",
      playbackMode: "direct",
      status: "syncing",
      lastValidatedAt: "2026-04-04T12:00:00.000Z",
      lastSyncAt: null,
      lastSyncError: null,
      createdAt: "2026-04-04T10:00:00.000Z",
      updatedAt: "2026-04-05T10:00:00.000Z",
      epgSources: [],
    });
    mockedGetSyncStatus.mockResolvedValue({
      id: "sync-job-2",
      status: "running",
      jobType: "full",
      trigger: "manual",
      createdAt: "2026-04-05T10:00:00.000Z",
      startedAt: "2026-04-05T10:00:05.000Z",
      finishedAt: null,
      currentPhase: "fetching-epg",
      completedPhases: 3,
      totalPhases: 7,
      phaseMessage: "Fetching EPG feeds",
      errorMessage: null,
    });

    render(
      <QueryClientProvider client={new QueryClient()}>
        <SettingsPage />
      </QueryClientProvider>,
    );

    expect(
      await screen.findByRole("button", { name: /trigger full sync/i }),
    ).toBeDisabled();
  });
});
