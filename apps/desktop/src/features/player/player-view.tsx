import { useEffect, useRef } from "react";
import Hls from "hls.js";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { usePlayerStore } from "@/store/player-store";

export function PlayerView() {
  const source = usePlayerStore((state) => state.source);
  const setSource = usePlayerStore((state) => state.setSource);
  const videoRef = useRef<HTMLVideoElement | null>(null);

  useEffect(() => {
    const video = videoRef.current;
    if (!video || !source || source.kind === "unsupported") {
      return;
    }

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
    <Card className="h-full bg-transparent shadow-none">
      <CardHeader>
        <CardTitle>Now Playing</CardTitle>
        <CardDescription>Browser-compatible playback for live TV and catch-up.</CardDescription>
      </CardHeader>
      <CardContent className="flex h-full flex-col gap-4">
        {source ? (
          <>
            <div className="flex flex-wrap items-center gap-2">
              <Badge>{source.live ? "Live" : "Archive"}</Badge>
              <Badge>{source.kind.toUpperCase()}</Badge>
            </div>
            <h3 className="text-lg font-semibold">{source.title}</h3>
            {source.kind === "unsupported" ? (
              <div className="rounded-xl border border-border bg-card p-4 text-sm text-muted-foreground">
                {source.unsupportedReason ?? "This provider stream is not browser-compatible in v1."}
              </div>
            ) : (
              <video ref={videoRef} controls autoPlay className="aspect-video w-full rounded-xl bg-black" />
            )}
            <Button variant="ghost" onClick={() => setSource(null)}>
              Clear player
            </Button>
          </>
        ) : (
          <div className="flex h-full flex-col items-center justify-center rounded-2xl border border-dashed border-border bg-card/50 px-8 text-center">
            <p className="text-lg font-semibold">Choose a channel or program</p>
            <p className="mt-2 text-sm text-muted-foreground">Playback sources appear here after a guide, favorites, or search action.</p>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

