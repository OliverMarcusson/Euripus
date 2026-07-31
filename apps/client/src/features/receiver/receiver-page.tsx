import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  PlaybackSource,
  ReceiverPlaybackStatePayload,
} from "@euripus/shared";
import { Tv } from "lucide-react";
import {
  API_BASE_URL,
  acknowledgeReceiverCommand,
  startReceiverCastTranscode,
  stopReceiverCastTranscode,
  updateReceiverPlaybackState,
} from "@/lib/api";
import type { RemoteDeviceEventPayload } from "@/lib/remote-events";
import { PlyrSurface } from "@/components/player/plyr-surface";
import type { PlaybackFailure } from "@/lib/hls";
import {
  initializeGoogleCastReceiver,
  isGoogleCastReceiver,
  publishGoogleCastReceiverStatus,
} from "@/lib/google-cast-receiver";
import { formatEventChannelTitle } from "@/lib/utils";
import { createUuid } from "@/lib/uuid";
import { useReceiverSession } from "@/features/receiver/use-receiver-session";
import {
  ReceiverSpinner,
  ReceiverStatusScreen,
} from "@/features/receiver/receiver-status-screen";

const RECEIVER_STORAGE_KEY = "euripus-receiver-device";
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
    return { deviceKey: createUuid(), receiverCredential: null };
  }

  const raw = window.localStorage.getItem(RECEIVER_STORAGE_KEY);
  if (!raw) {
    return { deviceKey: createUuid(), receiverCredential: null };
  }

  try {
    return JSON.parse(raw) as ReceiverPersistedState;
  } catch {
    return { deviceKey: createUuid(), receiverCredential: null };
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

export function describeReceiverFailure(failure: PlaybackFailure) {
  if (failure.kind === "provider-unavailable") {
    return failure.message;
  }

  switch (failure.reason) {
    case "codec":
      return "This stream's video format is not supported by this Cast device.";
    case "network":
      return "The receiver lost connection to this stream.";
    case "hls":
      return "This stream could not be played on the receiver.";
    case "stall":
    case "unexpected-end":
      return "This stream stopped unexpectedly.";
    default:
      return "Playback failed on the receiver.";
  }
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
  const castReceiver = useMemo(isGoogleCastReceiver, []);
  const persistReceiverCredential = useCallback(
    (receiverCredential: string) => {
      persistState({
        deviceKey: initial.deviceKey,
        receiverCredential,
      });
    },
    [initial.deviceKey],
  );
  const [source, setSource] = useState<PlaybackSource | null>(null);
  const [castFrameworkError, setCastFrameworkError] = useState<string | null>(
    null,
  );
  const [buffering, setBuffering] = useState(false);
  const [playbackError, setPlaybackError] = useState<string | null>(null);
  const [startingPlayback, setStartingPlayback] = useState(false);
  const [preparingTranscode, setPreparingTranscode] = useState(false);
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const displaySourceTitle = source
    ? formatEventChannelTitle(source.title)
    : null;
  const pendingCommandRef = useRef<PendingCommand | null>(null);
  const sourceRef = useRef<PlaybackSource | null>(null);
  const bufferingRef = useRef(false);
  const playbackErrorRef = useRef<string | null>(null);
  const lastPlaybackSyncRef = useRef<{
    normalizedPayload: string;
    sentAt: number;
  } | null>(null);
  const pendingPlaybackSyncTimerRef = useRef<number | null>(null);
  const activeCastTranscodeRef = useRef(false);
  const castTranscodeRequestInFlightRef = useRef(false);
  const preparingTranscodeRef = useRef(false);
  const preparedTranscodeUrlRef = useRef<string | null>(null);
  const {
    completePairing,
    error: receiverSessionError,
    pairingCode,
    session,
  } = useReceiverSession({
    castReceiver,
    deviceKey: initial.deviceKey,
    initialReceiverCredential: initial.receiverCredential,
    persistReceiverCredential,
  });
  const error = castFrameworkError ?? receiverSessionError;

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

  const beginTranscodePreparation = () => {
    preparingTranscodeRef.current = true;
    preparedTranscodeUrlRef.current = null;
    setPreparingTranscode(true);
  };

  const finishTranscodePreparation = () => {
    preparingTranscodeRef.current = false;
    preparedTranscodeUrlRef.current = null;
    setPreparingTranscode(false);
  };

  const finishTranscodePreparationWhenReady = () => {
    if (
      preparingTranscodeRef.current &&
      preparedTranscodeUrlRef.current === sourceRef.current?.url
    ) {
      finishTranscodePreparation();
    }
  };

  const failReceiverPlayback = (message: string) => {
    const currentSource = sourceRef.current;
    const pending = pendingCommandRef.current;

    updateBufferingState(false);
    updatePlaybackErrorState(message);
    setStartingPlayback(false);
    pendingCommandRef.current = null;

    if (!session?.sessionToken) {
      return;
    }
    if (pending) {
      void acknowledgeReceiverCommand(session.sessionToken, pending.id, {
        status: "failed",
        errorMessage: message,
      }).catch(() => undefined);
    }
    void updateReceiverPlaybackState(session.sessionToken, {
      title: currentSource?.title ?? null,
      sourceKind: currentSource?.kind ?? null,
      live: currentSource?.live ?? null,
      catchup: currentSource?.catchup ?? null,
      paused: true,
      buffering: false,
      positionSeconds: null,
      durationSeconds: null,
      errorMessage: message,
    }).catch(() => undefined);
  };

  const handleReceiverPlaybackFailure = async (failure: PlaybackFailure) => {
    const currentSource = sourceRef.current;
    const canRetryWithTranscoding =
      castReceiver &&
      failure.kind === "recoverable" &&
      failure.reason === "codec" &&
      (currentSource?.kind === "hls" || currentSource?.kind === "progressive") &&
      !currentSource.url.includes("/api/transcode/") &&
      !!session?.sessionToken;

    if (canRetryWithTranscoding && !castTranscodeRequestInFlightRef.current) {
      castTranscodeRequestInFlightRef.current = true;
      const originalSourceUrl = currentSource.url;
      beginTranscodePreparation();
      updateBufferingState(true);
      updatePlaybackErrorState(null);
      try {
        const transcodedSource = await startReceiverCastTranscode(
          session.sessionToken,
          currentSource,
        );
        if (sourceRef.current?.url !== originalSourceUrl) {
          await stopReceiverCastTranscode(session.sessionToken).catch(() => undefined);
          return;
        }
        activeCastTranscodeRef.current = true;
        preparedTranscodeUrlRef.current = transcodedSource.url;
        updateBufferingState(true);
        updateSourceState(transcodedSource);
        return;
      } catch (transcodeError) {
        const message =
          transcodeError instanceof Error
            ? transcodeError.message
            : "The server could not prepare a compatible stream.";
        finishTranscodePreparation();
        failReceiverPlayback(message);
        return;
      } finally {
        castTranscodeRequestInFlightRef.current = false;
      }
    }

    finishTranscodePreparation();
    failReceiverPlayback(describeReceiverFailure(failure));
  };

  useEffect(() => {
    if (!castReceiver) {
      return;
    }
    let active = true;
    void initializeGoogleCastReceiver().then((started) => {
      if (active && !started) {
        setCastFrameworkError(
          "Could not start the Google Cast receiver. Reload this screen or cast again.",
        );
      }
    });
    return () => {
      active = false;
    };
  }, [castReceiver]);

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

    const events = new EventSource(buildEventsUrl(session.sessionToken), { withCredentials: true });
    events.addEventListener("open", () => {
      if (castReceiver) {
        publishGoogleCastReceiverStatus({
          type: "receiver_status",
          deviceId: session.device.id,
          paired: pairingCode === null,
          pairingCode,
        });
      }
    });
    events.addEventListener("playback_command", (event) => {
      const payload = JSON.parse((event as MessageEvent<string>).data) as RemoteDeviceEventPayload;
      if (!payload.source) {
        return;
      }
      finishTranscodePreparation();
      if (activeCastTranscodeRef.current) {
        activeCastTranscodeRef.current = false;
        void stopReceiverCastTranscode(session.sessionToken).catch(() => undefined);
      }
      if (payload.source.kind === "unsupported") {
        setStartingPlayback(false);
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
      setStartingPlayback(true);
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
      // A transcode is served as a rolling window with no seekable range
      // behind it, so accepting a seek would strand playback.
      if (commandType === "seek" && activeCastTranscodeRef.current) {
        void acknowledgeReceiverCommand(session.sessionToken, payload.command.id, {
          status: "failed",
          errorMessage:
            "Seeking is not available while this stream is being converted.",
        }).catch(() => undefined);
        return;
      }
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
      if (commandType === "stop") {
        video?.pause();
        finishTranscodePreparation();
        setStartingPlayback(false);
        updatePlaybackErrorState(null);
        updateSourceState(null);
        updateBufferingState(false);
        if (activeCastTranscodeRef.current) {
          activeCastTranscodeRef.current = false;
          void stopReceiverCastTranscode(session.sessionToken).catch(() => undefined);
        }
      } else if (video) {
        if (commandType === "pause") {
          void video.pause();
        } else if (commandType === "play") {
          updatePlaybackErrorState(null);
          void video.play().catch(() => undefined);
        } else if (commandType === "seek" && typeof payload.positionSeconds === "number") {
          video.currentTime = payload.positionSeconds;
        }
      }
    });
    events.addEventListener("pairing_complete", (event) => {
      const payload = JSON.parse((event as MessageEvent<string>).data) as RemoteDeviceEventPayload;
      completePairing(payload.receiverCredential);
      if (castReceiver) {
        publishGoogleCastReceiverStatus({
          type: "receiver_status",
          deviceId: session.device.id,
          paired: true,
          pairingCode: null,
        });
      }
    });
    return () => {
      events.close();
    };
  }, [
    castReceiver,
    completePairing,
    pairingCode,
    session?.device.id,
    session?.sessionToken,
  ]);

  useEffect(() => {
    const sessionToken = session?.sessionToken;
    return () => {
      if (sessionToken && activeCastTranscodeRef.current) {
        activeCastTranscodeRef.current = false;
        void stopReceiverCastTranscode(sessionToken).catch(() => undefined);
      }
    };
  }, [session?.sessionToken]);

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
      finishTranscodePreparationWhenReady();
      setStartingPlayback(false);
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
      if (
        castTranscodeRequestInFlightRef.current ||
        preparingTranscodeRef.current
      ) {
        updateBufferingState(true);
        syncPlaybackState({ immediate: true, force: true });
        return;
      }
      const nextError = describeVideoError(video);
      setStartingPlayback(false);
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
      <ReceiverStatusScreen
        description={
          castReceiver
            ? undefined
            : "Open Euripus on your phone, enter the code below, and choose whether to remember this screen."
        }
        notice={error}
        role="status"
        title={castReceiver ? "Connecting to Euripus" : "Pair this screen"}
      >
        {castReceiver ? (
          <ReceiverSpinner />
        ) : (
          <div className="euripus-receiver__code-frame">
            <span className="euripus-receiver__code">
              {formatPairingCode(pairingCode)}
            </span>
          </div>
        )}
      </ReceiverStatusScreen>
    );
  }

  if (error) {
    return (
      <ReceiverStatusScreen
        description={error}
        role="alert"
        title="Receiver unavailable"
        tone="error"
      >
        <div className="euripus-receiver__icon">
          <Tv aria-hidden="true" />
        </div>
      </ReceiverStatusScreen>
    );
  }

  if (!session) {
    return (
      <ReceiverStatusScreen
        role="status"
        title="Starting receiver"
      >
        <ReceiverSpinner />
      </ReceiverStatusScreen>
    );
  }

  if (!source) {
    return (
      <ReceiverStatusScreen title="Nothing is playing">
        <div className="euripus-receiver__icon">
          <Tv aria-hidden="true" />
        </div>
      </ReceiverStatusScreen>
    );
  }

  if (source.kind === "unsupported" || playbackError) {
    return (
      <ReceiverStatusScreen
        description={
          playbackError ??
          source.unsupportedReason ??
          "This stream is not supported on the receiver."
        }
        role="alert"
        title="Playback unavailable"
        tone="error"
      >
        <div className="euripus-receiver__icon">
          <Tv aria-hidden="true" />
        </div>
      </ReceiverStatusScreen>
    );
  }

  return (
    <div className="euripus-receiver euripus-receiver--playback">
      <div className="euripus-plyr-shell euripus-plyr-shell--receiver">
        <PlyrSurface
          ariaLabel={`Playing ${displaySourceTitle}`}
          className="euripus-receiver__player"
          onPlaybackFailure={handleReceiverPlaybackFailure}
          source={source}
          uiMode="receiver"
          videoClassName="euripus-plyr-media"
          videoRef={videoRef}
        />
      </div>
      {preparingTranscode ? (
        <ReceiverStatusScreen
          overlay
          role="status"
          title="Preparing a compatible stream"
        >
          <ReceiverSpinner />
        </ReceiverStatusScreen>
      ) : startingPlayback ? (
        <ReceiverStatusScreen
          overlay
          role="status"
          title="Loading stream"
        >
          <ReceiverSpinner />
        </ReceiverStatusScreen>
      ) : null}
    </div>
  );
}
