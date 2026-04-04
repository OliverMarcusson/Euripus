import { Link, Outlet, useRouterState } from "@tanstack/react-router";
import { ChevronDown, Heart, LogOut, Search, Settings, ShieldCheck, Tv, TvMinimal } from "lucide-react";
import { PlayerView } from "@/features/player/player-view";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { logout } from "@/lib/api";
import { clearRefreshToken } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useAuthStore } from "@/store/auth-store";

const navigation = [
  { to: "/guide", label: "Guide", icon: TvMinimal },
  { to: "/search", label: "Search", icon: Search },
  { to: "/favorites", label: "Favorites", icon: Heart },
  { to: "/provider", label: "Provider", icon: ShieldCheck },
  { to: "/settings", label: "Settings", icon: Settings },
] as const;

export function AppShell() {
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const user = useAuthStore((state) => state.user);
  const clearSession = useAuthStore((state) => state.clearSession);
  const initials = (user?.username ?? "Guest")
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? "")
    .join("")
    .slice(0, 2);

  async function handleLogout() {
    await logout();
    await clearRefreshToken();
    clearSession();
  }

  return (
    <TooltipProvider delayDuration={300}>
      <div className="grid h-screen grid-cols-[240px_minmax(0,1fr)_360px] bg-muted/30 max-xl:grid-cols-[240px_minmax(0,1fr)] max-xl:grid-rows-[minmax(0,1fr)_320px] max-md:grid-cols-1 max-md:grid-rows-[auto_minmax(0,1fr)_320px]">
        <aside className="flex min-h-0 flex-col border-r border-border bg-sidebar">
          <div className="flex items-center gap-3 border-b border-border px-5 py-5">
            <div className="flex size-10 items-center justify-center rounded-xl bg-primary text-primary-foreground">
              <Tv className="size-4" aria-hidden="true" />
            </div>
            <div className="flex flex-col">
              <span className="text-sm font-semibold tracking-tight">Euripus</span>
              <span className="text-xs text-muted-foreground">Live TV, search, and sync</span>
            </div>
          </div>

          <ScrollArea className="min-h-0 flex-1">
            <div className="flex flex-col gap-6 px-3 py-4">
              <div className="flex flex-col gap-1">
                <p className="px-2 text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">Browse</p>
                <nav className="flex flex-col gap-1">
                  {navigation.map((item) => {
                    const Icon = item.icon;
                    const active = pathname === item.to;

                    return (
                      <Tooltip key={item.to}>
                        <TooltipTrigger asChild>
                          <Link
                            to={item.to}
                            className={cn(
                              "flex items-center gap-3 rounded-xl px-3 py-2.5 text-sm font-medium transition-colors",
                              active
                                ? "bg-primary text-primary-foreground shadow-sm"
                                : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
                            )}
                          >
                            <Icon className="size-4" aria-hidden="true" />
                            <span className="truncate">{item.label}</span>
                          </Link>
                        </TooltipTrigger>
                        <TooltipContent side="right" className="md:hidden">
                          {item.label}
                        </TooltipContent>
                      </Tooltip>
                    );
                  })}
                </nav>
              </div>
            </div>
          </ScrollArea>

          <Separator />

          <div className="px-3 py-3">
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="ghost" className="h-auto w-full justify-start rounded-xl px-3 py-3 text-left">
                  <Avatar className="size-9">
                    <AvatarFallback className="bg-muted text-xs font-medium">{initials || "GU"}</AvatarFallback>
                  </Avatar>
                  <div className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate text-sm font-medium">{user?.username ?? "Guest"}</span>
                    <span className="truncate text-xs text-muted-foreground">Account menu</span>
                  </div>
                  <ChevronDown className="text-muted-foreground" aria-hidden="true" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-56">
                <DropdownMenuLabel>{user?.username ?? "Guest"}</DropdownMenuLabel>
                <DropdownMenuSeparator />
                <DropdownMenuGroup>
                  <DropdownMenuItem asChild>
                    <Link to="/settings">Open settings</Link>
                  </DropdownMenuItem>
                  <DropdownMenuItem onClick={handleLogout}>
                    <LogOut data-icon="inline-start" />
                    Sign out
                  </DropdownMenuItem>
                </DropdownMenuGroup>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </aside>

        <ScrollArea className="min-h-0 max-md:order-2">
          <main className="mx-auto flex min-h-full w-full max-w-[1200px] flex-col p-6 lg:p-8">
            <Outlet />
          </main>
        </ScrollArea>

        <aside className="min-h-0 border-l border-border bg-background max-xl:col-span-2 max-xl:border-l-0 max-xl:border-t max-md:order-3 max-md:col-span-1">
          <ScrollArea className="h-full">
            <div className="p-5 lg:p-6">
              <PlayerView />
            </div>
          </ScrollArea>
        </aside>
      </div>
    </TooltipProvider>
  );
}
