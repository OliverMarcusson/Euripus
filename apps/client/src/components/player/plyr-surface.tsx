import { useEffect, useRef, type MutableRefObject } from "react";
import type { PlaybackSource } from "@euripus/shared";
import { bindPlaybackSource } from "@/lib/plyr-player";
import type { PlaybackFailure } from "@/lib/hls";
import { attachPlaybackSeekDebugging } from "@/lib/playback-diagnostics";
import { usePlaybackSettingsStore } from "@/store/playback-settings-store";

let nextPlaybackSessionSequence = 0;

function createPlaybackSessionId() {
  nextPlaybackSessionSequence += 1;
  return `playback-session-${nextPlaybackSessionSequence}`;
}

type PlyrSurfaceProps = {
  ariaLabel: string;
  className?: string;
  onPlaybackFailure?: (failure: PlaybackFailure) => void | Promise<void>;
  onPlaybackHealthy?: () => void;
  source: PlaybackSource;
  uiMode: "local" | "receiver";
  videoClassName?: string;
  videoRef?: MutableRefObject<HTMLVideoElement | null>;
};

export function PlyrSurface({
  ariaLabel,
  className,
  onPlaybackFailure,
  onPlaybackHealthy,
  source,
  uiMode,
  videoClassName,
  videoRef,
}: PlyrSurfaceProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const livePlaybackPreference = usePlaybackSettingsStore(
    (state) => state.livePlaybackPreference,
  );
  const onPlaybackFailureRef = useRef<typeof onPlaybackFailure>(onPlaybackFailure);
  const onPlaybackHealthyRef = useRef<typeof onPlaybackHealthy>(onPlaybackHealthy);

  useEffect(() => {
    onPlaybackFailureRef.current = onPlaybackFailure;
    onPlaybackHealthyRef.current = onPlaybackHealthy;
  }, [onPlaybackFailure, onPlaybackHealthy]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container || source.kind === "unsupported") {
      return;
    }

    const video = document.createElement("video");
    video.autoplay = true;
    video.playsInline = true;
    video.ariaLabel = ariaLabel;
    if (uiMode === "local") {
      video.tabIndex = 0;
    }
    if (videoClassName) {
      video.className = videoClassName;
    }

    container.replaceChildren(video);
    if (videoRef) {
      videoRef.current = video;
    }

    const playbackSessionId = createPlaybackSessionId();
    const detachSeekDebugging = attachPlaybackSeekDebugging(container, video, {
      playbackSessionId,
      sourceKind: source.kind,
      sourceUrl: source.url,
      live: source.live,
    });
    const session = bindPlaybackSource(video, source, {
      playbackSessionId,
      uiMode,
      livePlaybackPreference,
      onPlaybackFailure: (failure) => onPlaybackFailureRef.current?.(failure),
      onPlaybackHealthy: () => onPlaybackHealthyRef.current?.(),
    });
    return () => {
      detachSeekDebugging();
      session.destroy();
      if (videoRef?.current === video) {
        videoRef.current = null;
      }
      container.replaceChildren();
    };
  }, [
    ariaLabel,
    livePlaybackPreference,
    source,
    uiMode,
    videoClassName,
    videoRef,
  ]);

  return <div ref={containerRef} className={className} />;
}
