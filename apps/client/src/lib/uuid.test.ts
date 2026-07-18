import { describe, expect, it, vi } from "vitest";
import { createUuid } from "@/lib/uuid";

describe("createUuid", () => {
  it("uses crypto.randomUUID when the runtime provides it", () => {
    const randomUUID = vi.fn(() => "native-uuid");

    expect(createUuid({ randomUUID })).toBe("native-uuid");
    expect(randomUUID).toHaveBeenCalledOnce();
  });

  it("creates an RFC 4122 UUID with getRandomValues on older runtimes", () => {
    const getRandomValues = vi.fn((bytes: Uint8Array) => {
      bytes.fill(0);
      return bytes;
    });

    expect(createUuid({ getRandomValues })).toBe(
      "00000000-0000-4000-8000-000000000000",
    );
    expect(getRandomValues).toHaveBeenCalledOnce();
  });
});
