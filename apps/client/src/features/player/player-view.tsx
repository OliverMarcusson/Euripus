import { useCallback, useRef, useState } from "react";
import {
  ChevronDown,
  ChevronUp,
  Pause,
  Play,
  Radio,
  Square,
  X,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { PlyrSurface } from "@/components/player/plyr-surface";
import {
  Empty,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import {
  startChannelPlayback,
  startProgramPlayback,
  pauseRemotePlayback,
  resumeRemotePlayback,
  stopRemotePlayback,
} from "@/lib/api";
import { logPlaybackDiagnostic } from "@/lib/playback-diagnostics";
import { usePlayerStore } from "@/store/player-store";
import { useRemoteControllerStore } from "@/store/remote-controller-store";
import { cn, formatRelativeTime } from "@/lib/utils";

export function PlayerView() {
  const currentRequest = usePlayerStore((state) => state.currentRequest);
  const loading = usePlayerStore((state) => state.loading);
  const source = usePlayerStore((state) => state.source);
  const setLoading = usePlayerStore((state) => state.setLoading);
  const setPlayback = usePlayerStore((state) => state.setPlayback);
  const setSource = usePlayerStore((state) => state.setSource);
  const remoteTarget = useRemoteControllerStore((state) => state.target);
  const [minimized, setMinimized] = useState(false);
  const recoveryInFlightRef = useRef(false);

  const handleRecoveryNeeded = useCallback(async () => {
    if (!currentRequest || recoveryInFlightRef.current || loading) {
      return;
    }

    recoveryInFlightRef.current = true;
    setLoading(true);
    logPlaybackDiagnostic("warn", "playback-recovery-started", {
      ownershipHint: "client-player",
      requestKind: currentRequest.kind,
      requestId: currentRequest.id,
      previousSourceKind: source?.kind ?? null,
      previousSourceUrl: source?.url ?? null,
    });
    try {
      const nextSource =
        currentRequest.kind === "channel"
          ? await startChannelPlayback(currentRequest.id)
          : await startProgramPlayback(currentRequest.id);
      setPlayback(nextSource, currentRequest);
      logPlaybackDiagnostic("info", "playback-recovery-succeeded", {
        ownershipHint: "client-player",
        requestKind: currentRequest.kind,
        requestId: currentRequest.id,
        nextSourceKind: nextSource.kind,
        nextSourceUrl: nextSource.url,
      });
    } catch {
      logPlaybackDiagnostic("error", "playback-recovery-failed", {
        ownershipHint: "client-player",
        requestKind: currentRequest.kind,
        requestId: currentRequest.id,
      });
      setLoading(false);
    } finally {
      recoveryInFlightRef.current = false;
    }
  }, [currentRequest, loading, setLoading, setPlayback]);

  if (!source) {
    if (remoteTarget?.currentPlayback) {
      return (
        <Card className="h-full border-0 bg-transparent shadow-none">
          <CardContent className="flex h-full flex-col gap-4 p-0">
            <div className="rounded-2xl border border-border/40 bg-muted/10 p-5">
              <div className="flex flex-wrap items-center gap-2">
                <Badge
                  variant={
                    remoteTarget.currentPlayback.live ? "live" : "outline"
                  }
                >
                  {remoteTarget.currentPlayback.live ? "Live" : "Archive"}
                </Badge>
                <Badge variant="outline">{remoteTarget.name}</Badge>
              </div>
              <h2 className="mt-3 text-lg font-semibold">
                {remoteTarget.currentPlayback.title}
              </h2>
              <div className="mt-4 flex flex-wrap gap-2">
                <Button
                  size="sm"
                  variant="secondary"
                  onClick={() => void resumeRemotePlayback()}
                >
                  <Play data-icon="inline-start" />
                  Play
                </Button>
                <Button
                  size="sm"
                  variant="secondary"
                  onClick={() => void pauseRemotePlayback()}
                >
                  <Pause data-icon="inline-start" />
                  Pause
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => void stopRemotePlayback()}
                >
                  <Square data-icon="inline-start" />
                  Stop
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>
      );
    }

    return (
      <Card className="h-full max-md:hidden border-0 bg-transparent shadow-none">
        <CardContent className="flex h-full flex-col p-0 opacity-50 grayscale">
          <Empty className="flex-1 mt-6 border-0 rounded-2xl bg-muted/5">
            <EmptyHeader>
              <EmptyMedia
                variant="icon"
                className="text-muted-foreground/30 ring-0 shadow-none bg-background/50"
              >
                <Radio aria-hidden="true" className="size-6" />
              </EmptyMedia>
              <EmptyTitle className="text-sm font-semibold text-muted-foreground/60 mt-2">
                Choose a channel or program
              </EmptyTitle>
            </EmptyHeader>
          </Empty>
        </CardContent>
      </Card>
    );
  }

  if (minimized) {
    return (
      <Card className="overflow-hidden max-md:rounded-none max-md:border-x-0 max-md:border-b-0 max-md:shadow-none max-md:border-border/40 max-md:bg-sidebar/95 max-md:backdrop-blur-md supports-[backdrop-filter]:max-md:bg-sidebar/80 md:border-0 md:bg-transparent md:shadow-none">
        <div className="flex items-center gap-3 px-3 py-2 w-full">
          <div
            className="flex-1 min-w-0 flex flex-col justify-center cursor-pointer"
            onClick={() => setMinimized(false)}
          >
            <span className="truncate text-sm font-bold tracking-tight text-foreground/95">
              {source.title}
            </span>
            <span className="truncate text-[10px] font-bold text-muted-foreground/80 uppercase tracking-widest">
              {source.kind} {source.live ? "LIVE" : ""}
            </span>
          </div>
          <Button
            variant="ghost"
            size="icon"
            className="size-8 rounded-full shrink-0 md:hidden"
            onClick={() => setMinimized(false)}
          >
            <ChevronUp className="size-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="size-8 rounded-full shrink-0 text-muted-foreground hover:bg-muted/40"
            onClick={() => setSource(null)}
          >
            <X className="size-4" />
          </Button>
        </div>
      </Card>
    );
  }

  return (
    <Card
      className={cn(
        "flex flex-col overflow-hidden transition-all duration-300",
        "max-md:rounded-none max-md:shadow-none max-md:border-x-0 max-md:border-b-0 max-md:border-border/40 max-md:bg-sidebar/95 max-md:backdrop-blur-md supports-[backdrop-filter]:max-md:bg-sidebar/80",
        "md:h-full md:border-0 md:bg-transparent md:shadow-none md:rounded-none",
      )}
    >
      <CardHeader className="shrink-0 max-md:py-2.5 max-md:px-4 max-md:border-b max-md:border-border/20 flex-row justify-between items-center space-y-0 md:p-0 md:pb-4">
        <CardTitle className="max-md:text-[10px] text-xs uppercase tracking-widest text-muted-foreground/80 font-bold">
          Now Playing
        </CardTitle>
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 rounded-full shrink-0 text-muted-foreground hover:bg-muted/40"
            onClick={() => setMinimized(true)}
          >
            <ChevronDown aria-hidden="true" className="size-[18px]" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 md:hidden rounded-full shrink-0 text-muted-foreground hover:bg-muted/40"
            onClick={() => setSource(null)}
          >
            <X aria-hidden="true" className="size-[18px]" />
          </Button>
        </div>
      </CardHeader>
      <CardContent className="flex flex-1 overflow-y-auto flex-col gap-6 max-md:gap-4 max-md:p-4 md:p-0">
        {source ? (
          <div className="flex w-full flex-col gap-5 max-md:gap-4">
            <div className="hidden md:flex flex-wrap items-center gap-2 mt-1">
              <Badge
                variant={source.live ? "live" : "outline"}
                className="rounded-md px-2 py-0.5 text-xs font-bold uppercase tracking-wider"
              >
                {source.live ? "Live" : "Archive"}
              </Badge>
              <Badge
                variant="outline"
                className="rounded-md px-2 py-0.5 text-xs font-bold uppercase tracking-wider"
              >
                {source.kind}
              </Badge>
              {source.expiresAt ? (
                <Badge variant="outline" className="rounded-md">
                  Expires {formatRelativeTime(source.expiresAt)}
                </Badge>
              ) : null}
            </div>

            <div className="flex flex-col gap-2">
              <h2 className="text-[15px] font-bold tracking-tight text-foreground/95 md:text-xl">
                {source.title}
              </h2>
              <div className="flex flex-wrap items-center gap-2 md:hidden">
                <Badge
                  variant={source.live ? "live" : "outline"}
                  className="text-[10px] px-2 py-0.5 uppercase tracking-wider font-bold"
                >
                  {source.live ? "Live" : "Archive"}
                </Badge>
                <Badge
                  variant="outline"
                  className="text-[10px] px-2 py-0.5 uppercase tracking-wider font-bold bg-muted/20"
                >
                  {source.kind}
                </Badge>
                {source.expiresAt ? (
                  <Badge variant="outline" className="text-[10px] px-2 py-0.5">
                    Expires {formatRelativeTime(source.expiresAt)}
                  </Badge>
                ) : null}
              </div>
            </div>

            {source.kind === "unsupported" ? (
              <div className="rounded-2xl border border-border/40 bg-muted/20 p-5 text-sm text-muted-foreground w-full">
                {source.unsupportedReason ??
                  "This provider stream is not browser-compatible in v1."}
              </div>
            ) : (
              <div className="euripus-plyr-shell euripus-plyr-shell--local overflow-hidden rounded-2xl border border-border/40 bg-black/90 w-full ring-1 ring-white/10 shadow-[0_8px_32px_rgba(0,0,0,0.5)]">
                <PlyrSurface
                  ariaLabel={`Playing ${source.title}`}
                  className="contents"
                  onRecoveryNeeded={handleRecoveryNeeded}
                  source={source}
                  uiMode="local"
                  videoClassName="euripus-plyr-media aspect-video w-full bg-black object-contain max-md:min-h-[220px]"
                />
              </div>
            )}
          </div>
        ) : (
          <Empty className="flex-1 mt-6 border-0 max-md:hidden rounded-2xl bg-muted/5">
            <EmptyHeader>
              <EmptyMedia
                variant="icon"
                className="text-muted-foreground/30 ring-0 shadow-none bg-background/50"
              >
                <Radio aria-hidden="true" className="size-6" />
              </EmptyMedia>
              <EmptyTitle className="text-sm font-semibold text-muted-foreground/60 mt-2">
                Choose a channel or program
              </EmptyTitle>
            </EmptyHeader>
          </Empty>
        )}
      </CardContent>
    </Card>
  );
}
