import type { SportsAvailability, SportsEvent } from "@euripus/shared";
import {
  Dumbbell,
  LandPlot,
  type LucideIcon,
  ShoppingBasket,
  Trophy,
  Volleyball,
} from "lucide-react";
import { formatDateTime, formatRelativeTime, formatTimeRange } from "@/lib/utils";

const COMPETITION_LABELS: Record<string, string> = {
  allsvenskan: "Allsvenskan",
  superettan: "Superettan",
  premier_league: "Premier League",
  uefa_champions_league: "UEFA Champions League",
  pga_tour: "PGA Tour",
  fifa_world_cup: "FIFA World Cup",
  shl: "SHL",
  hockeyallsvenskan: "HockeyAllsvenskan",
  bandy_elitserien: "Bandy Elitserien",
};

export function formatCompetitionLabel(slug: string) {
  return (
    COMPETITION_LABELS[slug] ??
    slug
      .split("_")
      .filter(Boolean)
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join(" ")
  );
}

export function formatSportLabel(sport: string) {
  return sport
    .split(/[_-]/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

export function getSportIcon(sport: string): LucideIcon {
  switch (sport.trim().toLowerCase()) {
    case "soccer":
    case "football":
    case "futsal":
      return Volleyball;
    case "golf":
      return LandPlot;
    case "hockey":
    case "ice_hockey":
    case "ice-hockey":
      return Trophy;
    case "basketball":
      return ShoppingBasket;
    case "volleyball":
    case "beach_volleyball":
    case "beach-volleyball":
      return Volleyball;
    case "mma":
    case "boxing":
    case "wrestling":
      return Dumbbell;
    default:
      return Trophy;
  }
}

export function formatEventSchedule(event: SportsEvent) {
  return formatDateTime(event.startTime);
}

export function formatEventTimeRange(event: SportsEvent) {
  if (!event.endTime) {
    return `Starts ${formatDateTime(event.startTime)}`;
  }

  return formatTimeRange(event.startTime, event.endTime);
}

export function formatEventStatusLabel(status: string) {
  switch (status) {
    case "live":
      return "Live";
    case "upcoming":
      return "Upcoming";
    case "finished":
      return "Finished";
    case "postponed":
      return "Postponed";
    case "cancelled":
      return "Cancelled";
    default:
      return status.charAt(0).toUpperCase() + status.slice(1);
  }
}

export function getStatusBadgeVariant(status: string) {
  switch (status) {
    case "live":
      return "live" as const;
    case "finished":
      return "success" as const;
    case "cancelled":
      return "destructive" as const;
    default:
      return "outline" as const;
  }
}

const GENERIC_PARTICIPANT_PATTERN =
  /^(field|tbd|tba|to be announced|to be determined|players?\s+tbd|winner(?:\s+of)?\b)/i;

function normalizeDisplayText(value: string) {
  return value.trim().toLowerCase().replace(/\s+/g, " ");
}

function appendUniqueToken(tokens: string[], token?: string | null) {
  const normalized = token?.trim();
  if (!normalized || tokens.includes(normalized)) {
    return;
  }

  tokens.push(normalized);
}

function normalizeProviderToken(value: string) {
  return value
    .trim()
    .toLowerCase()
    .replace(/\+/g, " plus ")
    .replace(/[^a-z0-9]+/g, "")
    .trim();
}

const EURIPUS_PROVIDER_RULE_ALIASES: Record<string, string> = {
  tv4: "tv4play",
  tv4play: "tv4play",
  espnplus: "espnplus",
  espnplay: "espnplus",
};

export function getEuripusProviderRuleValue(availability: SportsAvailability) {
  const candidates = [
    availability.providerFamily,
    availability.providerLabel,
    availability.channelName,
  ]
    .map((value) => value?.trim())
    .filter(Boolean) as string[];

  for (const candidate of candidates) {
    const normalized = normalizeProviderToken(candidate);
    const canonical = EURIPUS_PROVIDER_RULE_ALIASES[normalized];
    if (canonical) {
      return canonical;
    }
  }

  return availability.providerFamily?.trim().toLowerCase() || null;
}

function isGenericParticipant(value?: string | null) {
  if (!value?.trim()) {
    return true;
  }

  return GENERIC_PARTICIPANT_PATTERN.test(normalizeDisplayText(value));
}

export function formatParticipants(event: SportsEvent) {
  const home = event.participants?.home?.trim();
  const away = event.participants?.away?.trim();
  const homeIsGeneric = isGenericParticipant(home);
  const awayIsGeneric = isGenericParticipant(away);

  if (home && away && !homeIsGeneric && !awayIsGeneric) {
    return `${home} vs ${away}`;
  }

  if (home && !homeIsGeneric && awayIsGeneric) {
    return home;
  }

  if (away && !awayIsGeneric && homeIsGeneric) {
    return away;
  }

  return event.title;
}

export function formatEventSecondaryText(event: SportsEvent) {
  const headline = formatParticipants(event);

  if (event.title?.trim() && normalizeDisplayText(event.title) !== normalizeDisplayText(headline)) {
    return event.title;
  }

  if (
    event.roundLabel?.trim() &&
    normalizeDisplayText(event.roundLabel) !== normalizeDisplayText(headline)
  ) {
    return event.roundLabel;
  }

  return null;
}

export function formatAvailabilityLine(availability: SportsAvailability) {
  const parts = [availability.providerLabel, availability.channelName]
    .map((value) => value?.trim())
    .filter(Boolean);
  return parts.join(" · ") || "Watch guidance available";
}

export function formatAvailabilityMeta(availability: SportsAvailability) {
  return [availability.watchType, availability.market?.toUpperCase()]
    .filter(Boolean)
    .join(" · ");
}

export function getPrimaryAvailability(event: SportsEvent) {
  return event.watch.availabilities[0] ?? null;
}

export function buildEuripusSearchQuery(
  availability: SportsAvailability,
  hint: string,
) {
  const tokens: string[] = [];

  if (availability.market?.trim()) {
    appendUniqueToken(tokens, `country:${availability.market.trim().toLowerCase()}`);
  }

  const providerRuleValue = getEuripusProviderRuleValue(availability);
  if (providerRuleValue) {
    appendUniqueToken(tokens, `provider:${providerRuleValue}`);
  }

  appendUniqueToken(tokens, hint.trim());

  return tokens.join(" ");
}

export function buildEuripusSearchQueries(availability: SportsAvailability) {
  const queries: string[] = [];

  for (const hint of availability.searchHints) {
    const query = buildEuripusSearchQuery(availability, hint);
    if (query && !queries.includes(query)) {
      queries.push(query);
    }
  }

  return queries;
}

export function formatEventRelativeStart(event: SportsEvent) {
  return formatRelativeTime(event.startTime);
}
