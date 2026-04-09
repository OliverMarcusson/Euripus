import { useEffect, useRef, type MutableRefObject } from "react";
import type { PlaybackSource } from "@euripus/shared";
import { bindPlaybackSource } from "@/lib/plyr-player";

type PlyrSurfaceProps = {
  ariaLabel: string;
  className?: string;
  source: PlaybackSource;
  uiMode: "local" | "receiver";
  videoClassName?: string;
  videoRef?: MutableRefObject<HTMLVideoElement | null>;
};

export function PlyrSurface({
  ariaLabel,
  className,
  source,
  uiMode,
  videoClassName,
  videoRef,
}: PlyrSurfaceProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);

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

    const session = bindPlaybackSource(video, source, { uiMode });
    return () => {
      session.destroy();
      if (videoRef?.current === video) {
        videoRef.current = null;
      }
      container.replaceChildren();
    };
  }, [ariaLabel, source, uiMode, videoClassName, videoRef]);

  return <div ref={containerRef} className={className} />;
}
