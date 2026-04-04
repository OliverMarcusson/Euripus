import { useEffect } from "react";
import { THEME_MEDIA_QUERY, useThemeStore } from "@/store/theme-store";

export function useThemeSync() {
  const preference = useThemeStore((state) => state.preference);
  const syncResolvedTheme = useThemeStore((state) => state.syncResolvedTheme);

  useEffect(() => {
    syncResolvedTheme();

    if (typeof window === "undefined" || typeof window.matchMedia !== "function" || preference !== "system") {
      return;
    }

    const mediaQuery = window.matchMedia(THEME_MEDIA_QUERY);
    const handleChange = () => {
      syncResolvedTheme();
    };

    if (typeof mediaQuery.addEventListener === "function") {
      mediaQuery.addEventListener("change", handleChange);
      return () => {
        mediaQuery.removeEventListener("change", handleChange);
      };
    }

    mediaQuery.addListener(handleChange);

    return () => {
      mediaQuery.removeListener(handleChange);
    };
  }, [preference, syncResolvedTheme]);
}
