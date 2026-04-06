import type { ReactNode } from "react";
import { Outlet, createRootRoute, createRoute, createRouter, Navigate } from "@tanstack/react-router";
import { AppShell } from "@/components/layout/app-shell";
import { AuthPage } from "@/features/auth/auth-page";
import { FavoritesPage } from "@/features/channels/favorites-page";
import { GuidePage } from "@/features/channels/guide-page";
import { SearchPage } from "@/features/search/search-page";
import { SettingsPage } from "@/features/auth/settings-page";
import { ReceiverPage } from "@/features/receiver/receiver-page";
import { useAuthStore } from "@/store/auth-store";

export function SessionBootstrapFallback() {
  return <div className="grid min-h-screen place-items-center">Loading session...</div>;
}

export function RequireAuth({ children }: { children: ReactNode }) {
  const hydrated = useAuthStore((state) => state.hydrated);
  const user = useAuthStore((state) => state.user);

  if (!hydrated) {
    return <SessionBootstrapFallback />;
  }

  if (!user) {
    return <Navigate to="/auth" />;
  }

  return <>{children}</>;
}

export function AuthEntry() {
  const hydrated = useAuthStore((state) => state.hydrated);
  const user = useAuthStore((state) => state.user);

  if (!hydrated) {
    return <SessionBootstrapFallback />;
  }

  if (user) {
    return <Navigate to="/guide" />;
  }

  return <AuthPage />;
}

export function IndexRedirect() {
  const hydrated = useAuthStore((state) => state.hydrated);
  const user = useAuthStore((state) => state.user);

  if (!hydrated) {
    return <SessionBootstrapFallback />;
  }

  return <Navigate to={user ? "/guide" : "/auth"} />;
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
  component: AuthEntry,
});

const receiverRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/receiver",
  component: ReceiverPage,
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

const settingsRoute = createRoute({
  getParentRoute: () => authenticatedRoute,
  path: "/settings",
  component: SettingsPage,
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: IndexRedirect,
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  authRoute,
  receiverRoute,
  authenticatedRoute.addChildren([guideRoute, searchRoute, favoritesRoute, settingsRoute]),
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
