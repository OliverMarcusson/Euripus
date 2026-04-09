import { useEffect, useMemo, useRef, useState } from "react";
import type { PlaybackSource, ReceiverSession } from "@euripus/shared";
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

export function ReceiverPage() {
  const initial = useMemo(loadPersistedState, []);
  const [session, setSession] = useState<ReceiverSession | null>(null);
  const [pairingCode, setPairingCode] = useState<string | null>(null);
  const [source, setSource] = useState<PlaybackSource | null>(null);
  const [error, setError] = useState<string | null>(null);
  const videoRef = useRef<HTMLVideoElement | null>(null);

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
      if (payload.source) {
        setSource(payload.source);
      }
      void acknowledgeReceiverCommand(session.sessionToken, payload.command.id, { status: "acknowledged" }).catch(() => undefined);
    });
    events.addEventListener("transport_command", (event) => {
      const payload = JSON.parse((event as MessageEvent<string>).data) as RemoteDeviceEventPayload;
      const video = videoRef.current;
      if (video) {
        if (payload.command.commandType === "pause") {
          void video.pause();
        } else if (payload.command.commandType === "play") {
          void video.play().catch(() => undefined);
        } else if (payload.command.commandType === "seek" && typeof payload.positionSeconds === "number") {
          video.currentTime = payload.positionSeconds;
        } else if (payload.command.commandType === "stop") {
          video.pause();
          setSource(null);
        }
      }
      void acknowledgeReceiverCommand(session.sessionToken, payload.command.id, { status: "acknowledged" }).catch(() => undefined);
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

    const video = videoRef.current;
    const sync = () =>
      void updateReceiverPlaybackState(session.sessionToken, {
        title: source?.title ?? null,
        sourceKind: source?.kind ?? null,
        live: source?.live ?? null,
        catchup: source?.catchup ?? null,
        paused: video ? video.paused : true,
        positionSeconds: video ? video.currentTime : null,
        durationSeconds: video && Number.isFinite(video.duration) ? video.duration : null,
      }).catch(() => undefined);

    sync();
    if (!video) {
      return;
    }
    video.addEventListener("pause", sync);
    video.addEventListener("play", sync);
    video.addEventListener("timeupdate", sync);
    return () => {
      video.removeEventListener("pause", sync);
      video.removeEventListener("play", sync);
      video.removeEventListener("timeupdate", sync);
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
      {source.kind === "unsupported" ? (
        <main className="relative grid min-h-screen place-items-center px-6 py-10">
          <div className="max-w-2xl rounded-lg border border-amber-400/30 bg-amber-400/10 p-6 text-amber-100">
            {source.unsupportedReason ?? "This stream is not supported on the receiver."}
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
