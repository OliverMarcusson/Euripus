import type { SportsEvent } from "@euripus/shared";
import { LandPlot, Trophy, Volleyball } from "lucide-react";
import {
  buildEuripusSearchQueries,
  buildEuripusSearchQuery,
  formatEventSecondaryText,
  formatParticipants,
  getEuripusProviderRuleValue,
  getSportIcon,
} from "@/features/sports/sports-formatting";

const baseEvent = {
  id: "event-1",
  sport: "golf",
  competition: "pga_tour",
  title: "RBC Heritage Round 2",
  startTime: "2026-04-17T18:00:00.000Z",
  endTime: null,
  status: "upcoming",
  venue: "Harbour Town Golf Links",
  roundLabel: "Round 2",
  participants: {
    home: "RBC Heritage",
    away: "Field",
  },
  source: "pga-tour",
  sourceUrl: null,
  watch: {
    recommendedMarket: "se",
    recommendedProvider: "Max",
    availabilities: [],
  },
  searchMetadata: {
    queries: [],
    keywords: [],
  },
} satisfies SportsEvent;

describe("sports formatting", () => {
  it("uses a cleaner headline when one participant is a generic placeholder", () => {
    expect(formatParticipants(baseEvent)).toBe("RBC Heritage");
    expect(formatEventSecondaryText(baseEvent)).toBe("RBC Heritage Round 2");
  });

  it("keeps head-to-head fixtures intact and avoids duplicate secondary copy", () => {
    const event = {
      ...baseEvent,
      sport: "soccer",
      competition: "allsvenskan",
      title: "Halmstads BK vs IFK Göteborg",
      roundLabel: "Round 3",
      participants: {
        home: "Halmstads BK",
        away: "IFK Göteborg",
      },
    } satisfies SportsEvent;

    expect(formatParticipants(event)).toBe("Halmstads BK vs IFK Göteborg");
    expect(formatEventSecondaryText(event)).toBe("Round 3");
  });

  it("maps sports to sensible icons", () => {
    expect(getSportIcon("soccer")).toBe(Volleyball);
    expect(getSportIcon("golf")).toBe(LandPlot);
    expect(getSportIcon("hockey")).toBe(Trophy);
    expect(getSportIcon("ice_hockey")).toBe(Trophy);
    expect(getSportIcon("unknown-sport")).toBe(Trophy);
  });

  it("builds Euripus search queries that use country/provider rule filters", () => {
    const availability = {
      market: "se",
      providerFamily: "tv4",
      providerLabel: "TV4 Play",
      channelName: "TV4 Fotboll",
      watchType: "streaming+linear",
      confidence: 0.9,
      source: "overlay",
      searchHints: [
        "Halmstads BK vs IFK Göteborg TV4 Play",
        "Halmstads BK vs IFK Göteborg TV4 Play",
      ],
    };

    expect(getEuripusProviderRuleValue(availability)).toBe("tv4play");
    expect(
      buildEuripusSearchQuery(availability, "Halmstads BK vs IFK Göteborg TV4 Play"),
    ).toBe("country:se provider:tv4play Halmstads BK vs IFK Göteborg TV4 Play");
    expect(buildEuripusSearchQueries(availability)).toEqual([
      "country:se provider:tv4play Halmstads BK vs IFK Göteborg TV4 Play",
    ]);
  });

  it("prefers canonical plus-provider rule values from provider labels", () => {
    const availability = {
      market: "us",
      providerFamily: "espn",
      providerLabel: "ESPN+",
      channelName: "Main Feed",
      watchType: "streaming",
      confidence: 0.95,
      source: "overlay",
      searchHints: ["RBC Heritage Main Feed"],
    };

    expect(getEuripusProviderRuleValue(availability)).toBe("espnplus");
    expect(buildEuripusSearchQueries(availability)).toEqual([
      "country:us provider:espnplus RBC Heritage Main Feed",
    ]);
  });
});
