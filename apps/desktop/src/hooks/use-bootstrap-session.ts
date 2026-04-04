import { useEffect } from "react";
import { refresh } from "@/lib/api";
import { useAuthStore } from "@/store/auth-store";

export function useBootstrapSession() {
  const setSession = useAuthStore((state) => state.setSession);
  const clearSession = useAuthStore((state) => state.clearSession);
  const setHydrated = useAuthStore((state) => state.setHydrated);

  useEffect(() => {
    let mounted = true;

    async function bootstrap() {
      try {
        const session = await refresh();
        if (mounted) {
          setSession(session);
        }
      } catch {
        if (mounted) {
          clearSession();
        }
      } finally {
        if (mounted) {
          setHydrated(true);
        }
      }
    }

    void bootstrap();

    return () => {
      mounted = false;
    };
  }, [clearSession, setHydrated, setSession]);
}
