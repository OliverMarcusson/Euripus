import type { PlaybackSource } from "@euripus/shared";
import { create } from "zustand";

const CAST_SENDER_SDK_URL =
  "https://www.gstatic.com/cv/js/sender/v1/cast_sender.js?loadCastFramework=1";

type CastSession = {
  getCastDevice: () => { friendlyName?: string };
  loadMedia: (request: unknown) => Promise<unknown>;
};

type CastContext = {
  addEventListener: (
    eventType: string,
    listener: (event: { castState?: string; sessionState?: string }) => void,
  ) => void;
  endCurrentSession: (stopCasting: boolean) => void;
  getCastState: () => string;
  getCurrentSession: () => CastSession | null;
  requestSession: () => Promise<unknown>;
  setOptions: (options: Record<string, unknown>) => void;
};

type CastWindow = Window & {
  __onGCastApiAvailable?: (available: boolean) => void;
  cast?: {
    framework: {
      CastContext: { getInstance: () => CastContext };
      CastContextEventType: {
        CAST_STATE_CHANGED: string;
        SESSION_STATE_CHANGED: string;
      };
      CastState: { NO_DEVICES_AVAILABLE: string };
      SessionState: {
        SESSION_ENDED: string;
        SESSION_ENDING: string;
        SESSION_RESUMED: string;
        SESSION_STARTED: string;
        SESSION_STARTING: string;
      };
    };
  };
  chrome?: {
    cast: {
      AutoJoinPolicy: { ORIGIN_SCOPED: string };
      media: {
        DEFAULT_MEDIA_RECEIVER_APP_ID: string;
        GenericMediaMetadata: new () => { title?: string };
        MediaInfo: new (contentId: string, contentType: string) => {
          metadata?: unknown;
          streamType?: string;
        };
        LoadRequest: new (mediaInfo: unknown) => {
          autoplay?: boolean;
          currentTime?: number;
        };
        StreamType: { BUFFERED: string; LIVE: string };
      };
    };
  };
};

type GoogleCastState = {
  available: boolean;
  connected: boolean;
  deviceName: string | null;
  initialized: boolean;
  initializing: boolean;
};

const initialState: GoogleCastState = {
  available: false,
  connected: false,
  deviceName: null,
  initialized: false,
  initializing: false,
};

export const useGoogleCastStore = create<GoogleCastState>(() => initialState);

let castContext: CastContext | null = null;
let initialization: Promise<boolean> | null = null;

function castWindow() {
  return window as CastWindow;
}

function syncSessionState() {
  const session = castContext?.getCurrentSession() ?? null;
  useGoogleCastStore.setState({
    connected: !!session,
    deviceName: session?.getCastDevice().friendlyName ?? null,
  });
}

function configureCastFramework() {
  const targetWindow = castWindow();
  const framework = targetWindow.cast?.framework;
  const chromeCast = targetWindow.chrome?.cast;
  if (!framework || !chromeCast) {
    return false;
  }

  castContext = framework.CastContext.getInstance();
  castContext.setOptions({
    receiverApplicationId: chromeCast.media.DEFAULT_MEDIA_RECEIVER_APP_ID,
    autoJoinPolicy: chromeCast.AutoJoinPolicy.ORIGIN_SCOPED,
  });
  useGoogleCastStore.setState({
    available:
      castContext.getCastState() !== framework.CastState.NO_DEVICES_AVAILABLE,
  });
  castContext.addEventListener(
    framework.CastContextEventType.CAST_STATE_CHANGED,
    (event) => {
      useGoogleCastStore.setState({
        available: event.castState !== framework.CastState.NO_DEVICES_AVAILABLE,
      });
    },
  );
  castContext.addEventListener(
    framework.CastContextEventType.SESSION_STATE_CHANGED,
    syncSessionState,
  );
  syncSessionState();
  return true;
}

export function initializeGoogleCast() {
  if (initialization) {
    return initialization;
  }

  useGoogleCastStore.setState({ initializing: true });
  initialization = new Promise<boolean>((resolve) => {
    const targetWindow = castWindow();
    const finish = (available: boolean) => {
      const initialized = available && configureCastFramework();
      useGoogleCastStore.setState({
        initialized: true,
        initializing: false,
        ...(initialized ? {} : { available: false }),
      });
      resolve(initialized);
    };

    if (targetWindow.cast?.framework && targetWindow.chrome?.cast) {
      finish(true);
      return;
    }

    const previousCallback = targetWindow.__onGCastApiAvailable;
    targetWindow.__onGCastApiAvailable = (available) => {
      previousCallback?.(available);
      finish(available);
    };

    if (!document.querySelector(`script[src="${CAST_SENDER_SDK_URL}"]`)) {
      const script = document.createElement("script");
      script.src = CAST_SENDER_SDK_URL;
      script.async = true;
      script.onerror = () => finish(false);
      document.head.append(script);
    }
  });

  return initialization;
}

export async function requestGoogleCastSession() {
  await initializeGoogleCast();
  if (!castContext) {
    throw new Error("Google Cast is not available in this browser.");
  }
  await castContext.requestSession();
  syncSessionState();
}

export function endGoogleCastSession() {
  castContext?.endCurrentSession(true);
  useGoogleCastStore.setState({ connected: false, deviceName: null });
}

function contentTypeFor(source: PlaybackSource) {
  switch (source.kind) {
    case "hls":
      return "application/vnd.apple.mpegurl";
    case "mpegts":
      return "video/mp2t";
    case "progressive":
      return "video/mp4";
    default:
      throw new Error(source.unsupportedReason ?? "This media cannot be cast.");
  }
}

export async function loadGoogleCastMedia(source: PlaybackSource) {
  const targetWindow = castWindow();
  const chromeCast = targetWindow.chrome?.cast;
  const session = castContext?.getCurrentSession();
  if (!chromeCast || !session) {
    throw new Error("Connect to a Google Cast device first.");
  }
  if (source.kind === "unsupported" || !source.url) {
    throw new Error(source.unsupportedReason ?? "This media cannot be cast.");
  }

  const mediaInfo = new chromeCast.media.MediaInfo(
    source.url,
    contentTypeFor(source),
  );
  const metadata = new chromeCast.media.GenericMediaMetadata();
  metadata.title = source.title;
  mediaInfo.metadata = metadata;
  mediaInfo.streamType = source.live
    ? chromeCast.media.StreamType.LIVE
    : chromeCast.media.StreamType.BUFFERED;

  const request = new chromeCast.media.LoadRequest(mediaInfo);
  request.autoplay = true;
  request.currentTime = 0;
  await session.loadMedia(request);
}
