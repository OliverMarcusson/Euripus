import Hls, { type ErrorData, type HlsConfig } from "hls.js";

export const IPTV_HLS_CONFIG = {
  lowLatencyMode: false,
  liveSyncDurationCount: 1,
  liveMaxLatencyDurationCount: 2,
  maxBufferLength: 6,
  backBufferLength: 12,
  manifestLoadingTimeOut: 10_000,
  fragLoadingTimeOut: 15_000,
} satisfies Partial<HlsConfig>;

const LIVE_EDGE_HARD_RESYNC_SECONDS = 4;
const LIVE_EDGE_PLAYBACK_RATE_DRIFT_SECONDS = 1.5;
const LIVE_EDGE_PLAYBACK_RATE_FAST_DRIFT_SECONDS = 4;
const LIVE_EDGE_MIN_FORWARD_BUFFER_SECONDS = 1.5;
const LIVE_EDGE_SNAP_BACKOFF_SECONDS = 0.5;

type HlsErrorRecoveryState = {
  mediaRecoveryAttempts: number;
};

type HlsErrorController = Pick<Hls, "destroy" | "recoverMediaError" | "startLoad">;
type HlsLiveSyncController = Pick<Hls, "liveSyncPosition">;
type HlsSession = {
  destroy: () => void;
};

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

  hls.on(Hls.Events.ERROR, (_event, data) => {
    handleIptvHlsError(hls, data, recoveryState);
  });

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

  if (live) {
    hls.on(Hls.Events.MANIFEST_PARSED, () => {
      syncLivePlaybackPosition(video, hls, { force: true });
      updateLivePlaybackRate(video, hls);
    });
    hls.on(Hls.Events.LEVEL_UPDATED, handleLiveUpdate);
    video.addEventListener("timeupdate", handleLiveUpdate);
    video.addEventListener("seeking", handleLiveSeek);
    video.addEventListener("pause", handlePause);
  }

  hls.loadSource(sourceUrl);
  hls.attachMedia(video);

  return {
    destroy() {
      if (live) {
        video.removeEventListener("timeupdate", handleLiveUpdate);
        video.removeEventListener("seeking", handleLiveSeek);
        video.removeEventListener("pause", handlePause);
      }
      if (video.playbackRate !== 1) {
        video.playbackRate = 1;
      }
      hls.destroy();
    },
  };
}
