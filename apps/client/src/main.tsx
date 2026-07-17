import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "@tanstack/react-router";
import { Toaster } from "sonner";
import { useEffect } from "react";
import { router } from "@/router";
import { useBootstrapSession } from "@/hooks/use-bootstrap-session";
import { useThemeSync } from "@/hooks/use-theme-sync";
import { initializeGoogleCast } from "@/lib/google-cast";
import { registerPwaServiceWorker } from "@/lib/pwa";
import { QUERY_CACHE_GC_TIME_MS } from "@/lib/query-cache";
import { useThemeStore } from "@/store/theme-store";
import "plyr/dist/plyr.css";
import "./index.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 0,
      gcTime: QUERY_CACHE_GC_TIME_MS,
      refetchOnWindowFocus: false,
      refetchOnReconnect: false,
    },
  },
});

function Bootstrapper() {
  useBootstrapSession();
  useThemeSync();
  const resolvedTheme = useThemeStore((state) => state.resolvedTheme);

  useEffect(() => {
    registerPwaServiceWorker();
    void initializeGoogleCast();
  }, []);

  return (
    <>
      <RouterProvider router={router} />
      <Toaster richColors position="top-right" theme={resolvedTheme} />
    </>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <Bootstrapper />
    </QueryClientProvider>
  </React.StrictMode>,
);
