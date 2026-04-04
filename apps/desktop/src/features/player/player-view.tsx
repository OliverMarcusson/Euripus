import { useEffect, useRef } from "react";
import Hls from "hls.js";
import { Radio } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { useTvAutoFocus } from "@/hooks/use-tv-auto-focus";
import { usePlayerStore } from "@/store/player-store";
import { formatRelativeTime } from "@/lib/utils";

export function PlayerView() {
  const source = usePlayerStore((state) => state.source);
  const setSource = usePlayerStore((state) => state.setSource);
  const videoRef = useRef<HTMLVideoElement | null>(null);
  useTvAutoFocus(source ? "[data-player-clear='true']" : null, [source?.title]);

  useEffect(() => {
    const video = videoRef.current;
    if (!video || !source || source.kind === "unsupported") {
      return;
    }

    video.removeAttribute("src");
    video.load();

    if (source.kind === "hls" && Hls.isSupported()) {
      const hls = new Hls();
      hls.loadSource(source.url);
      hls.attachMedia(video);
      return () => hls.destroy();
    }

    video.src = source.url;
    return () => {
      video.removeAttribute("src");
      video.load();
    };
  }, [source]);

  return (
    <Card className="h-full">
      <CardHeader>
        <CardTitle>Now Playing</CardTitle>
      </CardHeader>
      <CardContent className="flex h-full flex-col gap-5">
        {source ? (
          <>
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant={source.live ? "live" : "outline"}>{source.live ? "Live" : "Archive"}</Badge>
              <Badge variant="outline">{source.kind.toUpperCase()}</Badge>
              {source.expiresAt ? <Badge variant="outline">Expires {formatRelativeTime(source.expiresAt)}</Badge> : null}
            </div>

            <div className="flex flex-col gap-1.5">
              <h2 className="text-lg font-semibold">{source.title}</h2>
            </div>

            {source.kind === "unsupported" ? (
              <div className="rounded-2xl border border-border/70 bg-muted/40 p-4 text-sm text-muted-foreground">
                {source.unsupportedReason ?? "This provider stream is not browser-compatible in v1."}
              </div>
            ) : (
              <div className="overflow-hidden rounded-2xl border border-border/70 bg-black">
                <video
                  ref={videoRef}
                  controls
                  autoPlay
                  tabIndex={0}
                  data-tv-focusable="true"
                  className="aspect-video w-full"
                  aria-label={`Playing ${source.title}`}
                />
              </div>
            )}

            <Button data-player-clear="true" data-tv-autofocus="true" variant="ghost" onClick={() => setSource(null)}>
              Clear player
            </Button>
          </>
        ) : (
            <Empty className="min-h-[320px] border-0">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <Radio aria-hidden="true" />
                </EmptyMedia>
                <EmptyTitle>Choose a channel or program</EmptyTitle>
              </EmptyHeader>
            </Empty>
        )}
      </CardContent>
    </Card>
  );
}
