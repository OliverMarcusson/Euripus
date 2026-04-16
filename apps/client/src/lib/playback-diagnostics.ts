type PlaybackLogLevel = "info" | "warn" | "error";

type PlaybackDiagnosticPayload = Record<string, unknown>;

type PlaybackOwnershipHint =
  | "client-player"
  | "euripus-relay"
  | "iptv-provider"
  | "unknown";

function playbackDiagnosticsEnabled() {
  if (import.meta.env.DEV) {
    return true;
  }

  if (typeof window === "undefined") {
    return false;
  }

  try {
    return window.localStorage.getItem("euripus:playback-debug") === "1";
  } catch {
    return false;
  }
}

function isRelayUrl(value: string | null | undefined) {
  if (!value) {
    return false;
  }

  try {
    const url = new URL(value, window.location.origin);
    return url.origin === window.location.origin && url.pathname.startsWith("/api/relay/");
  } catch {
    return false;
  }
}

export function inferPlaybackOwnershipHint(url: string | null | undefined): PlaybackOwnershipHint {
  if (isRelayUrl(url)) {
    return "euripus-relay";
  }

  if (url) {
    return "iptv-provider";
  }

  return "unknown";
}

export function logPlaybackDiagnostic(
  level: PlaybackLogLevel,
  event: string,
  payload: PlaybackDiagnosticPayload,
) {
  if (!playbackDiagnosticsEnabled()) {
    return;
  }

  const logger =
    level === "error"
      ? console.error
      : level === "warn"
        ? console.warn
        : console.info;

  logger("[playback]", {
    event,
    timestamp: new Date().toISOString(),
    ...payload,
  });
}

export function attachPlaybackSeekDebugging(
  container: HTMLElement,
  video: HTMLVideoElement,
  {
    playbackSessionId,
    sourceKind,
    sourceUrl,
    live,
  }: {
    playbackSessionId?: string;
    sourceKind: string;
    sourceUrl: string;
    live: boolean;
  },
) {
  if (!playbackDiagnosticsEnabled()) {
    return () => undefined;
  }

  const ownershipHint = inferPlaybackOwnershipHint(sourceUrl);
  let lastUserInputAt: number | null = null;
  let lastUserInputType: string | null = null;
  let lastObservedTime = video.currentTime;

  const markUserInput = (event: Event) => {
    lastUserInputAt = performance.now();
    lastUserInputType = event.type;
  };

  const logSeekEvent = (eventName: "video-seeking" | "video-seeked") => {
    const now = performance.now();
    const recentUserInputMs =
      lastUserInputAt == null ? null : Math.round(now - lastUserInputAt);

    logPlaybackDiagnostic("warn", eventName, {
      playbackSessionId,
      ownershipHint,
      sourceKind,
      sourceUrl,
      live,
      currentTime: video.currentTime,
      readyState: video.readyState,
      seeking: video.seeking,
      recentUserInputMs,
      lastUserInputType,
      likelyUserInitiated:
        recentUserInputMs != null && recentUserInputMs >= 0 && recentUserInputMs < 1500,
    });
  };

  const handleSeeking = () => {
    logSeekEvent("video-seeking");
  };
  const handleSeeked = () => {
    logSeekEvent("video-seeked");
  };
  const handleTimeUpdate = () => {
    const currentTime = video.currentTime;
    const rewindDelta = lastObservedTime - currentTime;
    const recentUserInputMs =
      lastUserInputAt == null ? null : Math.round(performance.now() - lastUserInputAt);

    if (
      rewindDelta > 1 &&
      !video.seeking &&
      (recentUserInputMs == null || recentUserInputMs > 2000)
    ) {
      logPlaybackDiagnostic("warn", "video-current-time-regressed", {
        playbackSessionId,
        ownershipHint,
        sourceKind,
        sourceUrl,
        live,
        previousTime: lastObservedTime,
        currentTime,
        rewindDelta,
        readyState: video.readyState,
        recentUserInputMs,
        lastUserInputType,
      });
    }

    lastObservedTime = currentTime;
  };

  container.addEventListener("pointerdown", markUserInput, true);
  container.addEventListener("keydown", markUserInput, true);
  video.addEventListener("seeking", handleSeeking);
  video.addEventListener("seeked", handleSeeked);
  video.addEventListener("timeupdate", handleTimeUpdate);

  return () => {
    container.removeEventListener("pointerdown", markUserInput, true);
    container.removeEventListener("keydown", markUserInput, true);
    video.removeEventListener("seeking", handleSeeking);
    video.removeEventListener("seeked", handleSeeked);
    video.removeEventListener("timeupdate", handleTimeUpdate);
  };
}
