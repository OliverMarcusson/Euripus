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
  browserPlaybackWarning?: string | null;
  epgSources: EpgSource[];
};

export type SyncJob = {
  id: string;
  status: "queued" | "running" | "succeeded" | "failed";
  jobType: "full" | "epg" | "channels";
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

export type SearchBackendStatus = {
  meilisearch: "disabled" | "indexing" | "ready";
  progressPercent?: number | null;
  indexedDocuments?: number | null;
  totalDocuments?: number | null;
};

export type SearchBackend = "meilisearch" | "postgres";

export type SearchFilterProviderOption = {
  value: string;
  countryCodes: string[];
};

export type SearchFilterOptionsResponse = {
  countries: string[];
  providers: SearchFilterProviderOption[];
};

export type AdminPatternKind = "country" | "provider" | "flag";

export type AdminMatchTarget =
  | "channel_name"
  | "category_name"
  | "program_title"
  | "channel_or_category"
  | "any_text";

export type AdminMatchMode = "prefix" | "contains" | "exact";

export type AdminPattern = {
  id: string;
  pattern: string;
};

export type AdminPatternGroup = {
  id: string;
  kind: AdminPatternKind;
  value: string;
  normalizedValue: string;
  matchTarget: AdminMatchTarget;
  matchMode: AdminMatchMode;
  priority: number;
  enabled: boolean;
  patternsText: string;
  countryCodesText: string;
  countryCodes: string[];
  patterns: AdminPattern[];
};

export type AdminPatternGroupInput = {
  kind: AdminPatternKind;
  value: string;
  matchTarget: AdminMatchTarget;
  matchMode: AdminMatchMode;
  priority: number;
  enabled: boolean;
  patternsText: string;
  countryCodesText: string;
};

export type AdminPatternGroupImportInput = {
  kind: string;
  value: string;
  matchTarget: string;
  matchMode: string;
  priority?: number;
  enabled?: boolean;
  patterns?: string[];
  patternsText?: string;
  countryCodes?: string[];
};

export type AdminPatternGroupImportRequest = {
  groups: AdminPatternGroupImportInput[];
};

export type AdminPatternGroupImportError = {
  index: number;
  field: string;
  message: string;
};

export type AdminSearchTestRequest = {
  channelName?: string | null;
  categoryName?: string | null;
  programTitle?: string | null;
};

export type AdminSearchTestResponse = {
  countryCode: string | null;
  providerName: string | null;
  isPpv: boolean;
  isVip: boolean;
  forceHasEpg: boolean;
};

export type AdminSearchQueryTestRequest = {
  query: string;
};

export type AdminSearchQueryTestResponse = {
  search: string;
  countries: string[];
  providers: string[];
  ppv: boolean | null;
  vip: boolean | null;
  requireEpg: boolean;
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
  isFavorite: boolean;
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
  backend: SearchBackend;
  items: T[];
  totalCount: number;
  nextOffset: number | null;
};

export type ChannelSearchResults = SearchResultPage<Channel>;

export type ProgramSearchResults = SearchResultPage<Program>;

export type FavoriteChannelEntry = GuideChannelEntry & {
  kind: "channel";
  order: number;
};

export type FavoriteCategoryEntry = {
  kind: "category";
  category: GuideCategorySummary;
  order: number;
};

export type FavoriteEntry = FavoriteCategoryEntry | FavoriteChannelEntry;

export type FavoriteOrderPayload = {
  categoryIds: string[];
  channelIds: string[];
};

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
  buffering: boolean;
  positionSeconds: number | null;
  durationSeconds: number | null;
  errorMessage: string | null;
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
  playbackStateStale: boolean;
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
  publicOrigin?: string | null;
  receiverCredential?: string | null;
};

export type ReceiverPlaybackStatePayload = {
  title: string | null;
  sourceKind?: PlaybackSource["kind"] | null;
  live?: boolean | null;
  catchup?: boolean | null;
  paused?: boolean | null;
  buffering?: boolean | null;
  positionSeconds?: number | null;
  durationSeconds?: number | null;
  errorMessage?: string | null;
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
  status: "queued" | "delivered" | "executing" | "succeeded" | "failed";
  sourceTitle: string;
  createdAt: string;
};

export type RemotePlaybackCommandAck = {
  status:
    | "delivered"
    | "executing"
    | "succeeded"
    | "failed"
    | "acknowledged";
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
