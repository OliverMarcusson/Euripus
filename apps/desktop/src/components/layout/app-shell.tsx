import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, Outlet, useRouterState } from "@tanstack/react-router";
import { ChevronDown, Heart, LogOut, MonitorUp, Search, Settings, Tv, TvMinimal } from "lucide-react";
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
import {
  clearRemoteControllerTarget,
  getRemoteControllerTarget,
  getRemoteDevices,
  logout,
  selectRemoteControllerTarget,
} from "@/lib/api";
import { cn } from "@/lib/utils";
import { useAuthStore } from "@/store/auth-store";
import { usePlayerStore } from "@/store/player-store";
import { useRemoteControllerStore } from "@/store/remote-controller-store";
import { useTvModeStore } from "@/store/tv-mode-store";

const navigation = [
  { to: "/guide", label: "Guide", icon: TvMinimal },
  { to: "/search", label: "Search", icon: Search },
  { to: "/favorites", label: "Favorites", icon: Heart },
  { to: "/settings", label: "Settings", icon: Settings },
] as const;

export function AppShell() {
  const queryClient = useQueryClient();
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const user = useAuthStore((state) => state.user);
  const clearSession = useAuthStore((state) => state.clearSession);
  const isTvMode = useTvModeStore((state) => state.isTvMode);
  const hasPlayerSource = usePlayerStore((state) => !!state.source);
  const setPlayerSource = usePlayerStore((state) => state.setSource);
  const remoteTarget = useRemoteControllerStore((state) => state.target);
  const setTargetSelection = useRemoteControllerStore((state) => state.setTargetSelection);
  const clearTarget = useRemoteControllerStore((state) => state.clearTarget);
  const initials = (user?.username ?? "Guest")
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? "")
    .join("")
    .slice(0, 2);
  const devicesQuery = useQuery({
    queryKey: ["remote", "devices"],
    queryFn: getRemoteDevices,
    enabled: !!user,
    refetchInterval: user ? 5_000 : false,
  });
  const targetQuery = useQuery({
    queryKey: ["remote", "controller", "target"],
    queryFn: getRemoteControllerTarget,
    enabled: !!user,
    refetchInterval: user ? 5_000 : false,
  });
  const selectTargetMutation = useMutation({
    mutationFn: selectRemoteControllerTarget,
    onSuccess: async (selection) => {
      setTargetSelection(selection);
      setPlayerSource(null);
      await queryClient.invalidateQueries({ queryKey: ["remote"] });
    },
  });
  const clearTargetMutation = useMutation({
    mutationFn: clearRemoteControllerTarget,
    onSuccess: async () => {
      clearTarget();
      setPlayerSource(null);
      await queryClient.invalidateQueries({ queryKey: ["remote"] });
    },
  });
  const remoteDevices = devicesQuery.data ?? [];

  useEffect(() => {
    if (!user) {
      clearTarget();
      return;
    }

    setTargetSelection(targetQuery.data ?? null);
  }, [clearTarget, setTargetSelection, targetQuery.data, user]);

  async function handleLogout() {
    await logout();
    clearSession();
  }

  const RemoteTargetMenu = ({ compact = false }: { compact?: boolean }) => (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant={remoteTarget ? "default" : "outline"}
          size={compact ? "sm" : "default"}
          className={cn("shrink-0", compact ? "h-9 px-3" : "justify-start")}
        >
          <MonitorUp className="size-4" />
          <span className={cn(compact ? "ml-2" : "ml-3")}>
            {remoteTarget ? remoteTarget.name : "This device"}
          </span>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-72">
        <DropdownMenuLabel>Playback target</DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuItem
          onClick={() => clearTargetMutation.mutate()}
          disabled={!remoteTarget || clearTargetMutation.isPending}
        >
          Play on this device
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        {remoteDevices.length ? (
          remoteDevices.map((device) => (
            <DropdownMenuItem
              key={device.id}
              onClick={() => selectTargetMutation.mutate(device.id)}
              disabled={selectTargetMutation.isPending}
            >
              <div className="flex min-w-0 flex-col">
                <span className="truncate font-medium">{device.name}</span>
                <span className="truncate text-xs text-muted-foreground">
                  {device.currentPlayback ? `Now playing ${device.currentPlayback.title}` : device.platform}
                </span>
              </div>
            </DropdownMenuItem>
          ))
        ) : (
          <DropdownMenuItem disabled>No remote targets online</DropdownMenuItem>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );

  const MobileTopHeader = () => (
    <div className="md:hidden flex h-14 shrink-0 items-center justify-between border-b border-border/40 bg-sidebar px-4 z-20">
      <div className="flex items-center gap-2">
        <Tv className="size-5 text-primary" aria-hidden="true" />
        <span className="text-sm font-semibold tracking-tight">Euripus</span>
      </div>
      <div className="flex items-center gap-2">
        <RemoteTargetMenu compact />
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="icon" className="size-8 rounded-full">
              <Avatar className="size-8">
                <AvatarFallback className="bg-muted text-xs font-medium">{initials || "GU"}</AvatarFallback>
              </Avatar>
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
    </div>
  );

  return (
    <TooltipProvider delayDuration={300}>
      <div
        data-tv-mode={isTvMode ? "true" : "false"}
        className={cn("flex h-screen w-full flex-col overflow-hidden bg-background md:flex-row")}
      >
        <MobileTopHeader />
        {remoteTarget ? (
          <div className="shrink-0 border-b border-border/40 bg-primary/5 px-4 py-2 text-sm text-foreground/80 md:px-6">
            Controlling <span className="font-semibold">{remoteTarget.name}</span>
            {remoteTarget.currentPlayback ? ` • ${remoteTarget.currentPlayback.title}` : ""}
          </div>
        ) : null}

        <aside
          className={cn(
            "group/sidebar relative z-20 shrink-0 flex-col border-r border-border/40 bg-sidebar shadow-[4px_0_24px_rgba(0,0,0,0.02)] transition-[width] duration-300",
            isTvMode
              ? "w-[80px] overflow-hidden hover:w-[280px] focus-within:w-[280px] flex"
              : "w-[240px] max-md:hidden flex"
          )}
        >
          <div className="flex h-[88px] shrink-0 items-center px-6 border-b border-border/40 overflow-hidden">
            <div className="flex items-center gap-4 w-[240px]">
              <div className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground shadow-[0_0_12px_rgba(168,85,247,0.4)] ring-1 ring-white/10">
                <Tv className="size-4 shrink-0" aria-hidden="true" />
              </div>
              <div className={cn("flex flex-col min-w-0 transition-opacity duration-200", isTvMode ? "opacity-0 group-hover/sidebar:opacity-100 group-focus-within/sidebar:opacity-100" : "opacity-100")}>
                <span className="truncate text-[15px] font-bold tracking-[-0.02em] text-foreground/90">Euripus</span>
              </div>
            </div>
          </div>

          <ScrollArea className="min-h-0 flex-1">
            <div className="flex flex-col gap-6 px-3 py-6 w-[240px]">
              <div className="flex flex-col gap-1.5">
                <p className={cn("px-4 pb-1 text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground/60 transition-opacity duration-200", isTvMode ? "opacity-0 group-hover/sidebar:opacity-100 group-focus-within/sidebar:opacity-100" : "opacity-100")}>Menu</p>
                <nav className="flex flex-col gap-1">
                  {navigation.map((item) => {
                    const Icon = item.icon;
                    const active = pathname === item.to;

                    return (
                      <Tooltip key={item.to}>
                        <TooltipTrigger asChild>
                          <Link
                            to={item.to}
                            data-tv-focusable="true"
                            data-tv-autofocus={active ? "true" : undefined}
                            className={cn(
                              "flex items-center gap-4 rounded-xl px-4 py-3 text-[14px] font-semibold transition-all duration-300 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 overflow-hidden",
                              isTvMode && "min-h-14",
                              active
                                ? "bg-primary/10 text-primary translate-x-1"
                                : "text-muted-foreground/80 hover:bg-muted/40 hover:text-foreground",
                            )}
                          >
                            <Icon className={cn("size-[18px] shrink-0 transition-transform duration-300", active && "scale-110 drop-shadow-[0_0_8px_rgba(168,85,247,0.4)]")} aria-hidden="true" />
                            <span className={cn("truncate transition-opacity duration-200", isTvMode ? "opacity-0 group-hover/sidebar:opacity-100 group-focus-within/sidebar:opacity-100" : "opacity-100")}>{item.label}</span>
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

          <Separator className="w-[85%] mx-auto opacity-50" />

          <div className="px-3 py-3 overflow-hidden w-[240px]">
            <div className="mb-3">
              <RemoteTargetMenu />
            </div>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="ghost" className="h-[52px] w-[216px] justify-start rounded-xl px-2.5 text-left overflow-hidden hover:bg-muted/40">
                  <div className="flex items-center gap-3 w-full">
                    <Avatar className="size-8 shrink-0 ring-1 ring-border/50">
                      <AvatarFallback className="bg-primary/5 text-primary text-[10px] font-bold">{initials || "GU"}</AvatarFallback>
                    </Avatar>
                    <div className={cn("flex min-w-0 flex-1 flex-col transition-opacity duration-200", isTvMode ? "opacity-0 group-hover/sidebar:opacity-100 group-focus-within/sidebar:opacity-100" : "opacity-100")}>
                      <span className="truncate text-sm font-semibold text-foreground/90">{user?.username ?? "Guest"}</span>
                    </div>
                    <ChevronDown className={cn("text-muted-foreground/50 size-4 shrink-0 transition-opacity duration-200", isTvMode ? "opacity-0 group-hover/sidebar:opacity-100 group-focus-within/sidebar:opacity-100" : "opacity-100")} aria-hidden="true" />
                  </div>
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-[200px] rounded-xl">
                <DropdownMenuLabel className="font-bold">{user?.username ?? "Guest"}</DropdownMenuLabel>
                <DropdownMenuSeparator />
                <DropdownMenuGroup>
                  <DropdownMenuItem asChild className="rounded-lg cursor-pointer">
                    <Link to="/settings">Open settings</Link>
                  </DropdownMenuItem>
                  <DropdownMenuItem onClick={handleLogout} className="rounded-lg cursor-pointer text-destructive focus:text-destructive">
                    <LogOut className="mr-2 size-4" />
                    Sign out
                  </DropdownMenuItem>
                </DropdownMenuGroup>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </aside>

        <div className={cn(
          "flex min-w-0 flex-1 flex-col overflow-hidden relative",
          !isTvMode ? "md:flex-row" : "flex-row",
        )}>
          <ScrollArea className="flex-1 min-w-0 min-h-0 bg-background/50 outline-none z-0">
            <main className={cn(
              "mx-auto flex min-h-full w-full max-w-[1240px] flex-col p-4 md:p-8 relative overflow-x-hidden",
              isTvMode && "max-w-[1600px] p-8 lg:p-12 mb-10"
            )}>
              <Outlet />
            </main>
          </ScrollArea>

          <aside
            className={cn(
              "z-10 shrink-0 flex flex-col border-border/20 transition-all duration-300",
              isTvMode
                ? "w-[380px] border-l relative bg-background/95 backdrop-blur-xl h-full shadow-[-40px_0_100px_rgba(0,0,0,0.4)]"
                : cn(
                    hasPlayerSource ? "max-md:border-t max-md:border-border/40" : "max-md:hidden",
                    "md:h-full md:w-[320px] lg:w-[360px] md:border-l xl:bg-background/40"
                  )
            )}
          >
            <div className={cn(
              "p-0 w-full",
              !isTvMode ? "md:p-5 xl:p-6 md:h-full" : "py-8 pr-8 pl-4 h-full"
            )}>
              <PlayerView />
            </div>
          </aside>
        </div>

        {!isTvMode && (
          <nav className="md:hidden flex h-16 shrink-0 items-center justify-around border-t border-border/40 bg-sidebar z-30 pb-[env(safe-area-inset-bottom)]">
            {navigation.map((item) => {
              const Icon = item.icon;
              const active = pathname === item.to;
              return (
                <Link
                  key={item.to}
                  to={item.to}
                  className={cn(
                    "relative flex flex-col items-center justify-center gap-[3px] w-full h-full transition-colors",
                    active ? "text-primary" : "text-muted-foreground/60 hover:text-foreground"
                  )}
                >
                  <Icon className={cn("size-[22px] shrink-0 transition-transform duration-300", active && "scale-110 drop-shadow-[0_0_8px_rgba(168,85,247,0.4)]")} aria-hidden="true" />
                  <span className="text-[10px] font-bold tracking-wide">{item.label}</span>
                </Link>
              );
            })}
          </nav>
        )}
      </div>
    </TooltipProvider>
  );
}
