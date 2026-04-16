import { useEffect, useMemo, useRef, useState } from "react";
import type {
  PlaybackSource,
  ReceiverPlaybackStatePayload,
  ReceiverSession,
} from "@euripus/shared";
import { Tv } from "lucide-react";
import {
  API_BASE_URL,
  acknowledgeReceiverCommand,
  createReceiverSession,
  heartbeatReceiver,
  updateReceiverPlaybackState,
} from "@/lib/api";
import type { RemoteDeviceEventPayload } from "@/lib/remote-events";
import {
  Empty,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { PlyrSurface } from "@/components/player/plyr-surface";

const RECEIVER_STORAGE_KEY = "euripus-receiver-device";
const RECEIVER_HEARTBEAT_MS = 15_000;
const RECEIVER_PLAYBACK_SYNC_INTERVAL_MS = 3_000;
const SEEK_COMPLETION_TOLERANCE_SECONDS = 1.5;

type PendingCommand =
  | { id: string; kind: "playback_source" | "play" | "pause" | "stop" }
  | { id: string; kind: "seek"; positionSeconds: number | null };

type ReceiverPersistedState = {
  deviceKey: string;
  receiverCredential: string | null;
};

function loadPersistedState(): ReceiverPersistedState {
  if (typeof window === "undefined") {
    return { deviceKey: crypto.randomUUID(), receiverCredential: null };
  }

  const raw = window.localStorage.getItem(RECEIVER_STORAGE_KEY);
  if (!raw) {
    return { deviceKey: crypto.randomUUID(), receiverCredential: null };
  }

  try {
    return JSON.parse(raw) as ReceiverPersistedState;
  } catch {
    return { deviceKey: crypto.randomUUID(), receiverCredential: null };
  }
}

function persistState(next: ReceiverPersistedState) {
  if (typeof window === "undefined") {
    return;
  }
  window.localStorage.setItem(RECEIVER_STORAGE_KEY, JSON.stringify(next));
}

function buildEventsUrl(sessionToken: string) {
  const baseUrl = typeof window === "undefined" ? API_BASE_URL : new URL(API_BASE_URL, window.location.origin).toString();
  const url = new URL(`${baseUrl}/receiver/events`);
  url.searchParams.set("sessionToken", sessionToken);
  return url.toString();
}

function detectFormFactorHint() {
  if (typeof window === "undefined") {
    return "large-screen";
  }
  return window.innerWidth >= 960 ? "large-screen" : "desktop";
}

function formatPairingCode(code: string) {
  return code.split("").join(" ");
}

function normalizePlaybackSyncState(
  payload: ReceiverPlaybackStatePayload,
) {
  return {
    ...payload,
    positionSeconds:
      payload.positionSeconds == null ? null : Math.round(payload.positionSeconds),
    durationSeconds:
      payload.durationSeconds == null ? null : Math.round(payload.durationSeconds),
  };
}

function describeVideoError(video: HTMLVideoElement | null) {
  const mediaError = video?.error;
  if (!mediaError) {
    return null;
  }
  switch (mediaError.code) {
    case MediaError.MEDIA_ERR_ABORTED:
      return "Playback was interrupted before the stream finished loading.";
    case MediaError.MEDIA_ERR_NETWORK:
      return "The receiver lost connection while streaming.";
    case MediaError.MEDIA_ERR_DECODE:
      return "The receiver could not decode this stream.";
    case MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED:
      return "This stream is not supported on the receiver.";
    default:
      return "Playback failed on the receiver.";
  }
}

export function ReceiverPage() {
  const initial = useMemo(loadPersistedState, []);
  const [session, setSession] = useState<ReceiverSession | null>(null);
  const [pairingCode, setPairingCode] = useState<string | null>(null);
  const [source, setSource] = useState<PlaybackSource | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [buffering, setBuffering] = useState(false);
  const [playbackError, setPlaybackError] = useState<string | null>(null);
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const pendingCommandRef = useRef<PendingCommand | null>(null);
  const sourceRef = useRef<PlaybackSource | null>(null);
  const bufferingRef = useRef(false);
  const playbackErrorRef = useRef<string | null>(null);
  const lastPlaybackSyncRef = useRef<{
    normalizedPayload: string;
    sentAt: number;
  } | null>(null);
  const pendingPlaybackSyncTimerRef = useRef<number | null>(null);

  const updateSourceState = (nextSource: PlaybackSource | null) => {
    sourceRef.current = nextSource;
    setSource(nextSource);
  };

  const updateBufferingState = (nextBuffering: boolean) => {
    bufferingRef.current = nextBuffering;
    setBuffering(nextBuffering);
  };

  const updatePlaybackErrorState = (nextPlaybackError: string | null) => {
    playbackErrorRef.current = nextPlaybackError;
    setPlaybackError(nextPlaybackError);
  };

  useEffect(() => {
    let active = true;

    async function bootstrap() {
      try {
        const nextSession = await createReceiverSession({
          deviceKey: initial.deviceKey,
          name: "Browser receiver",
          platform: "web",
          formFactorHint: detectFormFactorHint(),
          appKind: "receiver-web",
          publicOrigin:
            typeof window === "undefined" ? null : window.location.origin,
          receiverCredential: initial.receiverCredential,
        });
        if (!active) {
          return;
        }
        setSession(nextSession);
        setPairingCode(nextSession.pairingCode);
        if (nextSession.receiverCredential) {
          persistState({
            deviceKey: initial.deviceKey,
            receiverCredential: nextSession.receiverCredential,
          });
        }
      } catch (nextError) {
        if (active) {
          setError(nextError instanceof Error ? nextError.message : "Receiver startup failed.");
        }
      }
    }

    void bootstrap();
    return () => {
      active = false;
    };
  }, [initial.deviceKey, initial.receiverCredential]);

  useEffect(() => {
    sourceRef.current = source;
  }, [source]);

  useEffect(() => {
    bufferingRef.current = buffering;
  }, [buffering]);

  useEffect(() => {
    playbackErrorRef.current = playbackError;
  }, [playbackError]);

  useEffect(() => () => {
    if (pendingPlaybackSyncTimerRef.current != null) {
      window.clearTimeout(pendingPlaybackSyncTimerRef.current);
    }
  }, []);

  useEffect(() => {
    if (!session?.sessionToken) {
      return;
    }

    const heartbeat = () => void heartbeatReceiver(session.sessionToken).catch(() => undefined);
    heartbeat();
    const timer = window.setInterval(heartbeat, RECEIVER_HEARTBEAT_MS);
    return () => window.clearInterval(timer);
  }, [session?.sessionToken]);

  useEffect(() => {
    if (!session?.sessionToken) {
      return;
    }

    const events = new EventSource(buildEventsUrl(session.sessionToken), { withCredentials: true });
    events.addEventListener("playback_command", (event) => {
      const payload = JSON.parse((event as MessageEvent<string>).data) as RemoteDeviceEventPayload;
      if (!payload.source) {
        return;
      }
      if (payload.source.kind === "unsupported") {
        updatePlaybackErrorState(
          payload.source.unsupportedReason ??
            "This stream is not supported on the receiver.",
        );
        updateBufferingState(false);
        updateSourceState(payload.source);
        pendingCommandRef.current = null;
        void acknowledgeReceiverCommand(session.sessionToken, payload.command.id, {
          status: "failed",
          errorMessage:
            payload.source.unsupportedReason ??
            "This stream is not supported on the receiver.",
        }).catch(() => undefined);
        return;
      }
      updatePlaybackErrorState(null);
      updateBufferingState(true);
      pendingCommandRef.current = {
        id: payload.command.id,
        kind: "playback_source",
      };
      void acknowledgeReceiverCommand(session.sessionToken, payload.command.id, {
        status: "executing",
      }).catch(() => undefined);
      updateSourceState(payload.source);
    });
    events.addEventListener("transport_command", (event) => {
      const payload = JSON.parse((event as MessageEvent<string>).data) as RemoteDeviceEventPayload;
      const video = videoRef.current;
      const commandType = payload.command.commandType;
      pendingCommandRef.current =
        commandType === "seek"
          ? {
              id: payload.command.id,
              kind: "seek",
              positionSeconds: payload.positionSeconds ?? null,
            }
          : {
              id: payload.command.id,
              kind:
                commandType === "pause" ||
                commandType === "play" ||
                commandType === "stop"
                  ? commandType
                  : "stop",
            };
      void acknowledgeReceiverCommand(session.sessionToken, payload.command.id, {
        status: "executing",
      }).catch(() => undefined);
      if (video) {
        if (commandType === "pause") {
          void video.pause();
        } else if (commandType === "play") {
          updatePlaybackErrorState(null);
          void video.play().catch(() => undefined);
        } else if (commandType === "seek" && typeof payload.positionSeconds === "number") {
          video.currentTime = payload.positionSeconds;
        } else if (commandType === "stop") {
          video.pause();
          updateSourceState(null);
          updateBufferingState(false);
        }
      }
    });
    events.addEventListener("pairing_complete", (event) => {
      const payload = JSON.parse((event as MessageEvent<string>).data) as RemoteDeviceEventPayload;
      if (payload.receiverCredential) {
        persistState({
          deviceKey: initial.deviceKey,
          receiverCredential: payload.receiverCredential,
        });
      }
      setPairingCode(null);
    });
    return () => {
      events.close();
    };
  }, [initial.deviceKey, session?.sessionToken]);

  useEffect(() => {
    if (!session?.sessionToken) {
      return;
    }

    const clearScheduledSync = () => {
      if (pendingPlaybackSyncTimerRef.current != null) {
        window.clearTimeout(pendingPlaybackSyncTimerRef.current);
        pendingPlaybackSyncTimerRef.current = null;
      }
    };

    const buildPlaybackPayload = (): ReceiverPlaybackStatePayload => {
      const currentSource = sourceRef.current;
      const currentPlaybackError = playbackErrorRef.current;
      const video = videoRef.current;
      const isBuffering =
        !!currentSource &&
        currentSource.kind !== "unsupported" &&
        !currentPlaybackError &&
        !!video &&
        !video.paused &&
        !video.ended &&
        video.readyState < HTMLMediaElement.HAVE_FUTURE_DATA;

      return {
        title: currentSource?.title ?? null,
        sourceKind: currentSource?.kind ?? null,
        live: currentSource?.live ?? null,
        catchup: currentSource?.catchup ?? null,
        paused: video ? video.paused : true,
        buffering: isBuffering || bufferingRef.current,
        positionSeconds: video ? video.currentTime : null,
        durationSeconds:
          video && Number.isFinite(video.duration) ? video.duration : null,
        errorMessage: currentPlaybackError,
      };
    };

    const syncPlaybackState = ({
      immediate = false,
      force = false,
    }: {
      immediate?: boolean;
      force?: boolean;
    } = {}) => {
      const payload = buildPlaybackPayload();
      const normalizedPayload = JSON.stringify(
        normalizePlaybackSyncState(payload),
      );
      const lastSync = lastPlaybackSyncRef.current;
      const now = Date.now();
      const msSinceLastSync = lastSync ? now - lastSync.sentAt : Infinity;

      if (!force && lastSync?.normalizedPayload === normalizedPayload) {
        return;
      }

      if (!immediate && msSinceLastSync < RECEIVER_PLAYBACK_SYNC_INTERVAL_MS) {
        if (pendingPlaybackSyncTimerRef.current == null) {
          pendingPlaybackSyncTimerRef.current = window.setTimeout(() => {
            pendingPlaybackSyncTimerRef.current = null;
            syncPlaybackState({ force: true });
          }, RECEIVER_PLAYBACK_SYNC_INTERVAL_MS - msSinceLastSync);
        }
        return;
      }

      clearScheduledSync();
      lastPlaybackSyncRef.current = {
        normalizedPayload,
        sentAt: now,
      };
      void updateReceiverPlaybackState(session.sessionToken, payload).catch(
        () => undefined,
      );
    };

    const maybeCompletePendingCommand = () => {
      const pending = pendingCommandRef.current;
      if (!pending) {
        return;
      }

      const currentPlaybackError = playbackErrorRef.current;
      const currentSource = sourceRef.current;
      const video = videoRef.current;

      if (currentPlaybackError) {
        pendingCommandRef.current = null;
        void acknowledgeReceiverCommand(session.sessionToken, pending.id, {
          status: "failed",
          errorMessage: currentPlaybackError,
        }).catch(() => undefined);
        return;
      }

      if (pending.kind === "stop" && !currentSource) {
        pendingCommandRef.current = null;
        void acknowledgeReceiverCommand(session.sessionToken, pending.id, {
          status: "succeeded",
        }).catch(() => undefined);
        return;
      }

      if (!video) {
        return;
      }

      if (
        pending.kind === "playback_source" &&
        video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA
      ) {
        pendingCommandRef.current = null;
        void acknowledgeReceiverCommand(session.sessionToken, pending.id, {
          status: "succeeded",
        }).catch(() => undefined);
        return;
      }

      if (
        pending.kind === "play" &&
        !video.paused &&
        video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA
      ) {
        pendingCommandRef.current = null;
        void acknowledgeReceiverCommand(session.sessionToken, pending.id, {
          status: "succeeded",
        }).catch(() => undefined);
        return;
      }

      if (pending.kind === "pause" && video.paused) {
        pendingCommandRef.current = null;
        void acknowledgeReceiverCommand(session.sessionToken, pending.id, {
          status: "succeeded",
        }).catch(() => undefined);
        return;
      }

      if (
        pending.kind === "seek" &&
        pending.positionSeconds != null &&
        Math.abs(video.currentTime - pending.positionSeconds) <=
          SEEK_COMPLETION_TOLERANCE_SECONDS &&
        !video.seeking
      ) {
        pendingCommandRef.current = null;
        void acknowledgeReceiverCommand(session.sessionToken, pending.id, {
          status: "succeeded",
        }).catch(() => undefined);
      }
    };

    syncPlaybackState({ immediate: true, force: true });
    maybeCompletePendingCommand();

    const video = videoRef.current;
    if (!video) {
      return () => {
        clearScheduledSync();
      };
    }

    const handleWaiting = () => {
      updateBufferingState(true);
      syncPlaybackState();
      maybeCompletePendingCommand();
    };
    const handlePlaying = () => {
      updateBufferingState(false);
      updatePlaybackErrorState(null);
      syncPlaybackState({ immediate: true });
      maybeCompletePendingCommand();
    };
    const handleCanPlay = () => {
      updateBufferingState(false);
      syncPlaybackState();
      maybeCompletePendingCommand();
    };
    const handlePause = () => {
      updateBufferingState(false);
      syncPlaybackState({ immediate: true });
      maybeCompletePendingCommand();
    };
    const handlePlay = () => {
      updatePlaybackErrorState(null);
      syncPlaybackState({ immediate: true });
      maybeCompletePendingCommand();
    };
    const handleTimeUpdate = () => {
      syncPlaybackState();
    };
    const handleSeeked = () => {
      updateBufferingState(false);
      syncPlaybackState({ immediate: true });
      maybeCompletePendingCommand();
    };
    const handleEnded = () => {
      updateBufferingState(false);
      syncPlaybackState({ immediate: true, force: true });
      maybeCompletePendingCommand();
    };
    const handleError = () => {
      const nextError = describeVideoError(video);
      updateBufferingState(false);
      updatePlaybackErrorState(nextError);
      syncPlaybackState({ immediate: true, force: true });
      maybeCompletePendingCommand();
    };

    video.addEventListener("pause", handlePause);
    video.addEventListener("play", handlePlay);
    video.addEventListener("playing", handlePlaying);
    video.addEventListener("canplay", handleCanPlay);
    video.addEventListener("loadeddata", handleCanPlay);
    video.addEventListener("timeupdate", handleTimeUpdate);
    video.addEventListener("waiting", handleWaiting);
    video.addEventListener("seeking", handleWaiting);
    video.addEventListener("seeked", handleSeeked);
    video.addEventListener("ended", handleEnded);
    video.addEventListener("error", handleError);
    return () => {
      clearScheduledSync();
      video.removeEventListener("pause", handlePause);
      video.removeEventListener("play", handlePlay);
      video.removeEventListener("playing", handlePlaying);
      video.removeEventListener("canplay", handleCanPlay);
      video.removeEventListener("loadeddata", handleCanPlay);
      video.removeEventListener("timeupdate", handleTimeUpdate);
      video.removeEventListener("waiting", handleWaiting);
      video.removeEventListener("seeking", handleWaiting);
      video.removeEventListener("seeked", handleSeeked);
      video.removeEventListener("ended", handleEnded);
      video.removeEventListener("error", handleError);
    };
  }, [session?.sessionToken, source]);

  if (pairingCode) {
    return (
      <div className="min-h-screen bg-background text-foreground">
        <div className="absolute inset-0 bg-[radial-gradient(circle_at_top,rgba(168,85,247,0.22),transparent_34%),radial-gradient(circle_at_80%_20%,rgba(192,132,252,0.16),transparent_28%),linear-gradient(180deg,rgba(10,10,15,0.96),rgba(5,5,10,1))]" />
        <main className="relative grid min-h-screen place-items-center px-6 py-10">
          <section className="flex w-full max-w-[52rem] flex-col items-center gap-8 text-center">
            <div className="flex flex-col items-center gap-3">
              <p className="text-sm font-medium uppercase tracking-[0.2em] text-white/80">
                Euripus Receiver
              </p>
              <div className="flex flex-col items-center gap-2">
                <h1 className="text-4xl font-semibold tracking-tight text-balance text-white">
                  Pair this screen
                </h1>
                <p className="max-w-2xl text-lg text-white/72 text-balance">
                  Open Euripus on your phone, enter the code below, and choose whether to remember
                  this screen.
                </p>
              </div>
            </div>

            <div className="inline-flex max-w-full items-center justify-center overflow-hidden rounded-lg border border-white/10 bg-white/[0.04] px-10 py-7 shadow-[0_0_0_1px_rgba(255,255,255,0.02),0_24px_80px_rgba(76,29,149,0.18)] backdrop-blur-sm">
              <span className="block whitespace-nowrap text-center text-7xl font-semibold text-white sm:text-8xl">
                {formatPairingCode(pairingCode)}
              </span>
            </div>

            {error ? <p className="text-sm text-destructive">{error}</p> : null}
          </section>
        </main>
      </div>
    );
  }

  if (!source) {
    return (
      <div className="min-h-screen bg-background text-foreground">
        <div className="absolute inset-0 bg-[radial-gradient(circle_at_top,rgba(168,85,247,0.18),transparent_34%),radial-gradient(circle_at_80%_20%,rgba(192,132,252,0.12),transparent_28%),linear-gradient(180deg,rgba(10,10,15,0.96),rgba(5,5,10,1))]" />
        <main className="relative grid min-h-screen place-items-center px-6 py-10">
          <Empty className="border-0">
            <EmptyHeader>
              <EmptyMedia variant="icon" className="border border-white/10 bg-white/[0.04] text-primary shadow-[0_18px_60px_rgba(76,29,149,0.22)]">
                <Tv aria-hidden="true" />
              </EmptyMedia>
              <EmptyTitle className="text-white">Nothing is playing</EmptyTitle>
            </EmptyHeader>
          </Empty>
        </main>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-black text-white">
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_top,rgba(168,85,247,0.14),transparent_30%),linear-gradient(180deg,rgba(10,10,15,0.24),rgba(10,10,15,0.4))]" />
      {source.kind === "unsupported" || playbackError ? (
        <main className="relative grid min-h-screen place-items-center px-6 py-10">
          <div className="max-w-2xl rounded-lg border border-amber-400/30 bg-amber-400/10 p-6 text-amber-100">
            {playbackError ??
              source.unsupportedReason ??
              "This stream is not supported on the receiver."}
          </div>
        </main>
      ) : (
        <div className="euripus-plyr-shell euripus-plyr-shell--receiver relative h-screen w-screen">
          <PlyrSurface
            ariaLabel={`Playing ${source.title}`}
            className="contents"
            source={source}
            uiMode="receiver"
            videoClassName="euripus-plyr-media relative h-screen w-screen bg-black object-contain"
            videoRef={videoRef}
          />
        </div>
      )}
    </div>
  );
}
