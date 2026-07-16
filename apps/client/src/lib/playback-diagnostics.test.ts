import { describe, expect, it } from "vitest";
import { sanitizePlaybackDiagnosticUrl } from "@/lib/playback-diagnostics";

describe("playback diagnostic URL sanitization", () => {
  it("removes signed query values and fragments while retaining origin and path", () => {
    expect(
      sanitizePlaybackDiagnosticUrl(
        "https://app.example.com/api/relay/hls?token=secret#fragment",
      ),
    ).toBe("https://app.example.com/api/relay/hls");
    expect(
      sanitizePlaybackDiagnosticUrl(
        "/api/relay/raw?token=segment-secret#fragment",
      ),
    ).toBe("/api/relay/raw");
  });

  it("redacts query and fragment text even when a diagnostic URL is malformed", () => {
    expect(
      sanitizePlaybackDiagnosticUrl("http://[invalid]?token=secret#fragment"),
    ).toBe("http://[invalid]");
  });
});
