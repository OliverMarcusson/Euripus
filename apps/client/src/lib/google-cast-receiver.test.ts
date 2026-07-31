import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  EURIPUS_CAST_NAMESPACE,
  isGoogleCastReceiver,
} from "@/lib/google-cast-receiver";

const BOOTSTRAP_SOURCE = readFileSync(
  resolve(process.cwd(), "public/cast-receiver-bootstrap.js"),
  "utf8",
);

type SenderMessageListener = (event: { data: unknown }) => void;

function installCastFramework() {
  const sentMessages: unknown[] = [];
  const messageListeners: SenderMessageListener[] = [];
  const senderConnectedListeners: Array<() => void> = [];
  const startOptions: Array<Record<string, unknown>> = [];

  const context = {
    addCustomMessageListener: (_namespace: string, listener: SenderMessageListener) => {
      messageListeners.push(listener);
    },
    addEventListener: (_eventType: string, listener: () => void) => {
      senderConnectedListeners.push(listener);
    },
    sendCustomMessage: (_namespace: string, _senderId: undefined, message: unknown) => {
      sentMessages.push(message);
    },
    start: (options: Record<string, unknown>) => {
      startOptions.push(options);
    },
  };

  (window as unknown as { cast: unknown }).cast = {
    framework: {
      CastReceiverContext: { getInstance: () => context },
      system: { EventType: { SENDER_CONNECTED: "senderconnected" } },
    },
  };

  return { sentMessages, messageListeners, senderConnectedListeners, startOptions };
}

function runBootstrap() {
  new Function(BOOTSTRAP_SOURCE)();
  return (window as unknown as { __euripusCastReceiver?: {
    started: boolean;
    failed: boolean;
    publish: (status: unknown) => void;
  } }).__euripusCastReceiver;
}

const STATUS = {
  type: "receiver_status" as const,
  deviceId: "device-1",
  paired: false,
  pairingCode: "ABCD",
};

describe("cast receiver bootstrap", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    window.history.replaceState({}, "", "/receiver?cast=1");
    delete (window as unknown as { cast?: unknown }).cast;
    delete (window as unknown as { __euripusCastReceiver?: unknown })
      .__euripusCastReceiver;
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("uses the same namespace as the sender", () => {
    expect(BOOTSTRAP_SOURCE).toContain(`"${EURIPUS_CAST_NAMESPACE}"`);
  });

  it("ignores pages that are not a Cast receiver", () => {
    window.history.replaceState({}, "", "/guide");
    installCastFramework();

    expect(runBootstrap()).toBeUndefined();
  });

  it("starts the receiver framework without waiting for a published status", () => {
    const framework = installCastFramework();

    const receiver = runBootstrap();

    expect(receiver?.started).toBe(true);
    expect(framework.startOptions).toEqual([
      { disableIdleTimeout: true, statusText: "Euripus Receiver" },
    ]);
    expect(framework.sentMessages).toEqual([]);
  });

  it("repeats the status until a sender acknowledges it", () => {
    const framework = installCastFramework();
    const receiver = runBootstrap();

    receiver?.publish(STATUS);
    expect(framework.sentMessages).toEqual([STATUS]);

    vi.advanceTimersByTime(4_000);
    expect(framework.sentMessages).toEqual([STATUS, STATUS, STATUS]);

    framework.messageListeners.forEach((listener) => {
      listener({ data: { type: "receiver_status_ack" } });
    });
    vi.advanceTimersByTime(10_000);
    expect(framework.sentMessages).toEqual([STATUS, STATUS, STATUS]);
  });

  it("re-announces when a sender asks for the status", () => {
    const framework = installCastFramework();
    const receiver = runBootstrap();

    receiver?.publish(STATUS);
    framework.messageListeners.forEach((listener) => {
      listener({ data: { type: "receiver_status_ack" } });
    });
    vi.advanceTimersByTime(10_000);
    framework.sentMessages.length = 0;

    framework.messageListeners.forEach((listener) => {
      listener({ data: JSON.stringify({ type: "request_receiver_status" }) });
    });

    expect(framework.sentMessages).toEqual([STATUS]);
  });

  it("announces a status published before a sender connects", () => {
    const framework = installCastFramework();
    const receiver = runBootstrap();

    receiver?.publish(STATUS);
    framework.messageListeners.forEach((listener) => {
      listener({ data: { type: "receiver_status_ack" } });
    });
    framework.sentMessages.length = 0;

    framework.senderConnectedListeners.forEach((listener) => {
      listener();
    });

    expect(framework.sentMessages).toEqual([STATUS]);
  });

  it("reports a Cast receiver from the query parameter", () => {
    expect(isGoogleCastReceiver()).toBe(true);
    window.history.replaceState({}, "", "/receiver");
    expect(isGoogleCastReceiver()).toBe(false);
  });
});
