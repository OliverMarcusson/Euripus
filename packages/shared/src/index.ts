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
  refreshToken: string;
  expiresAt: string;
};

export type ProviderStatus = "missing" | "valid" | "error" | "syncing";

export type ProviderProfile = {
  id: string;
  providerType: "xtreme";
  baseUrl: string;
  username: string;
  outputFormat: "m3u8" | "ts";
  status: ProviderStatus;
  lastValidatedAt: string | null;
  lastSyncAt: string | null;
  lastSyncError: string | null;
  createdAt: string;
  updatedAt: string;
};

export type SyncJob = {
  id: string;
  status: "queued" | "running" | "succeeded" | "failed";
  jobType: "full" | "epg";
  createdAt: string;
  startedAt: string | null;
  finishedAt: string | null;
  errorMessage: string | null;
};

export type Channel = {
  id: string;
  name: string;
  logoUrl: string | null;
  categoryName: string | null;
  remoteStreamId: number;
  epgChannelId: string | null;
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

export type GuideResponse = {
  channels: Channel[];
  programs: Program[];
};

export type SearchResults = {
  query: string;
  channels: Channel[];
  programs: Program[];
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

export type RegisterPayload = {
  username: string;
  password: string;
};

export type LoginPayload = RegisterPayload;

export type RefreshPayload = {
  refreshToken: string;
};

export type SaveProviderPayload = {
  baseUrl: string;
  username: string;
  password: string;
  outputFormat: "m3u8" | "ts";
};

export type ValidateProviderResponse = {
  valid: boolean;
  status: ProviderStatus;
  message: string;
};
