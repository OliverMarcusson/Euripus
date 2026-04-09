import type { PlaybackSource } from "@euripus/shared";
import Plyr from "plyr";
import {
  AUTO_HLS_QUALITY,
  createIptvHls,
  isIptvHlsSupported,
  type HlsQualityOption,
  type HlsSession,
} from "@/lib/hls";

type PlayerUiMode = "local" | "receiver";

type BoundPlaybackSession = {
  plyr: Plyr | null;
  destroy: () => void;
};

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
  "current-time",
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
  { uiMode = "local" }: { uiMode?: PlayerUiMode } = {},
): BoundPlaybackSession {
  resetMediaElement(video);

  let destroyed = false;
  let hlsSession: HlsSession | undefined;
  let qualitySignature = "";
  let unsubscribeFromQualities: (() => void) | undefined;
  const session: BoundPlaybackSession = {
    plyr: null,
    destroy() {
      destroyed = true;
      unsubscribeFromQualities?.();
      hlsSession?.destroy();
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
    hlsSession = createIptvHls(video, source.url, { live: source.live });
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
