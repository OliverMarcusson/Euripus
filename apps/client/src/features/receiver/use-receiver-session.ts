import { useCallback, useEffect, useRef, useState } from "react";
import type { ReceiverSession } from "@euripus/shared";
import {
  createReceiverSession,
  heartbeatReceiver,
  issueReceiverPairingCode,
} from "@/lib/api";

const RECEIVER_HEARTBEAT_MS = 15_000;
const RECEIVER_SESSION_RENEWAL_MARGIN_MS = 5 * 60_000;
const RECEIVER_PAIRING_CODE_RENEWAL_MS = 4 * 60_000;
const RECEIVER_RETRY_MS = 5_000;

type UseReceiverSessionOptions = {
  castReceiver: boolean;
  deviceKey: string;
  initialReceiverCredential: string | null;
  persistReceiverCredential: (credential: string) => void;
};

export function receiverSessionRenewalDelay(
  expiresAt: string,
  now = Date.now(),
) {
  const expiry = Date.parse(expiresAt);
  if (!Number.isFinite(expiry)) {
    return 0;
  }
  return Math.max(0, expiry - now - RECEIVER_SESSION_RENEWAL_MARGIN_MS);
}

function isUnauthorized(error: unknown) {
  return (
    typeof error === "object" &&
    error !== null &&
    "status" in error &&
    error.status === 401
  );
}

export function useReceiverSession({
  castReceiver,
  deviceKey,
  initialReceiverCredential,
  persistReceiverCredential,
}: UseReceiverSessionOptions) {
  const [session, setSession] = useState<ReceiverSession | null>(null);
  const [pairingCode, setPairingCode] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const receiverCredentialRef = useRef(initialReceiverCredential);
  const sessionRequestRef = useRef<Promise<ReceiverSession> | null>(null);
  const mountedRef = useRef(false);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const rememberReceiverCredential = useCallback(
    (credential: string | null | undefined) => {
      if (!credential) {
        return;
      }
      receiverCredentialRef.current = credential;
      persistReceiverCredential(credential);
    },
    [persistReceiverCredential],
  );

  const renewSession = useCallback(() => {
    if (sessionRequestRef.current) {
      return sessionRequestRef.current;
    }

    let request: Promise<ReceiverSession>;
    request = createReceiverSession({
      deviceKey,
      name: castReceiver ? "Google Cast receiver" : "Browser receiver",
      platform: castReceiver ? "google-cast" : "web",
      formFactorHint: castReceiver ? "tv" : detectFormFactorHint(),
      appKind: castReceiver ? "receiver-google-cast" : "receiver-web",
      publicOrigin:
        typeof window === "undefined" ? null : window.location.origin,
      receiverCredential: receiverCredentialRef.current,
    })
      .then((nextSession) => {
        rememberReceiverCredential(nextSession.receiverCredential);
        if (mountedRef.current) {
          setSession(nextSession);
          setPairingCode(nextSession.pairingCode);
          setError(null);
        }
        return nextSession;
      })
      .finally(() => {
        if (sessionRequestRef.current === request) {
          sessionRequestRef.current = null;
        }
      });

    sessionRequestRef.current = request;
    return request;
  }, [castReceiver, deviceKey, rememberReceiverCredential]);

  useEffect(() => {
    void renewSession().catch((nextError) => {
      if (mountedRef.current) {
        setError(
          nextError instanceof Error
            ? nextError.message
            : "Receiver startup failed.",
        );
      }
    });
  }, [renewSession]);

  useEffect(() => {
    if (!session) {
      return;
    }

    let active = true;
    let timer = 0;
    const renewWithRetry = () => {
      void renewSession().catch(() => {
        if (active) {
          timer = window.setTimeout(renewWithRetry, RECEIVER_RETRY_MS);
        }
      });
    };

    timer = window.setTimeout(
      renewWithRetry,
      receiverSessionRenewalDelay(session.expiresAt),
    );
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [renewSession, session]);

  useEffect(() => {
    if (!session?.sessionToken) {
      return;
    }

    const heartbeat = () => {
      void heartbeatReceiver(session.sessionToken).catch((heartbeatError) => {
        if (isUnauthorized(heartbeatError)) {
          void renewSession().catch(() => undefined);
        }
      });
    };
    heartbeat();
    const timer = window.setInterval(heartbeat, RECEIVER_HEARTBEAT_MS);
    return () => window.clearInterval(timer);
  }, [renewSession, session?.sessionToken]);

  useEffect(() => {
    if (!pairingCode || !session?.sessionToken) {
      return;
    }

    const sessionToken = session.sessionToken;
    let active = true;
    let timer = 0;
    function scheduleRefresh(delay: number) {
      timer = window.setTimeout(refreshPairingCode, delay);
    }
    function refreshPairingCode() {
      void issueReceiverPairingCode(sessionToken)
        .then((pairing) => {
          if (active) {
            setPairingCode(pairing.code);
            scheduleRefresh(RECEIVER_PAIRING_CODE_RENEWAL_MS);
          }
        })
        .catch((pairingError) => {
          if (!active) {
            return;
          }
          if (isUnauthorized(pairingError)) {
            void renewSession().catch(() => undefined);
            return;
          }
          scheduleRefresh(RECEIVER_RETRY_MS);
        });
    }

    scheduleRefresh(RECEIVER_PAIRING_CODE_RENEWAL_MS);
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [pairingCode, renewSession, session?.sessionToken]);

  const completePairing = useCallback(
    (receiverCredential?: string | null) => {
      rememberReceiverCredential(receiverCredential);
      setPairingCode(null);
    },
    [rememberReceiverCredential],
  );

  return {
    completePairing,
    error,
    pairingCode,
    renewSession,
    session,
  };
}

function detectFormFactorHint() {
  if (typeof window === "undefined") {
    return "large-screen";
  }
  return window.innerWidth >= 960 ? "large-screen" : "desktop";
}
