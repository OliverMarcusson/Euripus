import type { PlaybackSource } from "@euripus/shared";
import Plyr from "plyr";
import {
  AUTO_HLS_QUALITY,
  createIptvHls,
  isIptvHlsSupported,
  type HlsQualityOption,
  type HlsSession,
} from "@/lib/hls";
import {
  inferPlaybackOwnershipHint,
  logPlaybackDiagnostic,
} from "@/lib/playback-diagnostics";

type PlayerUiMode = "local" | "receiver";

type BoundPlaybackSession = {
  plyr: Plyr | null;
  destroy: () => void;
};

const LIVE_STALL_RECOVERY_DELAY_MS = 12_000;

const LOCAL_VOD_CONTROLS = [
  "play-large",
  "play",
  "progress",
  "current-time",
  "duration",
  "mute",
  "volume",
  "pip",
  "fullscreen",
] satisfies Plyr.Options["controls"];

const LOCAL_LIVE_CONTROLS = [
  "play-large",
  "play",
  "mute",
  "volume",
  "pip",
  "fullscreen",
] satisfies Plyr.Options["controls"];

function receiverControls(): Plyr.Options["controls"] {
  return [];
}

function hasQualityMenu(uiMode: PlayerUiMode, hlsSession?: HlsSession) {
  return uiMode === "local" && (hlsSession?.qualityOptions.length ?? 0) > 1;
}

function controlsForSource(
  source: PlaybackSource,
  uiMode: PlayerUiMode,
  { qualityMenuEnabled = false }: { qualityMenuEnabled?: boolean } = {},
) {
  if (uiMode === "receiver") {
    return receiverControls();
  }

  const baseControls = source.live ? LOCAL_LIVE_CONTROLS : LOCAL_VOD_CONTROLS;
  if (!qualityMenuEnabled) {
    return baseControls;
  }

  return [
    ...baseControls.slice(0, -2),
    "settings",
    ...baseControls.slice(-2),
  ] satisfies Plyr.Options["controls"];
}

function buildQualityLabels(options: HlsQualityOption[]) {
  const labels = options.reduce<Record<number, string>>(
    (result, option) => {
      result[option.value] = option.label;
      return result;
    },
    { [AUTO_HLS_QUALITY]: "Auto" },
  );

  return labels;
}

function getQualitySignature(uiMode: PlayerUiMode, hlsSession?: HlsSession) {
  if (!hasQualityMenu(uiMode, hlsSession)) {
    return "base";
  }

  return [
    AUTO_HLS_QUALITY,
    ...(hlsSession?.qualityOptions ?? []).map((option) => option.value),
  ].join(",");
}

function resetMediaElement(video: HTMLVideoElement) {
  video.removeAttribute("src");
  video.load();
  if (video.playbackRate !== 1) {
    video.playbackRate = 1;
  }
}

function applyPlayerClasses(
  video: HTMLVideoElement,
  source: PlaybackSource,
  uiMode: PlayerUiMode,
  qualityMenuEnabled: boolean,
) {
  const container = video.closest(".plyr");
  if (!container) {
    return;
  }

  container.classList.add(
    "euripus-plyr",
    `euripus-plyr--${uiMode}`,
    source.live ? "euripus-plyr--live" : "euripus-plyr--vod",
  );
  container.classList.toggle("euripus-plyr--abr", qualityMenuEnabled);
}

function createPlyrInstance(
  video: HTMLVideoElement,
  source: PlaybackSource,
  uiMode: PlayerUiMode,
  hlsSession?: HlsSession,
) {
  const qualityMenuEnabled = hasQualityMenu(uiMode, hlsSession);
  video.disablePictureInPicture = uiMode !== "local";
  const qualityOptions = hlsSession?.qualityOptions ?? [];
  const qualityConfig: Plyr.Options["quality"] = {
    default: qualityMenuEnabled
      ? (hlsSession?.getCurrentQuality() ?? AUTO_HLS_QUALITY)
      : 576,
    forced: qualityMenuEnabled,
    onChange: qualityMenuEnabled
      ? (quality) => {
          hlsSession?.setQuality(quality);
        }
      : undefined,
    options: qualityMenuEnabled
      ? [AUTO_HLS_QUALITY, ...qualityOptions.map((option) => option.value)]
      : [576],
  };

  const plyr = new Plyr(video, {
    autoplay: true,
    clickToPlay: uiMode === "local",
    controls: controlsForSource(source, uiMode, { qualityMenuEnabled }),
    disableContextMenu: true,
    fullscreen: {
      enabled: uiMode === "local",
      iosNative: true,
    },
    i18n: {
      pip: "Picture-in-Picture",
      ...(qualityMenuEnabled
        ? { qualityLabel: buildQualityLabels(qualityOptions) }
        : {}),
    },
    keyboard: {
      focused: uiMode === "local",
      global: false,
    },
    quality: qualityConfig,
    settings: qualityMenuEnabled ? ["quality"] : [],
    tooltips: {
      controls: uiMode === "local",
      seek: false,
    },
  });

  applyPlayerClasses(video, source, uiMode, qualityMenuEnabled);
  void video.play().catch(() => undefined);

  return plyr;
}

export function bindPlaybackSource(
  video: HTMLVideoElement,
  source: PlaybackSource,
  {
    playbackSessionId,
    uiMode = "local",
    onRecoveryNeeded,
  }: {
    playbackSessionId?: string;
    uiMode?: PlayerUiMode;
    onRecoveryNeeded?: () => void | Promise<void>;
  } = {},
): BoundPlaybackSession {
  resetMediaElement(video);

  let destroyed = false;
  let recoveryInFlight = false;
  let hlsSession: HlsSession | undefined;
  let qualitySignature = "";
  let unsubscribeFromQualities: (() => void) | undefined;
  let stallRecoveryTimeout: number | undefined;

  const clearStallRecovery = () => {
    if (stallRecoveryTimeout != null) {
      window.clearTimeout(stallRecoveryTimeout);
      stallRecoveryTimeout = undefined;
    }
  };

  const triggerRecovery = () => {
    if (destroyed || recoveryInFlight || !onRecoveryNeeded) {
      return;
    }

    logPlaybackDiagnostic("warn", "playback-session-recovery-requested", {
      playbackSessionId,
      ownershipHint: inferPlaybackOwnershipHint(source.url),
      sourceKind: source.kind,
      sourceUrl: source.url,
      live: source.live,
      currentTime: video.currentTime,
      readyState: video.readyState,
    });

    recoveryInFlight = true;
    void Promise.resolve(onRecoveryNeeded()).finally(() => {
      recoveryInFlight = false;
    });
  };

  const scheduleStallRecovery = () => {
    if (!source.live || !onRecoveryNeeded || video.paused || video.ended) {
      return;
    }

    logPlaybackDiagnostic("warn", "video-stall-detected", {
      playbackSessionId,
      ownershipHint: inferPlaybackOwnershipHint(source.url),
      sourceKind: source.kind,
      sourceUrl: source.url,
      currentTime: video.currentTime,
      readyState: video.readyState,
    });
    clearStallRecovery();
    stallRecoveryTimeout = window.setTimeout(() => {
      stallRecoveryTimeout = undefined;
      if (destroyed || video.paused || video.ended) {
        return;
      }

      const bufferedEnd = video.buffered.length
        ? video.buffered.end(video.buffered.length - 1)
        : video.currentTime;
      const forwardBufferSeconds = bufferedEnd - video.currentTime;
      if (
        video.readyState >= HTMLMediaElement.HAVE_FUTURE_DATA ||
        forwardBufferSeconds > 1
      ) {
        return;
      }

      logPlaybackDiagnostic("warn", "video-stall-recovery-requested", {
        playbackSessionId,
        ownershipHint: inferPlaybackOwnershipHint(source.url),
        sourceKind: source.kind,
        sourceUrl: source.url,
        currentTime: video.currentTime,
        readyState: video.readyState,
        forwardBufferSeconds,
      });
      triggerRecovery();
    }, LIVE_STALL_RECOVERY_DELAY_MS);
  };

  const handlePlaybackProgress = () => {
    clearStallRecovery();
  };

  const handlePlaybackStall = () => {
    scheduleStallRecovery();
  };

  const handlePlaybackError = () => {
    logPlaybackDiagnostic("error", "video-element-error", {
      playbackSessionId,
      ownershipHint: inferPlaybackOwnershipHint(source.url),
      sourceKind: source.kind,
      sourceUrl: source.url,
      currentTime: video.currentTime,
      readyState: video.readyState,
      mediaErrorCode: video.error?.code ?? null,
      mediaErrorMessage: video.error?.message ?? null,
    });
    triggerRecovery();
  };

  const handlePlaybackPause = () => {
    clearStallRecovery();
  };

  const handlePlaybackEnded = () => {
    if (source.live) {
      logPlaybackDiagnostic("warn", "live-video-ended-unexpectedly", {
        playbackSessionId,
        ownershipHint: inferPlaybackOwnershipHint(source.url),
        sourceKind: source.kind,
        sourceUrl: source.url,
        currentTime: video.currentTime,
      });
      triggerRecovery();
    }
  };

  video.addEventListener("playing", handlePlaybackProgress);
  video.addEventListener("progress", handlePlaybackProgress);
  video.addEventListener("timeupdate", handlePlaybackProgress);
  video.addEventListener("loadeddata", handlePlaybackProgress);
  video.addEventListener("pause", handlePlaybackPause);
  video.addEventListener("waiting", handlePlaybackStall);
  video.addEventListener("stalled", handlePlaybackStall);
  video.addEventListener("error", handlePlaybackError);
  video.addEventListener("ended", handlePlaybackEnded);

  logPlaybackDiagnostic("info", "playback-session-created", {
    playbackSessionId,
    ownershipHint: inferPlaybackOwnershipHint(source.url),
    sourceKind: source.kind,
    sourceUrl: source.url,
    live: source.live,
    uiMode,
  });

  const session: BoundPlaybackSession = {
    plyr: null,
    destroy() {
      destroyed = true;
      logPlaybackDiagnostic("info", "playback-session-destroyed", {
        playbackSessionId,
        ownershipHint: inferPlaybackOwnershipHint(source.url),
        sourceKind: source.kind,
        sourceUrl: source.url,
        live: source.live,
        currentTime: video.currentTime,
        readyState: video.readyState,
      });
      clearStallRecovery();
      unsubscribeFromQualities?.();
      hlsSession?.destroy();
      video.removeEventListener("playing", handlePlaybackProgress);
      video.removeEventListener("progress", handlePlaybackProgress);
      video.removeEventListener("timeupdate", handlePlaybackProgress);
      video.removeEventListener("loadeddata", handlePlaybackProgress);
      video.removeEventListener("pause", handlePlaybackPause);
      video.removeEventListener("waiting", handlePlaybackStall);
      video.removeEventListener("stalled", handlePlaybackStall);
      video.removeEventListener("error", handlePlaybackError);
      video.removeEventListener("ended", handlePlaybackEnded);
      resetMediaElement(video);
      session.plyr?.destroy();
      session.plyr = null;
    },
  };

  const syncPlyr = () => {
    if (destroyed) {
      return;
    }

    const nextSignature = getQualitySignature(uiMode, hlsSession);
    if (session.plyr && nextSignature === qualitySignature) {
      return;
    }

    session.plyr?.destroy();
    session.plyr = createPlyrInstance(video, source, uiMode, hlsSession);
    qualitySignature = nextSignature;
  };

  if (source.kind === "hls" && isIptvHlsSupported()) {
    hlsSession = createIptvHls(video, source.url, {
      live: source.live,
      playbackSessionId,
      onRecoveryNeeded: triggerRecovery,
    });
    unsubscribeFromQualities = hlsSession.onQualitiesChanged(() => {
      syncPlyr();
    });
  } else {
    video.src = source.url;
    syncPlyr();
  }

  if (!session.plyr) {
    syncPlyr();
  }

  return session;
}
