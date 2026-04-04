import { Link, Outlet, useRouterState } from "@tanstack/react-router";
import { TvMinimal, Search, Heart, Settings, ShieldCheck, LogOut } from "lucide-react";
import { Button } from "@/components/ui/button";
import { logout } from "@/lib/api";
import { clearRefreshToken } from "@/lib/tauri";
import { useAuthStore } from "@/store/auth-store";
import { PlayerView } from "@/features/player/player-view";
import { cn } from "@/lib/utils";

const navigation = [
  { to: "/guide", label: "Guide", icon: TvMinimal },
  { to: "/search", label: "Master Search", icon: Search },
  { to: "/favorites", label: "Favorites", icon: Heart },
  { to: "/provider", label: "Provider", icon: ShieldCheck },
  { to: "/settings", label: "Settings", icon: Settings },
];

export function AppShell() {
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const user = useAuthStore((state) => state.user);
  const clearSession = useAuthStore((state) => state.clearSession);

  async function handleLogout() {
    await logout();
    await clearRefreshToken();
    clearSession();
  }

  return (
    <div className="grid min-h-screen grid-cols-[260px_1fr_420px] gap-4 p-4 max-xl:grid-cols-[240px_1fr] max-xl:grid-rows-[1fr_auto] max-md:grid-cols-1">
      <aside className="glass-panel flex flex-col gap-6 rounded-[1.5rem] border border-border p-6">
        <div className="flex flex-col gap-2">
          <p className="text-xs font-semibold uppercase tracking-[0.28em] text-muted-foreground">Euripus</p>
          <h1 className="text-2xl font-semibold">IPTV control center</h1>
          <p className="text-sm text-muted-foreground">Search channels and EPG, sync favorites, and keep playback moving across devices.</p>
        </div>
        <nav className="flex flex-col gap-2">
          {navigation.map((item) => {
            const Icon = item.icon;
            const active = pathname === item.to;
            return (
              <Link
                key={item.to}
                to={item.to}
                className={cn(
                  "flex items-center gap-3 rounded-xl px-3 py-3 text-sm font-medium transition",
                  active ? "bg-primary text-primary-foreground shadow-sm" : "hover:bg-secondary/70",
                )}
              >
                <Icon />
                {item.label}
              </Link>
            );
          })}
        </nav>
        <div className="mt-auto flex flex-col gap-3 rounded-xl bg-card/70 p-4">
          <div>
            <p className="text-xs uppercase tracking-[0.24em] text-muted-foreground">Signed in</p>
            <p className="mt-1 text-sm font-medium">{user?.username ?? "Guest"}</p>
          </div>
          <Button variant="outline" onClick={handleLogout}>
            <LogOut />
            Logout
          </Button>
        </div>
      </aside>
      <main className="glass-panel rounded-[1.5rem] border border-border p-6 max-md:order-2">
        <Outlet />
      </main>
      <aside className="glass-panel rounded-[1.5rem] border border-border p-6 max-xl:col-span-2 max-md:order-1 max-md:col-span-1">
        <PlayerView />
      </aside>
    </div>
  );
}

