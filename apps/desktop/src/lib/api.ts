import type {
  ApiError,
  AuthSession,
  ChannelSearchResults,
  Channel,
  GuidePreferences,
  GuideCategoryResponse,
  GuideResponse,
  LoginPayload,
  PlaybackSource,
  Program,
  ProviderProfile,
  RecentChannel,
  RefreshPayload,
  RegisterPayload,
  SaveProviderPayload,
  ProgramSearchResults,
  Session,
  SyncJob,
  User,
  ValidateProviderResponse,
} from "@euripus/shared";
import { clearRefreshToken, loadRefreshToken, saveRefreshToken } from "@/lib/tauri";
import { useAuthStore } from "@/store/auth-store";

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? "http://127.0.0.1:8080";

async function request<T>(path: string, init: RequestInit = {}, retry = true): Promise<T> {
  const { accessToken, setSession, clearSession } = useAuthStore.getState();
  const headers = new Headers(init.headers);
  headers.set("Content-Type", "application/json");

  if (accessToken) {
    headers.set("Authorization", `Bearer ${accessToken}`);
  }

  const response = await fetch(`${API_BASE_URL}${path}`, {
    ...init,
    headers,
  });

  if (response.status === 401 && retry) {
    const refreshToken = await loadRefreshToken();
    if (refreshToken) {
      try {
        const nextSession = await refresh({ refreshToken });
        setSession(nextSession);
        await saveRefreshToken(nextSession.refreshToken);
        return request<T>(path, init, false);
      } catch {
        await clearRefreshToken();
        clearSession();
      }
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
  const session = await request<AuthSession>("/auth/register", {
    method: "POST",
    body: JSON.stringify(payload),
  });
  await saveRefreshToken(session.refreshToken);
  return session;
}

export async function login(payload: LoginPayload) {
  const session = await request<AuthSession>("/auth/login", {
    method: "POST",
    body: JSON.stringify(payload),
  });
  await saveRefreshToken(session.refreshToken);
  return session;
}

export async function refresh(payload: RefreshPayload) {
  return request<AuthSession>("/auth/refresh", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export async function logout() {
  const refreshToken = await loadRefreshToken();
  if (refreshToken) {
    await request<void>("/auth/logout", {
      method: "POST",
      body: JSON.stringify({ refreshToken }),
    }).catch(() => undefined);
  }
  await clearRefreshToken();
}

export function getCurrentUser() {
  return request<User>("/me");
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
