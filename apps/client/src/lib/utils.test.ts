import { describe, expect, it } from "vitest";
import { formatEventChannelTitle } from "@/lib/utils";

describe("formatEventChannelTitle", () => {
  it("converts month-first event times with explicit source time zones", () => {
    expect(
      formatEventChannelTitle("Knicks vs Celtics @ Jan 9 20:55 ET : ESPN PPV 1", {
        targetTimeZone: "UTC",
        now: new Date("2026-01-08T12:00:00.000Z"),
      }),
    ).toBe("Knicks vs Celtics @ Jan 10 01:55 GMT+0 : ESPN PPV 1");
  });

  it("converts weekday-day-month event times with explicit source time zones", () => {
    expect(
      formatEventChannelTitle(
        "ENDED | GOLF MAJOR ON THE RANGE | Wed 08 Apr 15:00 CEST (SE) | 8K EXCLUSIVE | SE: VIAPLAY PPV 2",
        {
          targetTimeZone: "UTC",
          now: new Date("2026-04-07T12:00:00.000Z"),
        },
      ),
    ).toBe(
      "ENDED | GOLF MAJOR ON THE RANGE | Wed 08 Apr 13:00 GMT+0 (SE) | 8K EXCLUSIVE | SE: VIAPLAY PPV 2",
    );
  });

  it("prefers explicit title time zones over program start fallbacks", () => {
    expect(
      formatEventChannelTitle("Knicks vs Celtics @ Jan 9 20:55 ET : ESPN PPV 1", {
        referenceStartAt: "2026-01-09T18:00:00.000Z",
        targetTimeZone: "UTC",
        now: new Date("2026-01-08T12:00:00.000Z"),
      }),
    ).toBe("Knicks vs Celtics @ Jan 10 01:55 GMT+0 : ESPN PPV 1");
  });

  it("uses fixed offsets for season-specific timezone abbreviations", () => {
    expect(
      formatEventChannelTitle("Knicks vs Celtics @ Jul 9 20:55 EST : ESPN PPV 1", {
        targetTimeZone: "UTC",
        now: new Date("2026-07-08T12:00:00.000Z"),
      }),
    ).toBe("Knicks vs Celtics @ Jul 10 01:55 GMT+0 : ESPN PPV 1");
  });

  it("converts month-first titles without @ markers and with 12-hour times", () => {
    expect(
      formatEventChannelTitle(
        "US (ESPN+ 034) | RBC Heritage: Spieth Featured Group (Second Round) Apr 17 2:00PM ET (2026-04-17 14:00:25)",
        {
          targetTimeZone: "UTC",
          now: new Date("2026-04-16T12:00:00.000Z"),
        },
      ),
    ).toBe(
      "US (ESPN+ 034) | RBC Heritage: Spieth Featured Group (Second Round) Apr 17 18:00 GMT+0 (2026-04-17 14:00:25)",
    );
  });

  it("uses program start times when titles omit an explicit source time zone", () => {
    expect(
      formatEventChannelTitle("PSG vs Liverpool @ Apr 9 20:55 : TeliaPlay SE 26", {
        referenceStartAt: "2026-04-09T18:55:00.000Z",
        targetTimeZone: "Europe/Helsinki",
      }),
    ).toBe("PSG vs Liverpool @ Apr 9 21:55 : TeliaPlay SE 26");
  });

  it("leaves unrelated channel titles untouched", () => {
    expect(formatEventChannelTitle("Arena Live")).toBe("Arena Live");
  });
});
