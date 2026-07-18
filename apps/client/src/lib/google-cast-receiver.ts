const CAST_RECEIVER_SDK_URL =
  "https://www.gstatic.com/cast/sdk/libs/caf_receiver/v3/cast_receiver_framework.js";

export const EURIPUS_CAST_NAMESPACE = "urn:x-cast:se.olivermarcusson.euripus.receiver";

type CastReceiverContext = {
  addCustomMessageListener: (
    namespace: string,
    listener: (event: unknown) => void,
  ) => void;
  addEventListener: (eventType: string, listener: () => void) => void;
  sendCustomMessage: (
    namespace: string,
    senderId: string | undefined,
    message: unknown,
  ) => void;
  start: (options?: Record<string, unknown>) => void;
};

type CastReceiverWindow = Window & {
  cast?: {
    framework?: {
      CastReceiverContext: { getInstance: () => CastReceiverContext };
      system: { EventType: { SENDER_CONNECTED: string } };
    };
  };
};

export type CastReceiverStatus = {
  type: "receiver_status";
  deviceId: string;
  paired: boolean;
  pairingCode: string | null;
};

let receiverContext: CastReceiverContext | null = null;
let initialization: Promise<boolean> | null = null;
let pendingStatus: CastReceiverStatus | null = null;

export function isGoogleCastReceiver() {
  if (typeof window === "undefined") {
    return false;
  }

  return (
    new URLSearchParams(window.location.search).get("cast") === "1" ||
    /CrKey|GoogleTV/i.test(window.navigator.userAgent)
  );
}

function sendPendingStatus() {
  if (!receiverContext || !pendingStatus) {
    return;
  }
  receiverContext.sendCustomMessage(
    EURIPUS_CAST_NAMESPACE,
    undefined,
    pendingStatus,
  );
}

export function publishGoogleCastReceiverStatus(status: CastReceiverStatus) {
  pendingStatus = status;
  sendPendingStatus();
}

export function initializeGoogleCastReceiver() {
  if (!isGoogleCastReceiver()) {
    return Promise.resolve(false);
  }
  if (initialization) {
    return initialization;
  }

  initialization = new Promise<boolean>((resolve) => {
    const targetWindow = window as CastReceiverWindow;
    const start = () => {
      const framework = targetWindow.cast?.framework;
      if (!framework) {
        resolve(false);
        return;
      }

      receiverContext = framework.CastReceiverContext.getInstance();
      // CAF requires custom namespaces to be registered before start().
      receiverContext.addCustomMessageListener(
        EURIPUS_CAST_NAMESPACE,
        () => undefined,
      );
      receiverContext.addEventListener(
        framework.system.EventType.SENDER_CONNECTED,
        sendPendingStatus,
      );
      receiverContext.start({
        disableIdleTimeout: true,
        statusText: "Euripus Receiver",
      });
      sendPendingStatus();
      resolve(true);
    };

    if (targetWindow.cast?.framework) {
      start();
      return;
    }

    const script = document.createElement("script");
    script.src = CAST_RECEIVER_SDK_URL;
    script.async = true;
    script.onload = start;
    script.onerror = () => resolve(false);
    document.head.append(script);
  });

  return initialization;
}
