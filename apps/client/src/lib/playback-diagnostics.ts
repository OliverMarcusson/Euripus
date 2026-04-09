type PlaybackLogLevel = "info" | "warn" | "error";

type PlaybackDiagnosticPayload = Record<string, unknown>;

type PlaybackOwnershipHint =
  | "client-player"
  | "euripus-relay"
  | "iptv-provider"
  | "unknown";

function isRelayUrl(value: string | null | undefined) {
  if (!value) {
    return false;
  }

  try {
    const url = new URL(value, window.location.origin);
    return url.origin === window.location.origin && url.pathname.startsWith("/api/relay/");
  } catch {
    return false;
  }
}

export function inferPlaybackOwnershipHint(url: string | null | undefined): PlaybackOwnershipHint {
  if (isRelayUrl(url)) {
    return "euripus-relay";
  }

  if (url) {
    return "iptv-provider";
  }

  return "unknown";
}

export function logPlaybackDiagnostic(
  level: PlaybackLogLevel,
  event: string,
  payload: PlaybackDiagnosticPayload,
) {
  const logger =
    level === "error"
      ? console.error
      : level === "warn"
        ? console.warn
        : console.info;

  logger("[playback]", {
    event,
    timestamp: new Date().toISOString(),
    ...payload,
  });
}
