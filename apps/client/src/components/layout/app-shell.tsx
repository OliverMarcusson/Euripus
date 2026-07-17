import { useEffect, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link, Outlet, useRouterState } from "@tanstack/react-router";
import { toast } from "sonner";
import {
  ChevronDown,
  Cast,
  Clapperboard,
  Heart,
  Film,
  LogOut,
  MonitorUp,
  Pause,
  Play,
  Search,
  Settings,
  Square,
  Trophy,
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
  logout,
  pairReceiver,
  pauseRemotePlayback,
  resumeRemotePlayback,
  selectRemoteControllerTarget,
  stopRemotePlayback,
} from "@/lib/api";
import {
  resolveRemoteTargetDevice,
  useRemoteControllerTargetQuery,
  useRemoteReceiversQuery,
} from "@/hooks/use-remote-control-state";
import {
  endGoogleCastSession,
  requestGoogleCastSession,
  useGoogleCastStore,
} from "@/lib/google-cast";
import { formatReceiverPlaybackSummary } from "@/lib/receiver-playback";
import { cn } from "@/lib/utils";
import { useAuthStore } from "@/store/auth-store";
import { usePlayerStore } from "@/store/player-store";
import { useRemoteControllerStore } from "@/store/remote-controller-store";
import { useChannelSettingsStore } from "@/store/channel-settings-store";

const navigation = [
  { to: "/guide", label: "Guide", icon: TvMinimal },
  { to: "/on-demand", label: "On Demand", icon: Film },
  { to: "/sports", label: "Sports", icon: Trophy },
  { to: "/search", label: "Search", icon: Search },
  { to: "/favorites", label: "Favorites", icon: Heart },
  { to: "/favorites/ppv", label: "PPV", icon: Clapperboard },
  { to: "/settings", label: "Settings", icon: Settings },
] as const;

function RemoteTargetMenu({
  compact = false,
  userAuthenticated,
}: {
  compact?: boolean;
  userAuthenticated: boolean;
}) {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const remoteTarget = useRemoteControllerStore((state) => state.target);
  const castAppIdConfigured = useGoogleCastStore((state) => state.appIdConfigured);
  const castAvailable = useGoogleCastStore((state) => state.available);
  const castConnected = useGoogleCastStore((state) => state.connected);
  const castDeviceName = useGoogleCastStore((state) => state.deviceName);
  const castReceiverDeviceId = useGoogleCastStore((state) => state.receiverDeviceId);
  const castReceiverPaired = useGoogleCastStore((state) => state.receiverPaired);
  const setTargetSelection = useRemoteControllerStore(
    (state) => state.setTargetSelection,
  );
  const clearTarget = useRemoteControllerStore((state) => state.clearTarget);
  const setPlayerSource = usePlayerStore((state) => state.setSource);
  const shouldPollTarget = userAuthenticated && (open || !!remoteTarget);
  const targetQuery = useRemoteControllerTargetQuery({
    enabled: userAuthenticated,
    refetchInterval: shouldPollTarget ? 5_000 : false,
  });
  const devicesQuery = useRemoteReceiversQuery({
    enabled: userAuthenticated && open,
    refetchInterval: open ? 5_000 : false,
  });
  const selectTargetMutation = useMutation({
    mutationFn: async (deviceId: string) => {
      if (castConnected && deviceId !== castReceiverDeviceId) {
        endGoogleCastSession();
      }
      return selectRemoteControllerTarget(deviceId);
    },
    onSuccess: async (selection) => {
      setTargetSelection(selection);
      setPlayerSource(null);
      queryClient.setQueryData(["remote", "controller", "target"], selection);
      await queryClient.invalidateQueries({ queryKey: ["remote", "receivers"] });
      setOpen(false);
    },
  });
  const clearTargetMutation = useMutation({
    mutationFn: clearRemoteControllerTarget,
    onSuccess: async () => {
      clearTarget();
      setPlayerSource(null);
      queryClient.setQueryData(["remote", "controller", "target"], null);
      await queryClient.invalidateQueries({ queryKey: ["remote", "receivers"] });
      setOpen(false);
    },
  });
  const connectCastMutation = useMutation({
    mutationFn: async () => {
      await requestGoogleCastSession();
      if (remoteTarget) {
        await clearRemoteControllerTarget();
      }
      clearTarget();
      queryClient.setQueryData(["remote", "controller", "target"], null);
    },
    onSuccess: () => {
      setPlayerSource(null);
      setOpen(false);
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : "Could not start casting.");
    },
  });
  const remoteDevices = devicesQuery.data ?? [];
  const resolvedTarget = resolveRemoteTargetDevice(
    remoteTarget,
    targetQuery.data?.device,
  );
  const buttonLabel = castConnected
    ? castDeviceName ?? "Google Cast"
    : resolvedTarget?.name ?? remoteTarget?.name ?? "This device";
  const targetActive = castConnected || !!remoteTarget;

  return (
    <DropdownMenu open={open} onOpenChange={setOpen}>
      <DropdownMenuTrigger asChild>
        <Button
          variant={targetActive ? "default" : "outline"}
          size={compact ? "sm" : "default"}
          className={cn("shrink-0", compact ? "h-9 px-3" : "justify-start")}
        >
          <MonitorUp className="size-4" />
          <span className={cn(compact ? "ml-2" : "ml-3")}>{buttonLabel}</span>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-72">
        <DropdownMenuLabel>Playback device</DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuItem
          onClick={() => {
            if (castConnected) {
              endGoogleCastSession();
              setPlayerSource(null);
              setOpen(false);
            } else {
              clearTargetMutation.mutate();
            }
          }}
          disabled={!targetActive || clearTargetMutation.isPending}
        >
          Play on this device
        </DropdownMenuItem>
        <DropdownMenuItem
          onClick={() => connectCastMutation.mutate()}
          disabled={
            castConnected ||
            !castAppIdConfigured ||
            !castAvailable ||
            connectCastMutation.isPending
          }
        >
          <Cast data-icon="inline-start" />
          {connectCastMutation.isPending
            ? "Opening receiver..."
            : !castAppIdConfigured
              ? "Cast App ID required"
              : castAvailable
                ? "Open Euripus receiver..."
                : "No Cast devices found"}
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
                  {formatReceiverPlaybackSummary(device)}
                </span>
              </div>
            </DropdownMenuItem>
          ))
        ) : (
          <DropdownMenuItem disabled>
            {devicesQuery.isPending ? "Loading screens..." : "No paired screens online"}
          </DropdownMenuItem>
        )}
        <DropdownMenuSeparator />
        <DropdownMenuItem asChild>
          <Link to="/settings">Pair a screen</Link>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function CastReceiverAutoSelector() {
  const queryClient = useQueryClient();
  const remoteTarget = useRemoteControllerStore((state) => state.target);
  const setTargetSelection = useRemoteControllerStore(
    (state) => state.setTargetSelection,
  );
  const castReceiverDeviceId = useGoogleCastStore(
    (state) => state.receiverDeviceId,
  );
  const castReceiverPaired = useGoogleCastStore(
    (state) => state.receiverPaired,
  );
  const castReceiverPairingCode = useGoogleCastStore(
    (state) => state.receiverPairingCode,
  );
  const castDeviceName = useGoogleCastStore((state) => state.deviceName);
  const selectMutation = useMutation({
    mutationFn: async () => {
      let deviceId = castReceiverDeviceId;
      if (!castReceiverPaired && castReceiverPairingCode) {
        const device = await pairReceiver({
          code: castReceiverPairingCode,
          rememberDevice: true,
          name: castDeviceName
            ? `Google Cast - ${castDeviceName}`
            : "Google Cast receiver",
        });
        deviceId = device.id;
        useGoogleCastStore.setState({
          receiverDeviceId: device.id,
          receiverPaired: true,
          receiverPairingCode: null,
        });
      }
      if (!deviceId) {
        throw new Error("The Cast receiver did not provide a device ID.");
      }
      return selectRemoteControllerTarget(deviceId);
    },
    onSuccess: (selection) => {
      setTargetSelection(selection);
      queryClient.setQueryData(["remote", "controller", "target"], selection);
      void queryClient.invalidateQueries({ queryKey: ["remote", "receivers"] });
    },
    onError: (error) => {
      toast.error(
        error instanceof Error
          ? error.message
          : "The Cast receiver could not be selected.",
      );
    },
  });

  useEffect(() => {
    if (
      (castReceiverPaired || !!castReceiverPairingCode) &&
      castReceiverDeviceId &&
      remoteTarget?.id !== castReceiverDeviceId &&
      !selectMutation.isPending
    ) {
      selectMutation.mutate();
    }
  }, [
    castReceiverDeviceId,
    castReceiverPaired,
    castReceiverPairingCode,
    remoteTarget?.id,
  ]);

  return null;
}

function RemoteTargetStatusBanner() {
  const remoteTarget = useRemoteControllerStore((state) => state.target);
  const castConnected = useGoogleCastStore((state) => state.connected);
  const castDeviceName = useGoogleCastStore((state) => state.deviceName);
  const targetQuery = useRemoteControllerTargetQuery({
    enabled: !!remoteTarget,
    refetchInterval: remoteTarget ? 5_000 : false,
  });
  const resolvedTarget = resolveRemoteTargetDevice(
    remoteTarget,
    targetQuery.data?.device,
  );
  const controlMutation = useMutation({
    mutationFn: async (command: "play" | "pause" | "stop") => {
      if (command === "play") return resumeRemotePlayback();
      if (command === "pause") return pauseRemotePlayback();
      return stopRemotePlayback();
    },
    onError: (error) => toast.error(error instanceof Error ? error.message : "Playback control failed."),
  });
  const controlsDisabled =
    controlMutation.isPending || !resolvedTarget?.currentPlayback;

  if (!remoteTarget && !castConnected) {
    return null;
  }

  return (
    <div className="shrink-0 border-b border-border/40 bg-primary/5 px-4 py-2 text-sm text-foreground/80 md:px-8">
      <div className="mx-auto flex w-full max-w-[1240px] flex-wrap items-center justify-center gap-2">
        <span>
          {castConnected ? <>
            Euripus Receiver open on <span className="font-semibold">{castDeviceName ?? "Google Cast"}</span>
          </> : <>
            Controlling <span className="font-semibold">{remoteTarget?.name}</span>
            {resolvedTarget?.currentPlayback ? ` - ${formatReceiverPlaybackSummary(resolvedTarget)}` : ""}
          </>}
        </span>
        <div className="flex items-center gap-1" aria-label="Playback controls">
          <Button size="icon" variant="ghost" className="size-7" aria-label="Play" disabled={controlsDisabled} onClick={() => controlMutation.mutate("play")}><Play className="size-4" /></Button>
          <Button size="icon" variant="ghost" className="size-7" aria-label="Pause" disabled={controlsDisabled} onClick={() => controlMutation.mutate("pause")}><Pause className="size-4" /></Button>
          <Button size="icon" variant="ghost" className="size-7" aria-label="Stop" disabled={controlsDisabled} onClick={() => controlMutation.mutate("stop")}><Square className="size-4" /></Button>
        </div>
      </div>
    </div>
  );
}

export function AppShell() {
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });
  const user = useAuthStore((state) => state.user);
  const clearSession = useAuthStore((state) => state.clearSession);
  const adminToolsEnabled = useChannelSettingsStore((state) => state.adminToolsEnabled);
  const hasPlayerSource = usePlayerStore((state) => !!state.source);
  const clearTarget = useRemoteControllerStore((state) => state.clearTarget);
  const initials = (user?.username ?? "Guest")
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? "")
    .join("")
    .slice(0, 2);

  useEffect(() => {
    if (!user) {
      clearTarget();
      endGoogleCastSession();
    }
  }, [clearTarget, user]);

  async function handleLogout() {
    await logout();
    clearSession();
  }

  const MobileTopHeader = () => (
    <div className="z-20 flex h-14 shrink-0 items-center justify-between border-b border-border/40 bg-sidebar px-4 md:hidden">
      <div className="flex items-center gap-2">
        <Tv className="size-5 text-primary" aria-hidden="true" />
        <span className="text-sm font-semibold tracking-tight">Euripus</span>
      </div>
      <div className="flex items-center gap-2">
        <RemoteTargetMenu compact userAuthenticated={!!user} />
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
              {user?.isAdmin && adminToolsEnabled ? (
                <DropdownMenuItem asChild><Link to="/admin">Open admin panel</Link></DropdownMenuItem>
              ) : null}
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
      <CastReceiverAutoSelector />
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
            <RemoteTargetMenu userAuthenticated={!!user} />
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
                {user?.isAdmin && adminToolsEnabled ? (
                  <DropdownMenuItem asChild className="cursor-pointer rounded-lg"><Link to="/admin">Open admin panel</Link></DropdownMenuItem>
                ) : null}
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
          <RemoteTargetStatusBanner />

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

      <nav className="z-30 flex h-16 shrink-0 items-center overflow-x-auto border-t border-border/40 bg-sidebar pb-[env(safe-area-inset-bottom)] md:hidden">
        {navigation.map((item) => {
          const Icon = item.icon;
          const active = pathname === item.to;
          return (
            <Link
              key={item.to}
              to={item.to}
              className={cn(
                "relative flex h-full min-w-20 flex-1 flex-col items-center justify-center gap-[3px] transition-colors",
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
