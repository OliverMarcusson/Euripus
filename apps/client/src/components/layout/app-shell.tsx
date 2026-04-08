import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, Outlet, useRouterState } from "@tanstack/react-router";
import {
  ChevronDown,
  Heart,
  LogOut,
  MonitorUp,
  Search,
  Settings,
  Tv,
  TvMinimal,
} from "lucide-react";
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
import {
  clearRemoteControllerTarget,
  getRemoteControllerTarget,
  getRemoteReceivers,
  logout,
  selectRemoteControllerTarget,
} from "@/lib/api";
import { cn } from "@/lib/utils";
import { useAuthStore } from "@/store/auth-store";
import { usePlayerStore } from "@/store/player-store";
import { useRemoteControllerStore } from "@/store/remote-controller-store";

const navigation = [
  { to: "/guide", label: "Guide", icon: TvMinimal },
  { to: "/search", label: "Search", icon: Search },
  { to: "/favorites", label: "Favorites", icon: Heart },
  { to: "/settings", label: "Settings", icon: Settings },
] as const;

export function AppShell() {
  const queryClient = useQueryClient();
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });
  const user = useAuthStore((state) => state.user);
  const clearSession = useAuthStore((state) => state.clearSession);
  const hasPlayerSource = usePlayerStore((state) => !!state.source);
  const setPlayerSource = usePlayerStore((state) => state.setSource);
  const remoteTarget = useRemoteControllerStore((state) => state.target);
  const setTargetSelection = useRemoteControllerStore(
    (state) => state.setTargetSelection,
  );
  const clearTarget = useRemoteControllerStore((state) => state.clearTarget);
  const initials = (user?.username ?? "Guest")
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? "")
    .join("")
    .slice(0, 2);
  const devicesQuery = useQuery({
    queryKey: ["remote", "receivers"],
    queryFn: getRemoteReceivers,
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
        <DropdownMenuLabel>Receiver</DropdownMenuLabel>
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
              disabled={selectTargetMutation.isPending || !device.online}
            >
              <div className="flex min-w-0 flex-col">
                <span className="truncate font-medium">{device.name}</span>
                <span className="truncate text-xs text-muted-foreground">
                  {device.currentPlayback
                    ? `Now playing ${device.currentPlayback.title}`
                    : device.online
                      ? device.platform
                      : "Offline"}
                </span>
              </div>
            </DropdownMenuItem>
          ))
        ) : (
          <DropdownMenuItem disabled>No paired screens online</DropdownMenuItem>
        )}
        <DropdownMenuSeparator />
        <DropdownMenuItem asChild>
          <Link to="/settings">Pair a screen</Link>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );

  const MobileTopHeader = () => (
    <div className="z-20 flex h-14 shrink-0 items-center justify-between border-b border-border/40 bg-sidebar px-4 md:hidden">
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
                <AvatarFallback className="bg-muted text-xs font-medium">
                  {initials || "GU"}
                </AvatarFallback>
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
    <div className="flex h-screen w-full flex-col overflow-hidden bg-background md:flex-row">
      <MobileTopHeader />

      <aside className="relative z-20 hidden w-[240px] shrink-0 flex-col border-r border-border/40 bg-sidebar shadow-[4px_0_24px_rgba(0,0,0,0.02)] md:flex">
        <div className="flex h-[88px] shrink-0 items-center gap-4 border-b border-border/40 px-6">
          <div className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground shadow-[0_0_12px_rgba(168,85,247,0.4)] ring-1 ring-white/10">
            <Tv className="size-4 shrink-0" aria-hidden="true" />
          </div>
          <span className="truncate text-[15px] font-bold tracking-[-0.02em] text-foreground/90">
            Euripus
          </span>
        </div>

        <ScrollArea className="min-h-0 flex-1">
          <div className="flex flex-col gap-6 px-3 py-6">
            <div className="flex flex-col gap-1.5">
              <p className="px-4 pb-1 text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground/60">
                Menu
              </p>
              <nav className="flex flex-col gap-1">
                {navigation.map((item) => {
                  const Icon = item.icon;
                  const active = pathname === item.to;

                  return (
                    <Link
                      key={item.to}
                      to={item.to}
                      className={cn(
                        "flex items-center gap-4 rounded-xl px-4 py-3 text-[14px] font-semibold transition-all duration-300 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
                        active
                          ? "translate-x-1 bg-primary/10 text-primary"
                          : "text-muted-foreground/80 hover:bg-muted/40 hover:text-foreground",
                      )}
                    >
                      <Icon
                        className={cn(
                          "size-[18px] shrink-0 transition-transform duration-300",
                          active &&
                            "scale-110 drop-shadow-[0_0_8px_rgba(168,85,247,0.4)]",
                        )}
                        aria-hidden="true"
                      />
                      <span className="truncate">{item.label}</span>
                    </Link>
                  );
                })}
              </nav>
            </div>
          </div>
        </ScrollArea>

        <Separator className="mx-auto w-[85%] opacity-50" />

        <div className="px-3 py-3">
          <div className="mb-3">
            <RemoteTargetMenu />
          </div>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                className="h-[52px] w-[216px] justify-start rounded-xl px-2.5 text-left hover:bg-muted/40"
              >
                <div className="flex w-full items-center gap-3">
                  <Avatar className="size-8 shrink-0 ring-1 ring-border/50">
                    <AvatarFallback className="bg-primary/5 text-[10px] font-bold text-primary">
                      {initials || "GU"}
                    </AvatarFallback>
                  </Avatar>
                  <div className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate text-sm font-semibold text-foreground/90">
                      {user?.username ?? "Guest"}
                    </span>
                  </div>
                  <ChevronDown
                    className="size-4 shrink-0 text-muted-foreground/50"
                    aria-hidden="true"
                  />
                </div>
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-[200px] rounded-xl">
              <DropdownMenuLabel className="font-bold">
                {user?.username ?? "Guest"}
              </DropdownMenuLabel>
              <DropdownMenuSeparator />
              <DropdownMenuGroup>
                <DropdownMenuItem asChild className="cursor-pointer rounded-lg">
                  <Link to="/settings">Open settings</Link>
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={handleLogout}
                  className="cursor-pointer rounded-lg text-destructive focus:text-destructive"
                >
                  <LogOut className="mr-2 size-4" />
                  Sign out
                </DropdownMenuItem>
              </DropdownMenuGroup>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </aside>

      <div className="relative flex min-w-0 flex-1 flex-col overflow-hidden md:flex-row">
        <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
          {remoteTarget ? (
            <div className="shrink-0 border-b border-border/40 bg-primary/5 px-4 py-2 text-sm text-foreground/80 md:px-8">
              <div className="mx-auto w-full max-w-[1240px] text-center">
                Controlling{" "}
                <span className="font-semibold">{remoteTarget.name}</span>
                {remoteTarget.currentPlayback
                  ? ` - ${remoteTarget.currentPlayback.title}`
                  : ""}
              </div>
            </div>
          ) : null}

          <ScrollArea className="z-0 min-h-0 min-w-0 flex-1 bg-background/50 outline-none">
            <main className="relative mx-auto flex min-h-full w-full max-w-[1240px] flex-col overflow-x-hidden p-4 md:p-8">
              <Outlet />
            </main>
          </ScrollArea>
        </div>

        <aside
          className={cn(
            "z-10 shrink-0 flex-col border-border/20",
            hasPlayerSource
              ? "max-md:flex max-md:border-t max-md:border-border/40"
              : "max-md:hidden",
            "md:flex md:h-full md:w-[320px] md:border-l lg:w-[360px] xl:bg-background/40",
          )}
        >
          <div className="w-full p-0 md:h-full md:p-5 xl:p-6">
            <PlayerView />
          </div>
        </aside>
      </div>

      <nav className="z-30 flex h-16 shrink-0 items-center justify-around border-t border-border/40 bg-sidebar pb-[env(safe-area-inset-bottom)] md:hidden">
        {navigation.map((item) => {
          const Icon = item.icon;
          const active = pathname === item.to;
          return (
            <Link
              key={item.to}
              to={item.to}
              className={cn(
                "relative flex h-full w-full flex-col items-center justify-center gap-[3px] transition-colors",
                active
                  ? "text-primary"
                  : "text-muted-foreground/60 hover:text-foreground",
              )}
            >
              <Icon
                className={cn(
                  "size-[22px] shrink-0 transition-transform duration-300",
                  active &&
                    "scale-110 drop-shadow-[0_0_8px_rgba(168,85,247,0.4)]",
                )}
                aria-hidden="true"
              />
              <span className="text-[10px] font-bold tracking-wide">
                {item.label}
              </span>
            </Link>
          );
        })}
      </nav>
    </div>
  );
}
