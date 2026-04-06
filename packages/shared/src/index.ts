export type ApiError = {
  error: string;
  message: string;
  status: number;
};

export type User = {
  id: string;
  username: string;
  createdAt: string;
};

export type Session = {
  id: string;
  createdAt: string;
  expiresAt: string;
  lastUsedAt: string | null;
  userAgent: string | null;
  current: boolean;
};

export type AuthSession = {
  user: User;
  accessToken: string;
  expiresAt: string;
};

export type ProviderStatus = "missing" | "valid" | "error" | "syncing";

export type EpgSource = {
  id: string;
  url: string;
  priority: number;
  enabled: boolean;
  sourceKind: string;
  lastSyncAt: string | null;
  lastSyncError: string | null;
  lastProgramCount: number | null;
  lastMatchedCount: number | null;
  createdAt: string;
  updatedAt: string;
};

export type SaveEpgSourceInput = {
  id?: string;
  url: string;
  priority: number;
  enabled: boolean;
};

export type ProviderProfile = {
  id: string;
  providerType: "xtreme";
  baseUrl: string;
  username: string;
  outputFormat: "m3u8" | "ts";
  playbackMode: "direct" | "relay";
  status: ProviderStatus;
  lastValidatedAt: string | null;
  lastSyncAt: string | null;
  lastSyncError: string | null;
  createdAt: string;
  updatedAt: string;
  epgSources: EpgSource[];
};

export type SyncJob = {
  id: string;
  status: "queued" | "running" | "succeeded" | "failed";
  jobType: "full" | "epg";
  trigger: "manual" | "scheduled";
  createdAt: string;
  startedAt: string | null;
  finishedAt: string | null;
  currentPhase: string | null;
  completedPhases: number;
  totalPhases: number;
  phaseMessage: string | null;
  errorMessage: string | null;
};

export type Channel = {
  id: string;
  name: string;
  logoUrl: string | null;
  categoryName: string | null;
  remoteStreamId: number;
  epgChannelId: string | null;
  hasEpg: boolean;
  hasCatchup: boolean;
  archiveDurationHours: number | null;
  streamExtension: string | null;
  isFavorite: boolean;
};

export type Program = {
  id: string;
  channelId: string | null;
  channelName: string | null;
  title: string;
  description: string | null;
  startAt: string;
  endAt: string;
  canCatchup: boolean;
};

export type GuideCategorySummary = {
  id: string;
  name: string;
  channelCount: number;
  liveNowCount: number;
};

export type GuideChannelEntry = {
  channel: Channel;
  program: Program | null;
};

export type GuideResponse = {
  categories: GuideCategorySummary[];
};

export type GuidePreferences = {
  includedCategoryIds: string[];
};

export type GuideCategoryResponse = {
  category: GuideCategorySummary;
  entries: GuideChannelEntry[];
  totalCount: number;
  nextOffset: number | null;
};

export type SearchResultPage<T> = {
  query: string;
  items: T[];
  totalCount: number;
  nextOffset: number | null;
};

export type ChannelSearchResults = SearchResultPage<Channel>;

export type ProgramSearchResults = SearchResultPage<Program>;

export type FavoriteEntry = GuideChannelEntry;

export type RecentChannel = {
  channel: Channel;
  lastPlayedAt: string;
};

export type PlaybackSource = {
  kind: "hls" | "mpegts" | "unsupported";
  url: string;
  headers: Record<string, string>;
  live: boolean;
  catchup: boolean;
  expiresAt: string | null;
  unsupportedReason: string | null;
  title: string;
};

export type ReceiverPlaybackState = {
  title: string;
  sourceKind: PlaybackSource["kind"];
  live: boolean;
  catchup: boolean;
  updatedAt: string;
  paused: boolean;
  positionSeconds: number | null;
  durationSeconds: number | null;
};

export type ReceiverDevice = {
  id: string;
  name: string;
  platform: string;
  formFactorHint: string | null;
  appKind: string;
  remembered: boolean;
  online: boolean;
  currentController: boolean;
  lastSeenAt: string;
  updatedAt: string;
  currentPlayback: ReceiverPlaybackState | null;
};

export type ReceiverSession = {
  sessionToken: string;
  expiresAt: string;
  receiverCredential: string | null;
  device: ReceiverDevice;
  pairingCode: string | null;
  paired: boolean;
};

export type PairReceiverPayload = {
  code: string;
  rememberDevice: boolean;
  name?: string;
};

export type ReceiverSessionPayload = {
  deviceKey: string;
  name: string;
  platform: string;
  formFactorHint: string | null;
  appKind: string;
  receiverCredential?: string | null;
};

export type ReceiverPlaybackStatePayload = {
  title: string | null;
  sourceKind?: PlaybackSource["kind"] | null;
  live?: boolean | null;
  catchup?: boolean | null;
  paused?: boolean | null;
  positionSeconds?: number | null;
  durationSeconds?: number | null;
};

export type RemoteControllerTarget = {
  device: ReceiverDevice;
  selectedAt: string;
} | null;

export type RemotePlaybackCommand = {
  id: string;
  targetDeviceId: string;
  targetDeviceName: string;
  commandType: string;
  status: "queued" | "delivered" | "acknowledged" | "failed";
  sourceTitle: string;
  createdAt: string;
};

export type RemotePlaybackCommandAck = {
  status: "delivered" | "acknowledged" | "failed";
  errorMessage?: string | null;
};

export type RemoteCommandAck = RemotePlaybackCommandAck;

export type ReceiverPairingCode = {
  code: string;
  expiresAt: string;
  device: ReceiverDevice;
};

export type RegisterPayload = {
  username: string;
  password: string;
};

export type LoginPayload = RegisterPayload;

export type SaveProviderPayload = {
  baseUrl: string;
  username: string;
  password: string;
  outputFormat: "m3u8" | "ts";
  playbackMode: "direct" | "relay";
  epgSources: SaveEpgSourceInput[];
};

export type ValidateProviderResponse = {
  valid: boolean;
  status: ProviderStatus;
  message: string;
};

export type ServerNetworkStatus = {
  serverStatus: "online";
  vpnActive: boolean;
  vpnProvider: string | null;
  publicIp: string | null;
  publicIpCheckedAt: string;
  publicIpError: string | null;
};
