import Hls, { type ErrorData, type HlsConfig, type Level } from "hls.js";
import {
  inferPlaybackOwnershipHint,
  logPlaybackDiagnostic,
  sanitizePlaybackDiagnosticUrl,
} from "@/lib/playback-diagnostics";

export type LivePlaybackPreference = "stable" | "low-latency";

export const IPTV_HLS_CONFIG = {
  lowLatencyMode: false,
  liveSyncDurationCount: 10,
  liveMaxLatencyDurationCount: 20,
  maxBufferLength: 60,
  backBufferLength: 90,
  nudgeOnVideoHole: true,
  manifestLoadingTimeOut: 15_000,
  fragLoadingTimeOut: 25_000,
} satisfies Partial<HlsConfig>;

export const LOW_LATENCY_IPTV_HLS_CONFIG = {
  ...IPTV_HLS_CONFIG,
  lowLatencyMode: true,
  liveSyncDurationCount: 4,
  liveMaxLatencyDurationCount: 12,
  maxBufferLength: 45,
} satisfies Partial<HlsConfig>;

export function getIptvHlsConfig(
  livePlaybackPreference: LivePlaybackPreference = "stable",
) {
  return livePlaybackPreference === "low-latency"
    ? LOW_LATENCY_IPTV_HLS_CONFIG
    : IPTV_HLS_CONFIG;
}

type HlsErrorRecoveryState = {
  mediaRecoveryAttempts: number;
};

export const AUTO_HLS_QUALITY = 0;

export type HlsQualityOption = {
  value: number;
  label: string;
  level: number;
  bitrate: number;
};

type HlsErrorController = Pick<Hls, "destroy" | "recoverMediaError" | "startLoad">;
type HlsLiveSyncController = Pick<Hls, "liveSyncPosition">;
export type PlaybackFailure =
  | {
      kind: "recoverable";
      reason: "codec" | "hls" | "video-error" | "stall" | "unexpected-end";
    }
  | {
      kind: "provider-unavailable";
      message: string;
    };

export type HlsSession = {
  readonly qualityOptions: HlsQualityOption[];
  destroy: () => void;
  getCurrentQuality: () => number;
  onQualitiesChanged: (
    listener: (options: HlsQualityOption[]) => void,
  ) => () => void;
  setQuality: (quality: number) => void;
};

function getQualityValue(level: Pick<Level, "height" | "bitrate">) {
  if (typeof level.height === "number" && Number.isFinite(level.height) && level.height > 0) {
    return level.height;
  }

  const bitrateKbps = Math.round(level.bitrate / 1000);
  return Math.max(bitrateKbps, 1);
}

export function getIptvHlsQualityLabel(level: Pick<Level, "height" | "bitrate">) {
  if (typeof level.height === "number" && Number.isFinite(level.height) && level.height > 0) {
    return `${level.height}p`;
  }

  return `${Math.max(Math.round(level.bitrate / 1000), 1)} kbps`;
}

export function getIptvHlsQualityOptions(
  levels: Array<Pick<Level, "height" | "bitrate">>,
) {
  const optionsByValue = new Map<number, HlsQualityOption>();

  levels.forEach((level, index) => {
    const value = getQualityValue(level);
    const existing = optionsByValue.get(value);
    if (existing && existing.bitrate >= level.bitrate) {
      return;
    }

    optionsByValue.set(value, {
      value,
      label: getIptvHlsQualityLabel(level),
      level: index,
      bitrate: level.bitrate,
    });
  });

  return Array.from(optionsByValue.values()).sort((left, right) => right.value - left.value);
}

export function isIptvHlsSupported() {
  return Hls.isSupported();
}

export function isFatalHlsCodecError(
  data: Pick<ErrorData, "details" | "fatal">,
) {
  return data.fatal && data.details === Hls.ErrorDetails.BUFFER_ADD_CODEC_ERROR;
}

export function isProviderPlaceholderHlsError(
  sourceUrl: string,
  data: Pick<ErrorData, "fatal" | "response" | "type">,
) {
  if (
    !data.fatal ||
    data.type !== Hls.ErrorTypes.NETWORK_ERROR ||
    data.response?.code !== 460
  ) {
    return false;
  }

  try {
    return new URL(sourceUrl, window.location.origin).pathname === "/api/relay/hls";
  } catch {
    return false;
  }
}

export function handleIptvHlsError(
  hls: HlsErrorController,
  data: ErrorData,
  recoveryState: HlsErrorRecoveryState,
  {
    onFatalRecoveryNeeded,
  }: {
    onFatalRecoveryNeeded?: () => void;
  } = {},
) {
  if (data.fatal) {
    onFatalRecoveryNeeded?.();
    return;
  }

  if (data.type === Hls.ErrorTypes.NETWORK_ERROR) {
    hls.startLoad();
    return;
  }

  if (data.type === Hls.ErrorTypes.MEDIA_ERROR) {
    if (recoveryState.mediaRecoveryAttempts === 0) {
      recoveryState.mediaRecoveryAttempts += 1;
      hls.recoverMediaError();
      return;
    }

    onFatalRecoveryNeeded?.();
  }
}

function getBufferedEnd(video: HTMLVideoElement) {
  return video.buffered.length
    ? video.buffered.end(video.buffered.length - 1)
    : null;
}

function getBufferedRanges(video: HTMLVideoElement) {
  return Array.from({ length: video.buffered.length }, (_, index) => ({
    start: video.buffered.start(index),
    end: video.buffered.end(index),
  }));
}

function describeFrag(data: {
  frag?: {
    cc?: number;
    duration?: number;
    level?: number;
    sn?: number | "initSegment";
    start?: number;
    type?: string;
    url?: string;
  };
  part?: {
    duration?: number;
    index?: number;
  } | null;
}) {
  return {
    fragSn: data.frag?.sn ?? null,
    fragLevel: data.frag?.level ?? null,
    fragCc: data.frag?.cc ?? null,
    fragStart: data.frag?.start ?? null,
    fragDuration: data.frag?.duration ?? null,
    fragType: data.frag?.type ?? null,
    fragUrl: data.frag?.url
      ? sanitizePlaybackDiagnosticUrl(data.frag.url)
      : null,
    partIndex: data.part?.index ?? null,
    partDuration: data.part?.duration ?? null,
  };
}

export function syncLivePlaybackPosition(
  _video: HTMLVideoElement,
  _hls: HlsLiveSyncController,
) {
  // Avoid snapping live playback forward automatically. In practice this
  // proved too aggressive and made jittery live streams feel worse.
}

const LIVE_CATCH_UP_PLAYBACK_RATE = 1.02;
const LIVE_CATCH_UP_START_THRESHOLD_SECONDS = 2;
const LIVE_CATCH_UP_STOP_THRESHOLD_SECONDS = 0.5;
const LIVE_CATCH_UP_MIN_FORWARD_BUFFER_SECONDS = 3;

export function updateLivePlaybackRate(
  video: HTMLVideoElement,
  hls: HlsLiveSyncController,
  livePlaybackPreference: LivePlaybackPreference = "stable",
) {
  const liveSyncPosition = hls.liveSyncPosition;
  if (
    livePlaybackPreference !== "low-latency" ||
    video.paused ||
    liveSyncPosition == null ||
    !Number.isFinite(liveSyncPosition)
  ) {
    if (video.playbackRate !== 1) {
      video.playbackRate = 1;
    }
    return;
  }

  const secondsBehind = liveSyncPosition - video.currentTime;
  const bufferedEnd = getBufferedEnd(video);
  const hasCatchUpBuffer =
    bufferedEnd != null &&
    bufferedEnd - video.currentTime > LIVE_CATCH_UP_MIN_FORWARD_BUFFER_SECONDS;
  const catchUpThreshold =
    video.playbackRate > 1
      ? LIVE_CATCH_UP_STOP_THRESHOLD_SECONDS
      : LIVE_CATCH_UP_START_THRESHOLD_SECONDS;
  const nextPlaybackRate =
    hasCatchUpBuffer && secondsBehind > catchUpThreshold
      ? LIVE_CATCH_UP_PLAYBACK_RATE
      : 1;

  if (video.playbackRate !== nextPlaybackRate) {
    video.playbackRate = nextPlaybackRate;
  }
}

export function createIptvHls(
  video: HTMLVideoElement,
  sourceUrl: string,
  {
    live = false,
    livePlaybackPreference = "stable",
    playbackSessionId,
    onPlaybackFailure,
  }: {
    live?: boolean;
    livePlaybackPreference?: LivePlaybackPreference;
    playbackSessionId?: string;
    onPlaybackFailure?: (failure: PlaybackFailure) => void;
  } = {},
): HlsSession {
  const hls = new Hls(
    getIptvHlsConfig(live ? livePlaybackPreference : "stable"),
  );
  const recoveryState: HlsErrorRecoveryState = { mediaRecoveryAttempts: 0 };
  const diagnosticSourceUrl = sanitizePlaybackDiagnosticUrl(sourceUrl);
  const qualityListeners = new Set<(options: HlsQualityOption[]) => void>();
  let currentQuality = AUTO_HLS_QUALITY;
  let qualityOptions = getIptvHlsQualityOptions(hls.levels);
  let previousFrag:
    | {
        cc: number | null;
        sn: number | "initSegment" | null;
        start: number | null;
      }
    | null = null;

  const notifyQualityListeners = () => {
    qualityListeners.forEach((listener) => {
      listener(qualityOptions);
    });
  };

  const publishQualityOptions = () => {
    qualityOptions = getIptvHlsQualityOptions(hls.levels);
    if (
      currentQuality !== AUTO_HLS_QUALITY &&
      !qualityOptions.some((option) => option.value === currentQuality)
    ) {
      currentQuality = AUTO_HLS_QUALITY;
    }
    notifyQualityListeners();
  };

  const handleError = (_event: string, data: ErrorData) => {
    const failingUrl =
      data.context?.url ??
      data.url ??
      sourceUrl;
    const ownershipHint = inferPlaybackOwnershipHint(failingUrl);
    const responseCode =
      data.response?.code;

    logPlaybackDiagnostic(data.fatal ? "error" : "warn", "hls-error", {
      playbackSessionId,
      ownershipHint,
      sourceUrl: diagnosticSourceUrl,
      failingUrl: sanitizePlaybackDiagnosticUrl(failingUrl),
      live,
      fatal: data.fatal,
      errorType: data.type,
      errorDetails: data.details,
      errorMessage: data.error?.message ?? null,
      mimeType: data.mimeType ?? null,
      sourceBufferName: data.sourceBufferName ?? null,
      responseCode,
      mediaErrorRecoveryAttempts: recoveryState.mediaRecoveryAttempts,
      currentTime: video.currentTime,
      bufferedRanges: getBufferedRanges(video),
      readyState: video.readyState,
      paused: video.paused,
      liveSyncPosition: hls.liveSyncPosition,
    });

    if (isProviderPlaceholderHlsError(sourceUrl, data)) {
      logPlaybackDiagnostic("error", "provider-placeholder-detected", {
        playbackSessionId,
        ownershipHint: "euripus-relay",
        live,
        responseCode,
      });
      onPlaybackFailure?.({
        kind: "provider-unavailable",
        message: "This channel is currently unavailable from the provider.",
      });
      return;
    }

    if (isFatalHlsCodecError(data)) {
      onPlaybackFailure?.({ kind: "recoverable", reason: "codec" });
      return;
    }

    handleIptvHlsError(hls, data, recoveryState, {
      onFatalRecoveryNeeded: () => {
        onPlaybackFailure?.({ kind: "recoverable", reason: "hls" });
      },
    });
  };

  const handleLiveUpdate = () => {
    if (!live) {
      return;
    }
    syncLivePlaybackPosition(video, hls);
    updateLivePlaybackRate(video, hls, livePlaybackPreference);
  };

  const handleLiveSeek = () => {
    if (!live) {
      return;
    }
    queueMicrotask(() => {
      updateLivePlaybackRate(video, hls, livePlaybackPreference);
    });
  };

  const handlePause = () => {
    if (video.playbackRate !== 1) {
      video.playbackRate = 1;
    }
  };

  const handleManifestParsed = () => {
    logPlaybackDiagnostic("info", "hls-manifest-parsed", {
      playbackSessionId,
      ownershipHint: inferPlaybackOwnershipHint(sourceUrl),
      sourceUrl: diagnosticSourceUrl,
      live,
      levelCount: hls.levels.length,
    });
    publishQualityOptions();
    if (!live) {
      return;
    }

    updateLivePlaybackRate(video, hls, livePlaybackPreference);
  };

  const handleLevelsUpdated = () => {
    publishQualityOptions();
  };

  const logFragEvent = (
    eventName:
      | "hls-frag-loading"
      | "hls-frag-loaded"
      | "hls-frag-buffered"
      | "hls-frag-changed"
      | "hls-level-switching"
      | "hls-level-switched"
      | "hls-buffer-flushing"
      | "hls-buffer-flushed"
      | "hls-frag-load-emergency-aborted",
    data: {
      frag?: {
        cc?: number;
        duration?: number;
        level?: number;
        sn?: number | "initSegment";
        start?: number;
        type?: string;
        url?: string;
      };
      part?: {
        duration?: number;
        index?: number;
      } | null;
      level?: number;
      stats?: Record<string, number | undefined>;
      endOffset?: number;
      startOffset?: number;
      type?: string;
    },
  ) => {
    if (!import.meta.env.DEV) {
      return;
    }

    const stats = data.stats;
    logPlaybackDiagnostic("info", eventName, {
      playbackSessionId,
      ownershipHint: inferPlaybackOwnershipHint(sourceUrl),
      sourceUrl: diagnosticSourceUrl,
      live,
      currentTime: video.currentTime,
      bufferedRanges: getBufferedRanges(video),
      ...describeFrag(data),
      level: data.level ?? null,
      startOffset: data.startOffset ?? null,
      endOffset: data.endOffset ?? null,
      bufferType: data.type ?? null,
      requestToFirstByteMs:
        typeof stats?.tfirst === "number" && typeof stats?.trequest === "number"
          ? stats.tfirst - stats.trequest
          : null,
      requestToLoadMs:
        typeof stats?.tload === "number" && typeof stats?.trequest === "number"
          ? stats.tload - stats.trequest
          : null,
    });
  };

  const handleFragLoading = (
    _event: string,
    data: any,
  ) => logFragEvent("hls-frag-loading", data);

  const handleFragLoaded = (
    _event: string,
    data: any,
  ) => logFragEvent("hls-frag-loaded", data);

  const handleFragBuffered = (
    _event: string,
    data: any,
  ) => logFragEvent("hls-frag-buffered", data);
  const handleLevelSwitching = (
    _event: string,
    data: any,
  ) => logFragEvent("hls-level-switching", data);
  const handleLevelSwitched = (
    _event: string,
    data: any,
  ) => logFragEvent("hls-level-switched", data);
  const handleBufferFlushing = (
    _event: string,
    data: any,
  ) => logFragEvent("hls-buffer-flushing", data);
  const handleBufferFlushed = (
    _event: string,
    data: any,
  ) => logFragEvent("hls-buffer-flushed", data);
  const handleFragLoadEmergencyAborted = (
    _event: string,
    data: any,
  ) => logFragEvent("hls-frag-load-emergency-aborted", data);

  const handleFragChanged = (
    _event: string,
    data: any,
  ) => {
    logFragEvent("hls-frag-changed", data);

    const nextFrag = {
      cc: typeof data?.frag?.cc === "number" ? data.frag.cc : null,
      sn:
        typeof data?.frag?.sn === "number" || data?.frag?.sn === "initSegment"
          ? data.frag.sn
          : null,
      start: typeof data?.frag?.start === "number" ? data.frag.start : null,
    };

    const repeatedFragment =
      previousFrag != null &&
      nextFrag.sn != null &&
      previousFrag.sn != null &&
      nextFrag.cc === previousFrag.cc &&
      nextFrag.sn === previousFrag.sn;
    const rewoundFragment =
      previousFrag != null &&
      typeof nextFrag.sn === "number" &&
      typeof previousFrag.sn === "number" &&
      nextFrag.cc === previousFrag.cc &&
      nextFrag.sn < previousFrag.sn;

    if (repeatedFragment || rewoundFragment) {
      logPlaybackDiagnostic("warn", "hls-frag-sequence-anomaly", {
        playbackSessionId,
        ownershipHint: inferPlaybackOwnershipHint(sourceUrl),
        sourceUrl: diagnosticSourceUrl,
        live,
        currentTime: video.currentTime,
        bufferedRanges: getBufferedRanges(video),
        previousFragSn: previousFrag?.sn ?? null,
        previousFragStart: previousFrag?.start ?? null,
        nextFragSn: nextFrag.sn,
        nextFragStart: nextFrag.start,
        anomaly: repeatedFragment ? "repeated-frag" : "rewound-frag",
      });
    }

    previousFrag = nextFrag;
  };

  hls.on(Hls.Events.ERROR, handleError);
  hls.on(Hls.Events.MANIFEST_PARSED, handleManifestParsed);
  hls.on(Hls.Events.LEVELS_UPDATED, handleLevelsUpdated);
  hls.on(Hls.Events.FRAG_LOADING, handleFragLoading);
  hls.on(Hls.Events.FRAG_LOADED, handleFragLoaded);
  hls.on(Hls.Events.FRAG_BUFFERED, handleFragBuffered);
  hls.on(Hls.Events.FRAG_CHANGED, handleFragChanged);
  hls.on(Hls.Events.LEVEL_SWITCHING, handleLevelSwitching);
  hls.on(Hls.Events.LEVEL_SWITCHED, handleLevelSwitched);
  hls.on(Hls.Events.BUFFER_FLUSHING, handleBufferFlushing);
  hls.on(Hls.Events.BUFFER_FLUSHED, handleBufferFlushed);
  hls.on(
    Hls.Events.FRAG_LOAD_EMERGENCY_ABORTED,
    handleFragLoadEmergencyAborted,
  );

  logPlaybackDiagnostic("info", "hls-session-created", {
    playbackSessionId,
    ownershipHint: inferPlaybackOwnershipHint(sourceUrl),
    sourceUrl: diagnosticSourceUrl,
    live,
  });

  if (live) {
    hls.on(Hls.Events.LEVEL_UPDATED, handleLiveUpdate);
    video.addEventListener("timeupdate", handleLiveUpdate);
    video.addEventListener("seeking", handleLiveSeek);
    video.addEventListener("pause", handlePause);
  }

  hls.loadSource(sourceUrl);
  hls.attachMedia(video);

  return {
    get qualityOptions() {
      return qualityOptions;
    },
    getCurrentQuality() {
      return currentQuality;
    },
    onQualitiesChanged(listener) {
      qualityListeners.add(listener);
      listener(qualityOptions);
      return () => {
        qualityListeners.delete(listener);
      };
    },
    setQuality(quality) {
      if (quality === AUTO_HLS_QUALITY) {
        currentQuality = AUTO_HLS_QUALITY;
        hls.currentLevel = -1;
        return;
      }

      const nextQuality = qualityOptions.find((option) => option.value === quality);
      if (!nextQuality) {
        return;
      }

      currentQuality = nextQuality.value;
      hls.currentLevel = nextQuality.level;
    },
    destroy() {
      logPlaybackDiagnostic("info", "hls-session-destroyed", {
        playbackSessionId,
        ownershipHint: inferPlaybackOwnershipHint(sourceUrl),
        sourceUrl: diagnosticSourceUrl,
        live,
        currentTime: video.currentTime,
        bufferedRanges: getBufferedRanges(video),
      });
      qualityListeners.clear();
      hls.off(Hls.Events.ERROR, handleError);
      hls.off(Hls.Events.MANIFEST_PARSED, handleManifestParsed);
      hls.off(Hls.Events.LEVELS_UPDATED, handleLevelsUpdated);
      hls.off(Hls.Events.FRAG_LOADING, handleFragLoading);
      hls.off(Hls.Events.FRAG_LOADED, handleFragLoaded);
      hls.off(Hls.Events.FRAG_BUFFERED, handleFragBuffered);
      hls.off(Hls.Events.FRAG_CHANGED, handleFragChanged);
      hls.off(Hls.Events.LEVEL_SWITCHING, handleLevelSwitching);
      hls.off(Hls.Events.LEVEL_SWITCHED, handleLevelSwitched);
      hls.off(Hls.Events.BUFFER_FLUSHING, handleBufferFlushing);
      hls.off(Hls.Events.BUFFER_FLUSHED, handleBufferFlushed);
      hls.off(
        Hls.Events.FRAG_LOAD_EMERGENCY_ABORTED,
        handleFragLoadEmergencyAborted,
      );
      if (live) {
        video.removeEventListener("timeupdate", handleLiveUpdate);
        video.removeEventListener("seeking", handleLiveSeek);
        video.removeEventListener("pause", handlePause);
        hls.off(Hls.Events.LEVEL_UPDATED, handleLiveUpdate);
      }
      if (video.playbackRate !== 1) {
        video.playbackRate = 1;
      }
      hls.destroy();
    },
  };
}
