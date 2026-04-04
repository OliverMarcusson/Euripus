import { invoke } from "@tauri-apps/api/core";

export function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function loadRefreshToken() {
  if (isTauriRuntime()) {
    return invoke<string | null>("load_refresh_token");
  }

  return null;
}

export async function saveRefreshToken(token: string) {
  if (isTauriRuntime()) {
    await invoke("save_refresh_token", { token });
  }
}

export async function clearRefreshToken() {
  if (isTauriRuntime()) {
    await invoke("clear_refresh_token");
  }
}
