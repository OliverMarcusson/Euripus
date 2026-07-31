export const EURIPUS_CAST_NAMESPACE = "urn:x-cast:se.olivermarcusson.euripus.receiver";

const BOOTSTRAP_READY_TIMEOUT_MS = 15_000;
const BOOTSTRAP_POLL_INTERVAL_MS = 100;

export type CastReceiverStatus = {
  type: "receiver_status";
  deviceId: string;
  paired: boolean;
  pairingCode: string | null;
};

/**
 * Installed by `public/cast-receiver-bootstrap.js`, which starts the Cast
 * receiver framework before this bundle loads. See that file for why.
 */
type CastReceiverBootstrap = {
  namespace: string;
  status: CastReceiverStatus | null;
  started: boolean;
  failed: boolean;
  publish: (status: CastReceiverStatus) => void;
};

type CastReceiverWindow = Window & {
  __euripusCastReceiver?: CastReceiverBootstrap;
};

function bootstrap() {
  if (typeof window === "undefined") {
    return undefined;
  }
  return (window as CastReceiverWindow).__euripusCastReceiver;
}

export function isGoogleCastReceiver() {
  if (typeof window === "undefined") {
    return false;
  }

  return (
    new URLSearchParams(window.location.search).get("cast") === "1" ||
    /CrKey|GoogleTV/i.test(window.navigator.userAgent)
  );
}

export function publishGoogleCastReceiverStatus(status: CastReceiverStatus) {
  bootstrap()?.publish(status);
}

/**
 * Resolves once the bootstrap has started the Cast receiver framework, or
 * false if it gave up. The framework starts independently of this call; this
 * only exists so the receiver UI can report a failed start.
 */
export function initializeGoogleCastReceiver() {
  if (!isGoogleCastReceiver()) {
    return Promise.resolve(false);
  }

  return new Promise<boolean>((resolve) => {
    const deadline = Date.now() + BOOTSTRAP_READY_TIMEOUT_MS;
    const poll = () => {
      const current = bootstrap();
      if (current?.started) {
        resolve(true);
        return;
      }
      if (!current || current.failed || Date.now() >= deadline) {
        resolve(false);
        return;
      }
      window.setTimeout(poll, BOOTSTRAP_POLL_INTERVAL_MS);
    };
    poll();
  });
}
