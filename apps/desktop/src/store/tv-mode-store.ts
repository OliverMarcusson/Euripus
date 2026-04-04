import { create } from "zustand";

const TV_MODE_STORAGE_KEY = "euripus-tv-mode-preference";

export type TvModePreference = "auto" | "on" | "off";

type TvModeState = {
  preference: TvModePreference;
  isTvMode: boolean;
  setPreference: (preference: TvModePreference) => void;
  syncEnvironment: () => void;
};

function isTvModePreference(value: string | null): value is TvModePreference {
  return value === "auto" || value === "on" || value === "off";
}

function readStoredPreference(): TvModePreference {
  if (typeof window === "undefined") {
    return "auto";
  }

  const storedPreference = window.localStorage.getItem(TV_MODE_STORAGE_KEY);
  return isTvModePreference(storedPreference) ? storedPreference : "auto";
}

function writeStoredPreference(preference: TvModePreference) {
  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.setItem(TV_MODE_STORAGE_KEY, preference);
}

function detectTvEnvironment() {
  if (typeof window === "undefined") {
    return false;
  }

  const userAgent = window.navigator.userAgent.toLowerCase();
  const looksLikeAndroidTv =
    userAgent.includes("android tv") ||
    userAgent.includes("googletv") ||
    userAgent.includes("google tv") ||
    userAgent.includes("bravia") ||
    userAgent.includes("shield") ||
    userAgent.includes("aft");
  const looksLikeLargeScreen = window.innerWidth >= 1280 && window.innerHeight >= 720;

  return looksLikeAndroidTv && looksLikeLargeScreen;
}

function resolveTvMode(preference: TvModePreference) {
  if (preference === "on") {
    return true;
  }

  if (preference === "off") {
    return false;
  }

  return detectTvEnvironment();
}

function applyTvMode(isTvMode: boolean) {
  if (typeof document === "undefined") {
    return;
  }

  document.documentElement.classList.toggle("tv-mode", isTvMode);
  document.documentElement.dataset.tvMode = isTvMode ? "true" : "false";
  document.body.classList.toggle("tv-mode", isTvMode);
}

const initialPreference = readStoredPreference();
const initialTvMode = resolveTvMode(initialPreference);

applyTvMode(initialTvMode);

export const useTvModeStore = create<TvModeState>((set, get) => ({
  preference: initialPreference,
  isTvMode: initialTvMode,
  setPreference: (preference) => {
    writeStoredPreference(preference);
    const isTvMode = resolveTvMode(preference);
    applyTvMode(isTvMode);
    set({ preference, isTvMode });
  },
  syncEnvironment: () => {
    const isTvMode = resolveTvMode(get().preference);
    applyTvMode(isTvMode);
    set({ isTvMode });
  },
}));
