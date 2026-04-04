import type {
  ApiError,
  AuthSession,
  ChannelSearchResults,
  Channel,
  DesktopAuthSession,
  GuidePreferences,
  GuideCategoryResponse,
  GuideResponse,
  LoginPayload,
  PlaybackSource,
  Program,
  ProviderProfile,
  RecentChannel,
  RegisterPayload,
  SaveProviderPayload,
  ProgramSearchResults,
  Session,
  ServerNetworkStatus,
  SyncJob,
  User,
  ValidateProviderResponse,
} from "@euripus/shared";
import { clearRefreshToken, isTauriRuntime, loadRefreshToken, saveRefreshToken } from "@/lib/tauri";
import { useAuthStore } from "@/store/auth-store";

const DESKTOP_RUNTIME = isTauriRuntime();
const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? (DESKTOP_RUNTIME ? "http://127.0.0.1:8080" : "/api");
const CSRF_COOKIE_NAME = "euripus.csrf";

type RequestOptions = {
  retry?: boolean;
  includeCsrf?: boolean;
};

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

  if (includeCsrf && !DESKTOP_RUNTIME) {
    const csrfToken = readCookie(CSRF_COOKIE_NAME);
    if (csrfToken) {
      headers.set("X-CSRF-Token", csrfToken);
    }
  }

  const response = await fetch(`${API_BASE_URL}${path}`, {
    ...init,
    headers,
    credentials: DESKTOP_RUNTIME ? init.credentials : "include",
  });

  if (response.status === 401 && retry) {
    try {
      const nextSession = await refresh();
      setSession(nextSession);
      return request<T>(path, init, { retry: false, includeCsrf });
    } catch {
      await clearRefreshToken();
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

export async function register(payload: RegisterPayload) {
  const session = DESKTOP_RUNTIME
    ? await request<DesktopAuthSession>(
        "/auth/register",
        {
          method: "POST",
          body: JSON.stringify(payload),
        },
        { retry: false },
      )
    : await request<AuthSession>(
        "/auth/register",
        {
          method: "POST",
          body: JSON.stringify(payload),
        },
        { retry: false },
      );

  if (DESKTOP_RUNTIME) {
    await saveRefreshToken((session as DesktopAuthSession).refreshToken);
  }

  return session;
}

export async function login(payload: LoginPayload) {
  const session = DESKTOP_RUNTIME
    ? await request<DesktopAuthSession>(
        "/auth/login",
        {
          method: "POST",
          body: JSON.stringify(payload),
        },
        { retry: false },
      )
    : await request<AuthSession>(
        "/auth/login",
        {
          method: "POST",
          body: JSON.stringify(payload),
        },
        { retry: false },
      );

  if (DESKTOP_RUNTIME) {
    await saveRefreshToken((session as DesktopAuthSession).refreshToken);
  }

  return session;
}

export async function refresh() {
  if (DESKTOP_RUNTIME) {
    const refreshToken = await loadRefreshToken();
    if (!refreshToken) {
      throw new Error("No saved session.");
    }

    const session = await request<DesktopAuthSession>(
      "/auth/refresh",
      {
        method: "POST",
        body: JSON.stringify({ refreshToken }),
      },
      { retry: false },
    );
    await saveRefreshToken(session.refreshToken);
    return session;
  }

  return request<AuthSession>(
    "/auth/refresh",
    {
      method: "POST",
    },
    { retry: false, includeCsrf: true },
  );
}

export async function logout() {
  if (DESKTOP_RUNTIME) {
    const refreshToken = await loadRefreshToken();
    if (refreshToken) {
      await request<void>(
        "/auth/logout",
        {
          method: "POST",
          body: JSON.stringify({ refreshToken }),
        },
        { retry: false },
      ).catch(() => undefined);
    }

    await clearRefreshToken();
    return;
  }

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

export function getSessions() {
  return request<Session[]>("/sessions");
}

export function revokeSession(id: string) {
  return request<void>(`/sessions/${id}`, { method: "DELETE" });
}

export function getProvider() {
  return request<ProviderProfile | null>("/provider");
}

export function validateProvider(payload: SaveProviderPayload) {
  return request<ValidateProviderResponse>("/provider/validate", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export function saveProvider(payload: SaveProviderPayload) {
  return request<ProviderProfile>("/provider/xtreme", {
    method: "PUT",
    body: JSON.stringify(payload),
  });
}

export function triggerProviderSync() {
  return request<SyncJob>("/provider/sync", { method: "POST" });
}

export function getSyncStatus() {
  return request<SyncJob | null>("/provider/sync-status");
}

export function getChannels() {
  return request<Channel[]>("/channels");
}

export function getChannelGuide(id: string) {
  return request<Program[]>(`/guide/channel/${id}`);
}

export function getGuide() {
  return request<GuideResponse>("/guide");
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

export function getGuideCategory(categoryId: string, offset = 0, limit = 40) {
  const params = new URLSearchParams({
    offset: offset.toString(),
    limit: limit.toString(),
  });
  return request<GuideCategoryResponse>(`/guide/category/${encodeURIComponent(categoryId)}?${params.toString()}`);
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
  return request<Channel[]>("/favorites");
}

export function addFavorite(channelId: string) {
  return request<void>(`/favorites/${channelId}`, { method: "POST" });
}

export function removeFavorite(channelId: string) {
  return request<void>(`/favorites/${channelId}`, { method: "DELETE" });
}

export function getRecents() {
  return request<RecentChannel[]>("/recents");
}

export function startChannelPlayback(channelId: string) {
  return request<PlaybackSource>(`/playback/channel/${channelId}`, { method: "POST" });
}

export function startProgramPlayback(programId: string) {
  return request<PlaybackSource>(`/playback/program/${programId}`, { method: "POST" });
}
