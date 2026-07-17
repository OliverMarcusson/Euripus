export type ApiError = {
  error: string;
  message: string;
  status: number;
};

export type User = {
  id: string;
  username: string;
  providerLocked: boolean;
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
  isActive: boolean;
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

export type AdminQualityPrefix = {
  prefix: string;
  countryCode: string;
  channelCount: number;
  categoryCount: number;
  selected: boolean;
};

export type AdminQualityPrefixSettings = {
  prefixes: AdminQualityPrefix[];
  includeCategoriesWithoutCountryPrefix: boolean;
};

export type AdminQualityPrefixSettingsInput = {
  prefixes: string[];
  includeCategoriesWithoutCountryPrefix: boolean;
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
  isPpv?: boolean;
  isPpvFavorite?: boolean;
};

export type OnDemandMediaType = "movie" | "series";

export type OnDemandCategory = {
  id: string;
  mediaType: OnDemandMediaType;
  name: string;
  titleCount: number;
  isFavorite: boolean;
};

export type OnDemandTitle = {
  id: string;
  mediaType: OnDemandMediaType;
  name: string;
  categoryId: string | null;
  categoryName: string | null;
  posterUrl: string | null;
  backdropUrl: string | null;
  plot: string | null;
  genre: string | null;
  castNames: string | null;
  director: string | null;
  releaseDate: string | null;
  rating: number | null;
  durationMinutes: number | null;
  containerExtension: string | null;
  isFavorite: boolean;
};

export type OnDemandEpisode = {
  id: string;
  seriesId: string;
  seasonNumber: number;
  episodeNumber: number;
  name: string;
  plot: string | null;
  durationMinutes: number | null;
  posterUrl: string | null;
  containerExtension: string | null;
};

export type OnDemandPage = {
  items: OnDemandTitle[];
  totalCount: number;
  nextOffset: number | null;
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

export type SportsParticipants = {
  home: string | null;
  away: string | null;
};

export type SportsAvailability = {
  market: string | null;
  providerFamily: string | null;
  providerLabel: string;
  channelName: string | null;
  watchType: string | null;
  confidence: number | null;
  source: string | null;
  searchHints: string[];
};

export type SportsWatch = {
  recommendedMarket: string | null;
  recommendedProvider: string | null;
  availabilities: SportsAvailability[];
};

export type SportsSearchMetadata = {
  queries: string[];
  keywords: string[];
};

export type SportsEvent = {
  id: string;
  sport: string;
  competition: string;
  title: string;
  startTime: string;
  endTime: string | null;
  status: string;
  venue: string | null;
  roundLabel: string | null;
  participants: SportsParticipants | null;
  source: string | null;
  sourceUrl: string | null;
  watch: SportsWatch;
  searchMetadata: SportsSearchMetadata;
};

export type SportsEventListResponse = {
  count: number;
  events: SportsEvent[];
};

export type SportsCompetitionResponse = {
  competition: string;
  events: SportsEvent[];
};

export type SportsProvider = {
  family: string;
  market: string;
  aliases: string[];
};

export type SportsProviderCatalogResponse = {
  count: number;
  providers: SportsProvider[];
};

export type GoogleCalendarConnectionStatus = {
  configured: boolean;
  connected: boolean;
  needsReauthorization: boolean;
  selectedCalendarId: string | null;
  selectedCalendarName: string | null;
};

export type GoogleCalendarInfo = {
  id: string;
  summary: string;
  primary: boolean;
  accessRole: string;
};

export type GoogleCalendarConnectResponse = {
  authorizationUrl: string;
};

export type GoogleCalendarSelection = {
  calendarId: string;
};

export type SportsCalendarEventResponse = {
  googleEventId: string;
  htmlLink: string | null;
  created: boolean;
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

export type AiPpvSearchResult = {
  channel: Channel;
  program: Program | null;
  confidence: number;
  reason: string;
  matchedTerms: string[];
};

export type AiPpvSearchResponse = {
  query: string;
  backend: "openrouter" | "local_fallback";
  items: AiPpvSearchResult[];
  message?: string | null;
};

export type AiPpvSearchRequest = {
  query: string;
  limit?: number;
};

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

export type PpvFavoriteOrderPayload = {
  channelIds: string[];
};

export type RecentChannel = {
  channel: Channel;
  lastPlayedAt: string;
};

export type PlaybackSource = {
  kind: "hls" | "mpegts" | "progressive" | "unsupported";
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
  status: "delivered" | "executing" | "succeeded" | "failed" | "acknowledged";
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
  id?: string;
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

export type AdminRestrictedAccount = {
  id: string;
  username: string;
  createdAt: string;
  provider: ProviderProfile | null;
};

export type AdminRestrictedAccountInput = {
  username: string;
  password?: string;
  provider: Omit<SaveProviderPayload, "id">;
};

export type AdminRestrictedAccountSummary = {
  id: string;
  username: string;
  createdAt: string;
  providerId: string | null;
  providerStatus: ProviderStatus | null;
  providerLastSyncAt: string | null;
  providerLastSyncError: string | null;
  providerBaseUrl: string | null;
  providerUsername: string | null;
  providerOutputFormat: "m3u8" | "ts" | null;
  providerPlaybackMode: "direct" | "relay" | null;
  providerEpgUrls: string[];
};

export type ServerNetworkStatus = {
  serverStatus: "online";
  vpnActive: boolean;
  vpnProvider: string | null;
  publicIp: string | null;
  publicIpCheckedAt: string;
  publicIpError: string | null;
};
