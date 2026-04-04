import { useEffect } from "react";
import { useTvModeStore } from "@/store/tv-mode-store";

export function useTvModeSync() {
  const syncEnvironment = useTvModeStore((state) => state.syncEnvironment);

  useEffect(() => {
    syncEnvironment();

    function handleResize() {
      syncEnvironment();
    }

    window.addEventListener("resize", handleResize);
    return () => {
      window.removeEventListener("resize", handleResize);
    };
  }, [syncEnvironment]);
}
