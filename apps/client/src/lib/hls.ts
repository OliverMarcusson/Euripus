import Hls, { type ErrorData, type HlsConfig, type Level } from "hls.js";

export const IPTV_HLS_CONFIG = {
  lowLatencyMode: false,
  liveSyncDurationCount: 2,
  liveMaxLatencyDurationCount: 4,
  maxBufferLength: 10,
  backBufferLength: 16,
  manifestLoadingTimeOut: 12_000,
  fragLoadingTimeOut: 20_000,
} satisfies Partial<HlsConfig>;

const LIVE_EDGE_HARD_RESYNC_SECONDS = 4;
const LIVE_EDGE_PLAYBACK_RATE_DRIFT_SECONDS = 1.5;
const LIVE_EDGE_PLAYBACK_RATE_FAST_DRIFT_SECONDS = 4;
const LIVE_EDGE_MIN_FORWARD_BUFFER_SECONDS = 1.5;
const LIVE_EDGE_SNAP_BACKOFF_SECONDS = 0.5;

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

export function handleIptvHlsError(
  hls: HlsErrorController,
  data: ErrorData,
  recoveryState: HlsErrorRecoveryState,
) {
  if (data.fatal) {
    hls.destroy();
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

    hls.destroy();
  }
}

function getBufferedEnd(video: HTMLVideoElement) {
  return video.buffered.length
    ? video.buffered.end(video.buffered.length - 1)
    : null;
}

export function syncLivePlaybackPosition(
  video: HTMLVideoElement,
  hls: HlsLiveSyncController,
  { force = false }: { force?: boolean } = {},
) {
  const liveSyncPosition = hls.liveSyncPosition;
  if (liveSyncPosition == null || !Number.isFinite(liveSyncPosition)) {
    return;
  }

  const driftSeconds = liveSyncPosition - video.currentTime;
  if (!force && driftSeconds <= LIVE_EDGE_HARD_RESYNC_SECONDS) {
    return;
  }

  const nextTime = Math.max(
    0,
    liveSyncPosition - LIVE_EDGE_SNAP_BACKOFF_SECONDS,
  );
  if (Math.abs(video.currentTime - nextTime) < 0.25) {
    return;
  }

  video.currentTime = nextTime;
}

export function updateLivePlaybackRate(
  video: HTMLVideoElement,
  hls: HlsLiveSyncController,
) {
  const liveSyncPosition = hls.liveSyncPosition;
  if (liveSyncPosition == null || !Number.isFinite(liveSyncPosition)) {
    if (video.playbackRate !== 1) {
      video.playbackRate = 1;
    }
    return;
  }

  const bufferedEnd = getBufferedEnd(video);
  const forwardBufferSeconds =
    bufferedEnd == null ? 0 : bufferedEnd - video.currentTime;
  const driftSeconds = liveSyncPosition - video.currentTime;

  if (
    !video.paused &&
    driftSeconds > LIVE_EDGE_PLAYBACK_RATE_DRIFT_SECONDS &&
    forwardBufferSeconds > LIVE_EDGE_MIN_FORWARD_BUFFER_SECONDS
  ) {
    video.playbackRate =
      driftSeconds > LIVE_EDGE_PLAYBACK_RATE_FAST_DRIFT_SECONDS ? 1.1 : 1.05;
    return;
  }

  if (video.playbackRate !== 1) {
    video.playbackRate = 1;
  }
}

export function createIptvHls(
  video: HTMLVideoElement,
  sourceUrl: string,
  { live = false }: { live?: boolean } = {},
): HlsSession {
  const hls = new Hls(IPTV_HLS_CONFIG);
  const recoveryState: HlsErrorRecoveryState = { mediaRecoveryAttempts: 0 };
  const qualityListeners = new Set<(options: HlsQualityOption[]) => void>();
  let currentQuality = AUTO_HLS_QUALITY;
  let qualityOptions = getIptvHlsQualityOptions(hls.levels);

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
    handleIptvHlsError(hls, data, recoveryState);
  };

  const handleLiveUpdate = () => {
    if (!live) {
      return;
    }
    syncLivePlaybackPosition(video, hls);
    updateLivePlaybackRate(video, hls);
  };

  const handleLiveSeek = () => {
    if (!live) {
      return;
    }
    queueMicrotask(() => {
      syncLivePlaybackPosition(video, hls, { force: true });
      updateLivePlaybackRate(video, hls);
    });
  };

  const handlePause = () => {
    if (video.playbackRate !== 1) {
      video.playbackRate = 1;
    }
  };

  const handleManifestParsed = () => {
    publishQualityOptions();
    if (!live) {
      return;
    }

    syncLivePlaybackPosition(video, hls, { force: true });
    updateLivePlaybackRate(video, hls);
  };

  const handleLevelsUpdated = () => {
    publishQualityOptions();
  };

  hls.on(Hls.Events.ERROR, handleError);
  hls.on(Hls.Events.MANIFEST_PARSED, handleManifestParsed);
  hls.on(Hls.Events.LEVELS_UPDATED, handleLevelsUpdated);

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
      qualityListeners.clear();
      hls.off(Hls.Events.ERROR, handleError);
      hls.off(Hls.Events.MANIFEST_PARSED, handleManifestParsed);
      hls.off(Hls.Events.LEVELS_UPDATED, handleLevelsUpdated);
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
