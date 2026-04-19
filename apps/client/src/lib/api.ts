import type {
  AdminPatternGroup,
  AdminPatternGroupInput,
  AdminPatternGroupImportError,
  AdminPatternGroupImportRequest,
  AdminSearchQueryTestRequest,
  AdminSearchQueryTestResponse,
  AdminSearchTestRequest,
  AdminSearchTestResponse,
  ApiError,
  AuthSession,
  ChannelSearchResults,
  Channel,
  FavoriteChannelEntry,
  FavoriteEntry,
  FavoriteOrderPayload,
  GuidePreferences,
  GuideCategoryResponse,
  GuideResponse,
  LoginPayload,
  PlaybackSource,
  Program,
  ProgramSearchResults,
  PairReceiverPayload,
  PpvFavoriteOrderPayload,
  ProviderProfile,
  RecentChannel,
  RegisterPayload,
  SaveProviderPayload,
  Session,
  SportsCompetitionResponse,
  SportsEvent,
  SportsEventListResponse,
  SportsProviderCatalogResponse,
  ReceiverDevice,
  ReceiverPairingCode,
  ReceiverPlaybackStatePayload,
  ReceiverSession,
  ReceiverSessionPayload,
  RemoteCommandAck,
  RemoteControllerTarget,
  RemotePlaybackCommand,
  SearchFilterOptionsResponse,
  SearchBackendStatus,
  ServerNetworkStatus,
  SyncJob,
  User,
  ValidateProviderResponse,
} from "@euripus/shared";
import { useAuthStore } from "@/store/auth-store";

export const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? "/api";
const CSRF_COOKIE_NAME = "euripus.csrf";
const ADMIN_CSRF_COOKIE_NAME = "euripus.admin.csrf";

type RequestOptions = {
  retry?: boolean;
  includeCsrf?: boolean;
};

type ApiErrorPayload = ApiError & {
  details?: unknown;
};

export class ApiRequestError extends Error {
  status: number;
  code: string;
  details?: unknown;

  constructor(payload: ApiErrorPayload) {
    super(payload.message);
    this.name = "ApiRequestError";
    this.status = payload.status;
    this.code = payload.error;
    this.details = payload.details;
  }
}

function readCookie(name: string) {
  if (typeof document === "undefined") {
    return null;
  }

  const cookies = document.cookie ? document.cookie.split(/;\s*/) : [];
  const prefix = `${name}=`;
  for (const cookie of cookies) {
    if (cookie.startsWith(prefix)) {
      return decodeURIComponent(cookie.slice(prefix.length));
    }
  }

  return null;
}

function withJsonHeaders(headers: Headers) {
  if (!headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }
}

async function request<T>(
  path: string,
  init: RequestInit = {},
  { retry = true, includeCsrf = false }: RequestOptions = {},
): Promise<T> {
  const { accessToken, setSession, clearSession } = useAuthStore.getState();
  const headers = new Headers(init.headers);
  withJsonHeaders(headers);

  if (accessToken) {
    headers.set("Authorization", `Bearer ${accessToken}`);
  }

  if (includeCsrf) {
    const csrfToken = readCookie(CSRF_COOKIE_NAME);
    if (csrfToken) {
      headers.set("X-CSRF-Token", csrfToken);
    }
  }

  const response = await fetch(`${API_BASE_URL}${path}`, {
    ...init,
    headers,
    credentials: "include",
  });

  if (response.status === 401 && retry) {
    try {
      const nextSession = await refresh();
      setSession(nextSession);
      return request<T>(path, init, { retry: false, includeCsrf });
    } catch {
      clearSession();
    }
  }

  if (!response.ok) {
    const fallback: ApiError = {
      error: "request_failed",
      message: response.statusText,
      status: response.status,
    };
    const payload = (await response.json().catch(() => fallback)) as ApiError;
    throw new Error(payload.message);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return (await response.json()) as T;
}

async function adminRequest<T>(
  path: string,
  init: RequestInit = {},
  { includeCsrf = false }: Pick<RequestOptions, "includeCsrf"> = {},
): Promise<T> {
  const headers = new Headers(init.headers);
  withJsonHeaders(headers);

  if (includeCsrf) {
    const csrfToken = readCookie(ADMIN_CSRF_COOKIE_NAME);
    if (csrfToken) {
      headers.set("X-CSRF-Token", csrfToken);
    }
  }

  const response = await fetch(`${API_BASE_URL}${path}`, {
    ...init,
    headers,
    credentials: "include",
  });

  if (!response.ok) {
    const fallback: ApiErrorPayload = {
      error: "request_failed",
      message: response.statusText,
      status: response.status,
    };
    const payload = (await response.json().catch(() => fallback)) as ApiErrorPayload;
    throw new ApiRequestError(payload);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return (await response.json()) as T;
}

export async function register(payload: RegisterPayload) {
  return request<AuthSession>(
    "/auth/register",
    {
      method: "POST",
      body: JSON.stringify(payload),
    },
    { retry: false },
  );
}

export async function login(payload: LoginPayload) {
  return request<AuthSession>(
    "/auth/login",
    {
      method: "POST",
      body: JSON.stringify(payload),
    },
    { retry: false },
  );
}

export async function refresh() {
  return request<AuthSession>(
    "/auth/refresh",
    {
      method: "POST",
    },
    { retry: false, includeCsrf: true },
  );
}

export async function logout() {
  await request<void>(
    "/auth/logout",
    {
      method: "POST",
    },
    { retry: false, includeCsrf: true },
  ).catch(() => undefined);
}

export function getCurrentUser() {
  return request<User>("/me");
}

export function getServerNetworkStatus() {
  return request<ServerNetworkStatus>("/server/network");
}

export function getSearchBackendStatus() {
  return request<SearchBackendStatus>("/search/status");
}

export function getSearchFilterOptions() {
  return request<SearchFilterOptionsResponse>("/search/filter-options");
}

export function getAdminPatternGroups() {
  return adminRequest<AdminPatternGroup[]>("/admin/search/pattern-groups");
}

export function adminLogin(password: string) {
  return adminRequest<void>(
    "/admin/auth/login",
    {
      method: "POST",
      body: JSON.stringify({ password }),
    },
    { includeCsrf: false },
  );
}

export function adminLogout() {
  return adminRequest<void>(
    "/admin/auth/logout",
    {
      method: "POST",
    },
    { includeCsrf: true },
  );
}

export function createAdminPatternGroup(payload: AdminPatternGroupInput) {
  return adminRequest<AdminPatternGroup>(
    "/admin/search/pattern-groups",
    {
      method: "POST",
      body: JSON.stringify(payload),
    },
    { includeCsrf: true },
  );
}

export function importAdminPatternGroups(payload: AdminPatternGroupImportRequest) {
  return adminRequest<AdminPatternGroup[]>(
    "/admin/search/pattern-group-import",
    {
      method: "POST",
      body: JSON.stringify(payload),
    },
    { includeCsrf: true },
  );
}

export function updateAdminPatternGroup(
  id: string,
  payload: AdminPatternGroupInput,
) {
  return adminRequest<AdminPatternGroup>(
    `/admin/search/pattern-groups/${id}`,
    {
      method: "PUT",
      body: JSON.stringify(payload),
    },
    { includeCsrf: true },
  );
}

export function deleteAdminPatternGroup(id: string) {
  return adminRequest<void>(
    `/admin/search/pattern-groups/${id}`,
    {
      method: "DELETE",
    },
    { includeCsrf: true },
  );
}

export function deleteAllAdminPatternGroups() {
  return adminRequest<void>(
    "/admin/search/pattern-groups",
    {
      method: "DELETE",
    },
    { includeCsrf: true },
  );
}

export function testAdminSearchPatterns(payload: AdminSearchTestRequest) {
  return adminRequest<AdminSearchTestResponse>(
    "/admin/search/test",
    {
      method: "POST",
      body: JSON.stringify(payload),
    },
    { includeCsrf: true },
  );
}

export function testAdminSearchQuery(payload: AdminSearchQueryTestRequest) {
  return adminRequest<AdminSearchQueryTestResponse>(
    "/admin/search/test-query",
    {
      method: "POST",
      body: JSON.stringify(payload),
    },
    { includeCsrf: true },
  );
}

export function getAdminImportErrors(error: unknown): AdminPatternGroupImportError[] {
  if (error instanceof ApiRequestError && Array.isArray(error.details)) {
    return error.details.filter(isAdminImportError);
  }

  return [];
}

function isAdminImportError(value: unknown): value is AdminPatternGroupImportError {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { index?: unknown }).index === "number" &&
    typeof (value as { field?: unknown }).field === "string" &&
    typeof (value as { message?: unknown }).message === "string"
  );
}

export function getSessions() {
  return request<Session[]>("/sessions");
}

export function revokeSession(id: string) {
  return request<void>(`/sessions/${id}`, { method: "DELETE" });
}

export function getProviders() {
  return request<ProviderProfile[]>("/providers");
}

export function validateProvider(payload: SaveProviderPayload) {
  return request<ValidateProviderResponse>("/providers/validate", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export function saveProvider(payload: SaveProviderPayload) {
  return request<ProviderProfile>("/providers/xtreme", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export function deleteProvider(providerId: string) {
  return request<void>(`/providers/${providerId}`, { method: "DELETE" });
}

export function triggerProviderSync(providerId: string) {
  return request<SyncJob>(`/providers/${providerId}/sync`, { method: "POST" });
}

export function getSyncStatus(providerId: string) {
  return request<SyncJob | null>(`/providers/${providerId}/sync-status`);
}

export function getChannels() {
  return request<Channel[]>("/channels");
}

export function getChannelGuide(id: string) {
  return request<Program[]>(`/guide/channel/${id}`);
}

export function getGuide(withEpgOnly = false) {
  const params = new URLSearchParams();
  if (withEpgOnly) {
    params.set("withEpgOnly", "true");
  }

  const suffix = params.size ? `?${params.toString()}` : "";
  return request<GuideResponse>(`/guide${suffix}`);
}

export function getGuidePreferences() {
  return request<GuidePreferences>("/guide/preferences");
}

export function saveGuidePreferences(payload: GuidePreferences) {
  return request<GuidePreferences>("/guide/preferences", {
    method: "PUT",
    body: JSON.stringify(payload),
  });
}

export function getGuideCategory(
  categoryId: string,
  offset = 0,
  limit = 40,
  withEpgOnly = false,
) {
  const params = new URLSearchParams({
    offset: offset.toString(),
    limit: limit.toString(),
  });
  if (withEpgOnly) {
    params.set("withEpgOnly", "true");
  }
  return request<GuideCategoryResponse>(
    `/guide/category/${encodeURIComponent(categoryId)}?${params.toString()}`,
  );
}

export function getSportsLiveEvents() {
  return request<SportsEventListResponse>("/sports/live");
}

export function getSportsTodayEvents() {
  return request<SportsEventListResponse>("/sports/today");
}

export function getSportsUpcomingEvents(hours = 72) {
  const params = new URLSearchParams({ hours: hours.toString() });
  return request<SportsEventListResponse>(`/sports/upcoming?${params.toString()}`);
}

export function getSportsEvent(id: string) {
  return request<SportsEvent>(`/sports/events/${encodeURIComponent(id)}`);
}

export function getSportsCompetition(slug: string) {
  return request<SportsCompetitionResponse>(
    `/sports/competitions/${encodeURIComponent(slug)}`,
  );
}

export function getSportsProviders() {
  return request<SportsProviderCatalogResponse>("/sports/providers");
}

export function searchChannels(query: string, offset = 0, limit = 30) {
  const params = new URLSearchParams({
    q: query,
    offset: offset.toString(),
    limit: limit.toString(),
  });
  return request<ChannelSearchResults>(`/search/channels?${params.toString()}`);
}

export function searchPrograms(query: string, offset = 0, limit = 30) {
  const params = new URLSearchParams({
    q: query,
    offset: offset.toString(),
    limit: limit.toString(),
  });
  return request<ProgramSearchResults>(`/search/programs?${params.toString()}`);
}

export function getFavorites() {
  return request<FavoriteEntry[]>("/favorites");
}

export function addFavorite(channelId: string) {
  return request<void>(`/favorites/${channelId}`, { method: "POST" });
}

export function removeFavorite(channelId: string) {
  return request<void>(`/favorites/${channelId}`, { method: "DELETE" });
}

export function getPpvFavorites() {
  return request<FavoriteChannelEntry[]>("/favorites/ppv");
}

export function addPpvFavorite(channelId: string) {
  return request<void>(`/favorites/ppv/${channelId}`, { method: "POST" });
}

export function removePpvFavorite(channelId: string) {
  return request<void>(`/favorites/ppv/${channelId}`, { method: "DELETE" });
}

export function addCategoryFavorite(categoryId: string) {
  return request<void>(`/favorites/categories/${categoryId}`, { method: "POST" });
}

export function removeCategoryFavorite(categoryId: string) {
  return request<void>(`/favorites/categories/${categoryId}`, { method: "DELETE" });
}

export function reorderFavorites(payload: FavoriteOrderPayload) {
  return request<void>("/favorites/order", {
    method: "PUT",
    body: JSON.stringify(payload),
  });
}

export function reorderPpvFavorites(payload: PpvFavoriteOrderPayload) {
  return request<void>("/favorites/ppv/order", {
    method: "PUT",
    body: JSON.stringify(payload),
  });
}

export function getRecents() {
  return request<RecentChannel[]>("/recents");
}

export function startChannelPlayback(channelId: string) {
  return request<PlaybackSource>(`/playback/channel/${channelId}`, {
    method: "POST",
  });
}

export function startProgramPlayback(programId: string) {
  return request<PlaybackSource>(`/playback/program/${programId}`, {
    method: "POST",
  });
}

export function getRemoteReceivers() {
  return request<ReceiverDevice[]>("/remote/receivers");
}

export function pairReceiver(payload: PairReceiverPayload) {
  return request<ReceiverDevice>("/remote/pair", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export function unpairReceiver(deviceId: string) {
  return request<void>(`/remote/receivers/${deviceId}`, {
    method: "DELETE",
  });
}

export function getRemoteControllerTarget() {
  return request<RemoteControllerTarget>("/remote/controller/target");
}

export function selectRemoteControllerTarget(deviceId: string) {
  return request<RemoteControllerTarget>("/remote/controller/target", {
    method: "POST",
    body: JSON.stringify({ deviceId }),
  });
}

export function clearRemoteControllerTarget() {
  return request<void>("/remote/controller/target", {
    method: "DELETE",
  });
}

export function startRemoteChannelPlayback(channelId: string) {
  return request<RemotePlaybackCommand>(`/remote/play/channel/${channelId}`, {
    method: "POST",
  });
}

export function startRemoteProgramPlayback(programId: string) {
  return request<RemotePlaybackCommand>(`/remote/play/program/${programId}`, {
    method: "POST",
  });
}

export function pauseRemotePlayback() {
  return request<RemotePlaybackCommand>("/remote/command/pause", {
    method: "POST",
  });
}

export function resumeRemotePlayback() {
  return request<RemotePlaybackCommand>("/remote/command/play", {
    method: "POST",
  });
}

export function stopRemotePlayback() {
  return request<RemotePlaybackCommand>("/remote/command/stop", {
    method: "POST",
  });
}

export function seekRemotePlayback(positionSeconds: number) {
  return request<RemotePlaybackCommand>("/remote/command/seek", {
    method: "POST",
    body: JSON.stringify({ positionSeconds }),
  });
}

export async function createReceiverSession(payload: ReceiverSessionPayload) {
  const response = await fetch(`${API_BASE_URL}/receiver/session`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    credentials: "include",
    body: JSON.stringify(payload),
  });
  if (!response.ok) {
    throw new Error("Unable to start receiver session.");
  }
  return (await response.json()) as ReceiverSession;
}

async function receiverRequest<T>(
  path: string,
  sessionToken: string,
  init: RequestInit = {},
): Promise<T> {
  const headers = new Headers(init.headers);
  withJsonHeaders(headers);
  headers.set("Authorization", `Bearer ${sessionToken}`);
  const response = await fetch(`${API_BASE_URL}${path}`, {
    ...init,
    headers,
  });
  if (!response.ok) {
    const fallback: ApiErrorPayload = {
      error: "request_failed",
      message: response.statusText,
      status: response.status,
    };
    const payload = (await response.json().catch(() => fallback)) as ApiErrorPayload;
    throw new ApiRequestError(payload);
  }
  if (response.status === 204) {
    return undefined as T;
  }
  return (await response.json()) as T;
}

export function issueReceiverPairingCode(sessionToken: string) {
  return receiverRequest<ReceiverPairingCode>(
    "/receiver/pairing-code",
    sessionToken,
    { method: "POST" },
  );
}

export function heartbeatReceiver(sessionToken: string) {
  return receiverRequest<void>("/receiver/heartbeat", sessionToken, {
    method: "POST",
  });
}

export function updateReceiverPlaybackState(
  sessionToken: string,
  payload: ReceiverPlaybackStatePayload,
) {
  return receiverRequest<void>("/receiver/playback-state", sessionToken, {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export function acknowledgeReceiverCommand(
  sessionToken: string,
  commandId: string,
  payload: RemoteCommandAck,
) {
  return receiverRequest<void>(
    `/receiver/commands/${commandId}/ack`,
    sessionToken,
    {
      method: "POST",
      body: JSON.stringify(payload),
    },
  );
}
