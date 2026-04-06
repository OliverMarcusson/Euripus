import { create } from "zustand";

const THEME_STORAGE_KEY = "euripus-theme-preference";
const THEME_MEDIA_QUERY = "(prefers-color-scheme: dark)";

export type ThemePreference = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

type ThemeState = {
  preference: ThemePreference;
  resolvedTheme: ResolvedTheme;
  setPreference: (preference: ThemePreference) => void;
  syncResolvedTheme: () => void;
};

function isThemePreference(value: string | null): value is ThemePreference {
  return value === "system" || value === "light" || value === "dark";
}

function getSystemTheme(): ResolvedTheme {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return "light";
  }

  return window.matchMedia(THEME_MEDIA_QUERY).matches ? "dark" : "light";
}

function resolveTheme(preference: ThemePreference): ResolvedTheme {
  return preference === "system" ? getSystemTheme() : preference;
}

function applyTheme(resolvedTheme: ResolvedTheme) {
  if (typeof document === "undefined") {
    return;
  }

  document.documentElement.classList.toggle("dark", resolvedTheme === "dark");
  document.documentElement.dataset.theme = resolvedTheme;
  document.documentElement.style.colorScheme = resolvedTheme;
}

function readStoredPreference(): ThemePreference {
  if (typeof window === "undefined") {
    return "system";
  }

  const storage = window.localStorage;
  if (!storage || typeof storage.getItem !== "function") {
    return "system";
  }

  const storedPreference = storage.getItem(THEME_STORAGE_KEY);
  return isThemePreference(storedPreference) ? storedPreference : "system";
}

function writeStoredPreference(preference: ThemePreference) {
  if (typeof window === "undefined") {
    return;
  }

  const storage = window.localStorage;
  if (!storage || typeof storage.setItem !== "function") {
    return;
  }

  storage.setItem(THEME_STORAGE_KEY, preference);
}

const initialPreference = readStoredPreference();
const initialResolvedTheme = resolveTheme(initialPreference);

applyTheme(initialResolvedTheme);

export const useThemeStore = create<ThemeState>((set, get) => ({
  preference: initialPreference,
  resolvedTheme: initialResolvedTheme,
  setPreference: (preference) => {
    writeStoredPreference(preference);
    const resolvedTheme = resolveTheme(preference);
    applyTheme(resolvedTheme);
    set({ preference, resolvedTheme });
  },
  syncResolvedTheme: () => {
    const resolvedTheme = resolveTheme(get().preference);
    applyTheme(resolvedTheme);
    set({ resolvedTheme });
  },
}));

export { THEME_MEDIA_QUERY };
