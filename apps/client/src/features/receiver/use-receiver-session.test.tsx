import { act, cleanup, renderHook } from "@testing-library/react";
import type { ReceiverSession } from "@euripus/shared";
import {
  createReceiverSession,
  heartbeatReceiver,
  issueReceiverPairingCode,
} from "@/lib/api";
import {
  receiverSessionRenewalDelay,
  useReceiverSession,
} from "@/features/receiver/use-receiver-session";

vi.mock("@/lib/api", () => ({
  createReceiverSession: vi.fn(),
  heartbeatReceiver: vi.fn(),
  issueReceiverPairingCode: vi.fn(),
}));

const NOW = new Date("2026-07-31T12:00:00.000Z");

function receiverSession(
  overrides: Partial<ReceiverSession> = {},
): ReceiverSession {
  return {
    sessionToken: "session-1",
    expiresAt: new Date(NOW.getTime() + 12 * 60 * 60_000).toISOString(),
    receiverCredential: null,
    pairingCode: null,
    paired: true,
    device: {
      id: "device-1",
      name: "Google Cast receiver",
      platform: "google-cast",
      formFactorHint: "tv",
      appKind: "receiver-google-cast",
      remembered: true,
      online: true,
      currentController: false,
      lastSeenAt: NOW.toISOString(),
      updatedAt: NOW.toISOString(),
      currentPlayback: null,
      playbackStateStale: false,
    },
    ...overrides,
  };
}

function renderReceiverSession() {
  const persistReceiverCredential = vi.fn();
  const hook = renderHook(() =>
    useReceiverSession({
      castReceiver: true,
      deviceKey: "device-key-1",
      initialReceiverCredential: null,
      persistReceiverCredential,
    }),
  );
  return { ...hook, persistReceiverCredential };
}

describe("useReceiverSession", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
    vi.mocked(createReceiverSession).mockReset();
    vi.mocked(heartbeatReceiver).mockReset().mockResolvedValue(undefined);
    vi.mocked(issueReceiverPairingCode).mockReset();
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("renews the session five minutes before it expires", async () => {
    const firstSession = receiverSession({
      expiresAt: new Date(NOW.getTime() + 6 * 60_000).toISOString(),
      receiverCredential: "credential-1",
    });
    const renewedSession = receiverSession({
      sessionToken: "session-2",
      receiverCredential: "credential-1",
    });
    vi.mocked(createReceiverSession)
      .mockResolvedValueOnce(firstSession)
      .mockResolvedValueOnce(renewedSession);

    const { result, persistReceiverCredential } = renderReceiverSession();
    await act(async () => Promise.resolve());

    expect(result.current.session).toEqual(firstSession);
    expect(persistReceiverCredential).toHaveBeenCalledWith("credential-1");

    await act(async () => {
      await vi.advanceTimersByTimeAsync(60_000);
    });

    expect(result.current.session).toEqual(renewedSession);
    expect(createReceiverSession).toHaveBeenLastCalledWith(
      expect.objectContaining({ receiverCredential: "credential-1" }),
    );
  });

  it("renews immediately when a heartbeat reports an expired token", async () => {
    const firstSession = receiverSession();
    const renewedSession = receiverSession({ sessionToken: "session-2" });
    vi.mocked(createReceiverSession)
      .mockResolvedValueOnce(firstSession)
      .mockResolvedValueOnce(renewedSession);
    vi.mocked(heartbeatReceiver)
      .mockRejectedValueOnce({ status: 401 })
      .mockResolvedValue(undefined);

    const { result } = renderReceiverSession();
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(createReceiverSession).toHaveBeenCalledTimes(2);
    expect(result.current.session?.sessionToken).toBe("session-2");
  });

  it("refreshes an unclaimed pairing code before it expires", async () => {
    vi.mocked(createReceiverSession).mockResolvedValue(
      receiverSession({ pairingCode: "ABCD", paired: false }),
    );
    vi.mocked(issueReceiverPairingCode).mockResolvedValue({
      code: "EFGH",
      expiresAt: new Date(NOW.getTime() + 9 * 60_000).toISOString(),
      device: receiverSession().device,
    });

    const { result } = renderReceiverSession();
    await act(async () => Promise.resolve());
    expect(result.current.pairingCode).toBe("ABCD");

    await act(async () => {
      await vi.advanceTimersByTimeAsync(4 * 60_000);
    });

    expect(issueReceiverPairingCode).toHaveBeenCalledWith("session-1");
    expect(result.current.pairingCode).toBe("EFGH");
  });

  it("uses the credential received when pairing for later renewals", async () => {
    vi.mocked(createReceiverSession)
      .mockResolvedValueOnce(
        receiverSession({
          expiresAt: new Date(NOW.getTime() + 6 * 60_000).toISOString(),
          pairingCode: "ABCD",
          paired: false,
        }),
      )
      .mockResolvedValueOnce(receiverSession({ sessionToken: "session-2" }));

    const { result, persistReceiverCredential } = renderReceiverSession();
    await act(async () => Promise.resolve());
    act(() => result.current.completePairing("paired-credential"));

    await act(async () => {
      await vi.advanceTimersByTimeAsync(60_000);
    });

    expect(persistReceiverCredential).toHaveBeenCalledWith(
      "paired-credential",
    );
    expect(createReceiverSession).toHaveBeenLastCalledWith(
      expect.objectContaining({ receiverCredential: "paired-credential" }),
    );
  });
});

describe("receiverSessionRenewalDelay", () => {
  it("renews immediately when the session is already inside the safety margin", () => {
    expect(
      receiverSessionRenewalDelay(
        new Date(NOW.getTime() + 60_000).toISOString(),
        NOW.getTime(),
      ),
    ).toBe(0);
  });
});
