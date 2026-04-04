import { invoke } from "@tauri-apps/api/core";

const FALLBACK_KEY = "euripus.refresh-token";

function canUseTauri() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function loadRefreshToken() {
  if (canUseTauri()) {
    return invoke<string | null>("load_refresh_token");
  }

  return window.localStorage.getItem(FALLBACK_KEY);
}

export async function saveRefreshToken(token: string) {
  if (canUseTauri()) {
    await invoke("save_refresh_token", { token });
    return;
  }

  window.localStorage.setItem(FALLBACK_KEY, token);
}

export async function clearRefreshToken() {
  if (canUseTauri()) {
    await invoke("clear_refresh_token");
    return;
  }

  window.localStorage.removeItem(FALLBACK_KEY);
}

