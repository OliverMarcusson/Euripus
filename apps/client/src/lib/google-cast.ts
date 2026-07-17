import { create } from "zustand";
import { EURIPUS_CAST_NAMESPACE } from "@/lib/google-cast-receiver";

const CAST_SENDER_SDK_URL =
  "https://www.gstatic.com/cv/js/sender/v1/cast_sender.js?loadCastFramework=1";

// Registered for https://tv.marcusson.dev/receiver?cast=1.
export const EURIPUS_CAST_RECEIVER_APP_ID = "EEC1D3B6";

const APP_ID_CONFIGURED = true;

type ReceiverStatusMessage = {
  type: "receiver_status";
  deviceId: string;
  paired: boolean;
  pairingCode: string | null;
};

type CastSession = {
  addMessageListener: (
    namespace: string,
    listener: (namespace: string, message: unknown) => void,
  ) => void;
  getCastDevice: () => { friendlyName?: string };
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
    };
  };
  chrome?: {
    cast: {
      AutoJoinPolicy: { ORIGIN_SCOPED: string };
    };
  };
};

type GoogleCastState = {
  appIdConfigured: boolean;
  available: boolean;
  connected: boolean;
  deviceName: string | null;
  initialized: boolean;
  initializing: boolean;
  receiverDeviceId: string | null;
  receiverPaired: boolean;
  receiverPairingCode: string | null;
};

const initialState: GoogleCastState = {
  appIdConfigured: APP_ID_CONFIGURED,
  available: false,
  connected: false,
  deviceName: null,
  initialized: false,
  initializing: false,
  receiverDeviceId: null,
  receiverPaired: false,
  receiverPairingCode: null,
};

export const useGoogleCastStore = create<GoogleCastState>(() => initialState);

let castContext: CastContext | null = null;
let initialization: Promise<boolean> | null = null;
const observedSessions = new WeakSet<object>();

function castWindow() {
  return window as CastWindow;
}

function parseReceiverStatus(message: unknown): ReceiverStatusMessage | null {
  try {
    const parsed = typeof message === "string" ? JSON.parse(message) : message;
    if (
      typeof parsed === "object" &&
      parsed !== null &&
      "type" in parsed &&
      parsed.type === "receiver_status" &&
      "deviceId" in parsed &&
      typeof parsed.deviceId === "string" &&
      "paired" in parsed &&
      typeof parsed.paired === "boolean" &&
      "pairingCode" in parsed &&
      (typeof parsed.pairingCode === "string" || parsed.pairingCode === null)
    ) {
      return parsed as ReceiverStatusMessage;
    }
  } catch {
    // Ignore malformed messages from receiver applications.
  }
  return null;
}

function observeSession(session: CastSession) {
  if (observedSessions.has(session)) {
    return;
  }
  observedSessions.add(session);
  session.addMessageListener(EURIPUS_CAST_NAMESPACE, (_namespace, message) => {
    const status = parseReceiverStatus(message);
    if (status) {
      useGoogleCastStore.setState({
        receiverDeviceId: status.deviceId,
        receiverPaired: status.paired,
        receiverPairingCode: status.pairingCode,
      });
    }
  });
}

function syncSessionState() {
  const session = castContext?.getCurrentSession() ?? null;
  if (session) {
    observeSession(session);
  }
  useGoogleCastStore.setState({
    connected: !!session,
    deviceName: session?.getCastDevice().friendlyName ?? null,
    ...(!session
      ? {
          receiverDeviceId: null,
          receiverPaired: false,
          receiverPairingCode: null,
        }
      : {}),
  });
}

function configureCastFramework() {
  const targetWindow = castWindow();
  const framework = targetWindow.cast?.framework;
  const chromeCast = targetWindow.chrome?.cast;
  if (!framework || !chromeCast || !APP_ID_CONFIGURED) {
    return false;
  }

  castContext = framework.CastContext.getInstance();
  castContext.setOptions({
    receiverApplicationId: EURIPUS_CAST_RECEIVER_APP_ID,
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
  if (!APP_ID_CONFIGURED) {
    useGoogleCastStore.setState({ initialized: true });
    return Promise.resolve(false);
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
  if (!APP_ID_CONFIGURED) {
    throw new Error("Register the Euripus Cast receiver App ID first.");
  }
  if (!castContext) {
    throw new Error("Google Cast is not available in this browser.");
  }
  await castContext.requestSession();
  syncSessionState();
}

export function endGoogleCastSession() {
  castContext?.endCurrentSession(true);
  useGoogleCastStore.setState({
    connected: false,
    deviceName: null,
    receiverDeviceId: null,
    receiverPaired: false,
    receiverPairingCode: null,
  });
}
