import type { ReactNode } from "react";
import { Outlet, createRootRoute, createRoute, createRouter, Navigate } from "@tanstack/react-router";
import { AppShell } from "@/components/layout/app-shell";
import { AuthPage } from "@/features/auth/auth-page";
import { FavoritesPage } from "@/features/channels/favorites-page";
import { GuidePage } from "@/features/channels/guide-page";
import { ProviderPage } from "@/features/provider/provider-page";
import { SearchPage } from "@/features/search/search-page";
import { SettingsPage } from "@/features/auth/settings-page";
import { useAuthStore } from "@/store/auth-store";

function RequireAuth({ children }: { children: ReactNode }) {
  const hydrated = useAuthStore((state) => state.hydrated);
  const user = useAuthStore((state) => state.user);

  if (!hydrated) {
    return <div className="grid min-h-screen place-items-center">Loading session...</div>;
  }

  if (!user) {
    return <Navigate to="/auth" />;
  }

  return <>{children}</>;
}

const rootRoute = createRootRoute({
  component: Outlet,
});

const authenticatedRoute = createRoute({
  getParentRoute: () => rootRoute,
  id: "authenticated",
  component: () => <RequireAuth>{<AppShell />}</RequireAuth>,
});

const authRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/auth",
  component: AuthPage,
});

const guideRoute = createRoute({
  getParentRoute: () => authenticatedRoute,
  path: "/guide",
  component: GuidePage,
});

const searchRoute = createRoute({
  getParentRoute: () => authenticatedRoute,
  path: "/search",
  component: SearchPage,
});

const favoritesRoute = createRoute({
  getParentRoute: () => authenticatedRoute,
  path: "/favorites",
  component: FavoritesPage,
});

const providerRoute = createRoute({
  getParentRoute: () => authenticatedRoute,
  path: "/provider",
  component: ProviderPage,
});

const settingsRoute = createRoute({
  getParentRoute: () => authenticatedRoute,
  path: "/settings",
  component: SettingsPage,
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: () => {
    const user = useAuthStore((state) => state.user);
    return <Navigate to={user ? "/guide" : "/auth"} />;
  },
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  authRoute,
  authenticatedRoute.addChildren([guideRoute, searchRoute, favoritesRoute, providerRoute, settingsRoute]),
]);

export const router = createRouter({
  routeTree,
  defaultPreload: "intent",
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
