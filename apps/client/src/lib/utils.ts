import type { Channel, Program } from "@euripus/shared";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

const MONTH_TOKEN_TO_NUMBER = new Map<string, number>([
  ["jan", 1],
  ["january", 1],
  ["feb", 2],
  ["february", 2],
  ["mar", 3],
  ["march", 3],
  ["apr", 4],
  ["april", 4],
  ["may", 5],
  ["jun", 6],
  ["june", 6],
  ["jul", 7],
  ["july", 7],
  ["aug", 8],
  ["august", 8],
  ["sep", 9],
  ["sept", 9],
  ["september", 9],
  ["oct", 10],
  ["october", 10],
  ["nov", 11],
  ["november", 11],
  ["dec", 12],
  ["december", 12],
] as const);

const MONTH_TOKEN_PATTERN =
  "Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:t|tember)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?";
const TIME_ZONE_TOKEN_PATTERN =
  "(?:(?:UTC|GMT)[+-]\\d{1,2}(?::?\\d{2})?|[+-]\\d{1,2}(?::?\\d{2})?|ET|EST|EDT|CT|CST|CDT|MT|MST|MDT|PT|PST|PDT|NDT|CET|CEST|EET|EEST|BST|GMT|UTC)";
const TIME_OF_DAY_TOKEN_PATTERN =
  String.raw`(?<time>\d{1,2}:\d{2})(?:\s*(?<meridiem>AM|PM))?`;
const OPTIONAL_TIME_ZONE_PATTERN =
  String.raw`(?:\s*(?:\((?<timeZoneParen>${TIME_ZONE_TOKEN_PATTERN})\)|(?<timeZone>${TIME_ZONE_TOKEN_PATTERN})))?`;
const ISO_EVENT_PATTERN = new RegExp(
  String.raw`(?<year>\d{4})-(?<numericMonth>\d{2})-(?<day>\d{2})(?<separator>\s*(?:\|\s*)?)${TIME_OF_DAY_TOKEN_PATTERN}${OPTIONAL_TIME_ZONE_PATTERN}`,
  "i",
);
const DAY_MONTH_YEAR_EVENT_PATTERN = new RegExp(
  String.raw`(?<day>\d{1,2})-(?<numericMonth>\d{1,2})-(?<year>\d{4})(?<separator>\s*(?:\|\s*)?)${TIME_OF_DAY_TOKEN_PATTERN}${OPTIONAL_TIME_ZONE_PATTERN}`,
  "i",
);
const MONTH_FIRST_EVENT_PATTERN = new RegExp(
  String.raw`(?<marker>@\s*)?(?<month>${MONTH_TOKEN_PATTERN})\s+(?<day>\d{1,2})(?:st|nd|rd|th)?\s+${TIME_OF_DAY_TOKEN_PATTERN}${OPTIONAL_TIME_ZONE_PATTERN}`,
  "i",
);
const WEEKDAY_DAY_MONTH_EVENT_PATTERN = new RegExp(
  String.raw`(?<weekday>Mon|Tue|Wed|Thu|Fri|Sat|Sun)\s+(?<day>\d{1,2})(?:st|nd|rd|th)?\s+(?<month>${MONTH_TOKEN_PATTERN})\s+${TIME_OF_DAY_TOKEN_PATTERN}${OPTIONAL_TIME_ZONE_PATTERN}`,
  "i",
);
const WEEKDAY_MONTH_DAY_EVENT_PATTERN = new RegExp(
  String.raw`(?<weekday>Mon|Tue|Wed|Thu|Fri|Sat|Sun)\s+(?<month>${MONTH_TOKEN_PATTERN})\s+(?<day>\d{1,2})(?:st|nd|rd|th)?\s+${TIME_OF_DAY_TOKEN_PATTERN}${OPTIONAL_TIME_ZONE_PATTERN}`,
  "i",
);
const ISO_DATE_PATTERN = /\b(?<year>\d{4})-(?<numericMonth>\d{2})-(?<day>\d{2})\b/;
const DAY_MONTH_YEAR_DATE_PATTERN = /\b(?<day>\d{1,2})-(?<numericMonth>\d{1,2})-(?<year>\d{4})\b/;

const IANA_TIME_ZONES_BY_TOKEN: Record<string, string> = {
  ET: "America/New_York",
  CT: "America/Chicago",
  MT: "America/Denver",
  PT: "America/Los_Angeles",
};

const FIXED_OFFSET_MINUTES_BY_TOKEN: Record<string, number> = {
  GMT: 0,
  UTC: 0,
  EST: -5 * 60,
  EDT: -4 * 60,
  CST: -6 * 60,
  CDT: -5 * 60,
  MST: -7 * 60,
  MDT: -6 * 60,
  PST: -8 * 60,
  PDT: -7 * 60,
  CET: 1 * 60,
  CEST: 2 * 60,
  EET: 2 * 60,
  EEST: 3 * 60,
  BST: 1 * 60,
  NDT: -(2 * 60 + 30),
};

type EventChannelTitleFormatOptions = {
  referenceStartAt?: string | null;
  targetTimeZone?: string;
  now?: Date;
};

type EventChannelPlaybackOptions = {
  referenceStartAt?: string | null;
  now?: Date;
  liveWindowHours?: number;
};

type EventTimestampMatch = {
  kind: "month-first" | "weekday-day-month" | "iso" | "day-month-year";
  match: RegExpExecArray;
  marker?: string;
  separator?: string;
  year: number | null;
  month: number;
  day: number;
  dayText: string;
  hour: number;
  minute: number;
  meridiem: "AM" | "PM" | null;
  timeZoneToken: string | null;
};

type EventDateParts = {
  year: number | null;
  month: number;
  day: number;
  hour: number | null;
  minute: number | null;
  timeZoneToken: string | null;
};

type NumericDateTimeParts = {
  year: number;
  month: number;
  day: number;
  hour: number;
  minute: number;
};

type DisplayDateTimeParts = NumericDateTimeParts & {
  weekday: string;
  monthShort: string;
  dayText: string;
  hourText: string;
  minuteText: string;
};

type SourceTimeZone =
  | { kind: "iana"; timeZone: string }
  | { kind: "offset"; offsetMinutes: number };

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatDateTime(value: string | null) {
  if (!value) {
    return "Never";
  }

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
    hour12: false,
  }).format(new Date(value));
}

export function formatRelativeTime(value: string | null) {
  if (!value) {
    return "Never";
  }

  const diffMs = new Date(value).getTime() - Date.now();
  const formatter = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
  const units = [
    { unit: "year", ms: 1000 * 60 * 60 * 24 * 365 },
    { unit: "month", ms: 1000 * 60 * 60 * 24 * 30 },
    { unit: "week", ms: 1000 * 60 * 60 * 24 * 7 },
    { unit: "day", ms: 1000 * 60 * 60 * 24 },
    { unit: "hour", ms: 1000 * 60 * 60 },
    { unit: "minute", ms: 1000 * 60 },
  ] as const;

  for (const { unit, ms } of units) {
    if (Math.abs(diffMs) >= ms || unit === "minute") {
      return formatter.format(Math.round(diffMs / ms), unit);
    }
  }

  return "Just now";
}

export function formatTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    hour: "numeric",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(value));
}

export function formatTimeRange(startAt: string, endAt: string) {
  return `${formatTime(startAt)} - ${formatTime(endAt)}`;
}

export function formatEventChannelTitle(
  title: string,
  options: EventChannelTitleFormatOptions = {},
) {
  const resolvedEvent = resolveEventChannelTimestamp(title, options);
  if (!resolvedEvent) {
    return title;
  }

  const replacement = formatEventTimestampReplacement(resolvedEvent.eventMatch, resolvedEvent.eventDate, {
    includeTimeZoneLabel: Boolean(resolvedEvent.eventMatch.timeZoneToken),
    targetTimeZone: options.targetTimeZone,
  });

  return title.replace(resolvedEvent.eventMatch.match[0], replacement);
}

export type EventChannelPlaybackState = "live" | "upcoming" | "info";

export function getEventChannelPlaybackState(
  title: string,
  options: EventChannelPlaybackOptions = {},
): EventChannelPlaybackState {
  const resolvedEvent = resolveEventChannelTimestamp(title, options);
  if (!resolvedEvent) {
    return "info";
  }

  const now = (options.now ?? new Date()).getTime();
  const startAt = resolvedEvent.eventDate.getTime();
  if (Number.isNaN(startAt)) {
    return "info";
  }

  if (startAt > now) {
    return "upcoming";
  }

  const liveWindowHours = options.liveWindowHours ?? 10;
  if (now - startAt <= liveWindowHours * 60 * 60 * 1000) {
    return "live";
  }

  return "info";
}

export function getTimeProgress(startAt: string, endAt: string, now = Date.now()) {
  const start = new Date(startAt).getTime();
  const end = new Date(endAt).getTime();

  if (Number.isNaN(start) || Number.isNaN(end) || end <= start) {
    return 0;
  }

  const progress = ((now - start) / (end - start)) * 100;
  return Math.max(0, Math.min(100, progress));
}

export function formatArchiveDuration(hours: number | null) {
  if (!hours) {
    return null;
  }

  return hours === 1 ? "1 hour archive" : `${hours} hour archive`;
}

export type ProgramPlaybackState = "live" | "catchup" | "upcoming" | "info";

export function getProgramPlaybackState(program: Program, now = Date.now()): ProgramPlaybackState {
  const start = new Date(program.startAt).getTime();
  const end = new Date(program.endAt).getTime();

  if (Number.isNaN(start) || Number.isNaN(end)) {
    return "info";
  }

  if (start <= now && end > now && program.channelId) {
    return "live";
  }

  if (end <= now && program.canCatchup) {
    return "catchup";
  }

  if (start > now) {
    return "upcoming";
  }

  return "info";
}

export function canPlayProgram(program: Program, now = Date.now()) {
  const state = getProgramPlaybackState(program, now);
  return state === "live" || state === "catchup";
}

export function isPpvChannel(channel: Pick<Channel, "isPpv" | "categoryName">) {
  return Boolean(channel.isPpv || /\bppv\b/i.test(channel.categoryName ?? ""));
}

export function shouldShowChannelForPpvDateFilter(
  channel: Pick<Channel, "name" | "isPpv" | "categoryName">,
  options: { enabled: boolean; now?: Date; targetTimeZone?: string },
) {
  if (!options.enabled || !isPpvChannel(channel)) {
    return true;
  }

  const now = options.now ?? new Date();
  const event = detectEventDateParts(channel.name);
  if (!event) {
    return true;
  }

  if (!event.timeZoneToken) {
    const year = event.year ?? inferClosestLocalYear(event.month, event.day, now);
    return sameCalendarDate(
      { year, month: event.month, day: event.day },
      getNumericDateTimeParts(now, options.targetTimeZone),
    );
  }

  const sourceTimeZone = resolveSourceTimeZone(event.timeZoneToken);
  if (!sourceTimeZone || event.hour === null || event.minute === null) {
    return true;
  }

  const eventMatch = event.timestampMatch;
  if (!eventMatch) {
    return true;
  }
  const eventDate = inferEventDate(eventMatch, sourceTimeZone, now);
  if (!eventDate) {
    return true;
  }

  const localEvent = getNumericDateTimeParts(eventDate, options.targetTimeZone);
  const localToday = getNumericDateTimeParts(now, options.targetTimeZone);
  if (sameCalendarDate(localEvent, localToday)) {
    return true;
  }

  const tomorrow = addCalendarDays(localToday, 1);
  return sameCalendarDate(localEvent, tomorrow) && localEvent.hour <= 6;
}

function resolveEventChannelTimestamp(
  title: string,
  options: { referenceStartAt?: string | null; now?: Date },
) {
  const eventMatch = detectEventTimestamp(title);
  if (!eventMatch) {
    return null;
  }

  const referenceDate = parseReferenceDate(options.referenceStartAt);
  const explicitSourceTimeZone = eventMatch.timeZoneToken
    ? resolveSourceTimeZone(eventMatch.timeZoneToken)
    : null;

  const eventDate = explicitSourceTimeZone
    ? inferEventDate(eventMatch, explicitSourceTimeZone, options.now ?? new Date())
      ?? referenceDate
    : referenceDate;
  if (!eventDate) {
    return null;
  }

  return { eventMatch, eventDate };
}

function detectEventTimestamp(title: string): EventTimestampMatch | null {
  const patterns: Array<{
    kind: EventTimestampMatch["kind"];
    pattern: RegExp;
  }> = [
    { kind: "weekday-day-month", pattern: WEEKDAY_DAY_MONTH_EVENT_PATTERN },
    { kind: "weekday-day-month", pattern: WEEKDAY_MONTH_DAY_EVENT_PATTERN },
    { kind: "month-first", pattern: MONTH_FIRST_EVENT_PATTERN },
    { kind: "iso", pattern: ISO_EVENT_PATTERN },
    { kind: "day-month-year", pattern: DAY_MONTH_YEAR_EVENT_PATTERN },
  ];

  for (const { kind, pattern } of patterns) {
    const match = pattern.exec(title);
    if (!match?.groups) {
      continue;
    }

    const month = match.groups.numericMonth
      ? Number.parseInt(match.groups.numericMonth, 10)
      : monthTokenToNumber(match.groups.month);
    const day = Number.parseInt(match.groups.day, 10);
    const year = match.groups.year
      ? Number.parseInt(match.groups.year, 10)
      : null;
    const [hour, minute] = parseTimeToken(
      match.groups.time,
      match.groups.meridiem,
    );
    if (
      month
      && month >= 1
      && month <= 12
      && Number.isFinite(day)
      && hour !== null
      && minute !== null
    ) {
      return {
        kind,
        match,
        marker: match.groups.marker,
        separator: match.groups.separator,
        year,
        month,
        day,
        dayText: match.groups.day,
        hour,
        minute,
        meridiem: normalizeMeridiem(match.groups.meridiem),
        timeZoneToken:
          match.groups.timeZoneParen ?? match.groups.timeZone ?? null,
      };
    }
  }

  return null;
}

function detectEventDateParts(title: string): EventDateParts & {
  timestampMatch?: EventTimestampMatch;
} | null {
  const timestampMatch = detectEventTimestamp(title);
  if (timestampMatch) {
    return { ...timestampMatch, timestampMatch };
  }

  for (const pattern of [ISO_DATE_PATTERN, DAY_MONTH_YEAR_DATE_PATTERN]) {
    const match = pattern.exec(title);
    if (!match?.groups) {
      continue;
    }
    const year = Number.parseInt(match.groups.year, 10);
    const month = Number.parseInt(match.groups.numericMonth, 10);
    const day = Number.parseInt(match.groups.day, 10);
    if (isValidDateParts(year, month, day)) {
      return {
        year,
        month,
        day,
        hour: null,
        minute: null,
        timeZoneToken: null,
      };
    }
  }

  return null;
}

function monthTokenToNumber(token: string) {
  return MONTH_TOKEN_TO_NUMBER.get(token.trim().toLowerCase()) ?? null;
}

function parseTimeToken(
  token: string,
  meridiemToken?: string | null,
): [number | null, number | null] {
  const [hourToken, minuteToken] = token.split(":");
  let hour = Number.parseInt(hourToken ?? "", 10);
  const minute = Number.parseInt(minuteToken ?? "", 10);

  if (!Number.isFinite(hour) || !Number.isFinite(minute) || minute > 59) {
    return [null, null];
  }

  const meridiem = normalizeMeridiem(meridiemToken);
  if ((meridiem && (hour < 1 || hour > 12)) || (!meridiem && (hour < 0 || hour > 23))) {
    return [null, null];
  }
  if (meridiem === "AM") {
    hour = hour % 12;
  } else if (meridiem === "PM") {
    hour = hour % 12 + 12;
  }

  return [hour, minute];
}

function normalizeMeridiem(value?: string | null): "AM" | "PM" | null {
  const normalizedValue = value?.trim().toUpperCase();
  if (normalizedValue === "AM" || normalizedValue === "PM") {
    return normalizedValue;
  }

  return null;
}

function parseReferenceDate(value?: string | null) {
  if (!value) {
    return null;
  }

  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

function resolveSourceTimeZone(token: string): SourceTimeZone | null {
  const normalizedToken = token.trim().toUpperCase();
  const ianaTimeZone = IANA_TIME_ZONES_BY_TOKEN[normalizedToken];
  if (ianaTimeZone) {
    return { kind: "iana", timeZone: ianaTimeZone };
  }

  const fixedOffsetMinutes = FIXED_OFFSET_MINUTES_BY_TOKEN[normalizedToken];
  if (fixedOffsetMinutes !== undefined) {
    return { kind: "offset", offsetMinutes: fixedOffsetMinutes };
  }

  const offsetMatch = /^(?:UTC|GMT)?([+-])(\d{1,2})(?::?(\d{2}))?$/i.exec(normalizedToken);
  if (!offsetMatch) {
    return null;
  }

  const sign = offsetMatch[1] === "-" ? -1 : 1;
  const hours = Number.parseInt(offsetMatch[2] ?? "", 10);
  const minutes = Number.parseInt(offsetMatch[3] ?? "0", 10);
  if (!Number.isFinite(hours) || !Number.isFinite(minutes)) {
    return null;
  }

  return {
    kind: "offset",
    offsetMinutes: sign * (hours * 60 + minutes),
  };
}

function inferEventDate(
  eventMatch: EventTimestampMatch,
  sourceTimeZone: SourceTimeZone,
  now: Date,
) {
  const years = eventMatch.year === null
    ? [now.getUTCFullYear() - 1, now.getUTCFullYear(), now.getUTCFullYear() + 1]
    : [eventMatch.year];
  const candidates = years
    .map((year) => buildEventDateForYear(eventMatch, sourceTimeZone, year))
    .filter((candidate): candidate is Date => candidate !== null)
    .sort(
      (left, right) =>
        Math.abs(left.getTime() - now.getTime()) - Math.abs(right.getTime() - now.getTime()),
    );

  return candidates[0] ?? null;
}

function buildEventDateForYear(
  eventMatch: EventTimestampMatch,
  sourceTimeZone: SourceTimeZone,
  year: number,
) {
  const isValidCalendarDate = new Date(Date.UTC(year, eventMatch.month - 1, eventMatch.day))
    .toISOString()
    .startsWith(`${year.toString().padStart(4, "0")}-${eventMatch.month.toString().padStart(2, "0")}-${eventMatch.day.toString().padStart(2, "0")}`);
  if (!isValidCalendarDate) {
    return null;
  }

  if (sourceTimeZone.kind === "offset") {
    return new Date(
      Date.UTC(year, eventMatch.month - 1, eventMatch.day, eventMatch.hour, eventMatch.minute)
        - sourceTimeZone.offsetMinutes * 60 * 1000,
    );
  }

  return zonedDateTimeToUtc(
    {
      year,
      month: eventMatch.month,
      day: eventMatch.day,
      hour: eventMatch.hour,
      minute: eventMatch.minute,
    },
    sourceTimeZone.timeZone,
  );
}

function isValidDateParts(year: number, month: number, day: number) {
  if (month < 1 || month > 12 || day < 1 || day > 31) {
    return false;
  }
  const candidate = new Date(Date.UTC(year, month - 1, day));
  return candidate.getUTCFullYear() === year
    && candidate.getUTCMonth() === month - 1
    && candidate.getUTCDate() === day;
}

function inferClosestLocalYear(month: number, day: number, now: Date) {
  const candidates = [now.getFullYear() - 1, now.getFullYear(), now.getFullYear() + 1]
    .filter((year) => isValidDateParts(year, month, day));
  return candidates.sort((left, right) => {
    const leftDistance = Math.abs(new Date(left, month - 1, day).getTime() - now.getTime());
    const rightDistance = Math.abs(new Date(right, month - 1, day).getTime() - now.getTime());
    return leftDistance - rightDistance;
  })[0] ?? now.getFullYear();
}

function sameCalendarDate(
  left: Pick<NumericDateTimeParts, "year" | "month" | "day">,
  right: Pick<NumericDateTimeParts, "year" | "month" | "day">,
) {
  return left.year === right.year && left.month === right.month && left.day === right.day;
}

function addCalendarDays(parts: NumericDateTimeParts, days: number): NumericDateTimeParts {
  const date = new Date(Date.UTC(parts.year, parts.month - 1, parts.day + days));
  return {
    year: date.getUTCFullYear(),
    month: date.getUTCMonth() + 1,
    day: date.getUTCDate(),
    hour: 0,
    minute: 0,
  };
}

function zonedDateTimeToUtc(parts: NumericDateTimeParts, timeZone: string) {
  let utcTimestamp = Date.UTC(parts.year, parts.month - 1, parts.day, parts.hour, parts.minute);

  for (let iteration = 0; iteration < 3; iteration += 1) {
    const actual = getNumericDateTimeParts(new Date(utcTimestamp), timeZone);
    const desiredTimestamp = Date.UTC(
      parts.year,
      parts.month - 1,
      parts.day,
      parts.hour,
      parts.minute,
    );
    const actualTimestamp = Date.UTC(
      actual.year,
      actual.month - 1,
      actual.day,
      actual.hour,
      actual.minute,
    );
    const differenceMs = desiredTimestamp - actualTimestamp;
    utcTimestamp += differenceMs;

    if (differenceMs === 0) {
      break;
    }
  }

  return new Date(utcTimestamp);
}

function getNumericDateTimeParts(date: Date, timeZone?: string): NumericDateTimeParts {
  const formatter = new Intl.DateTimeFormat("en-US", {
    timeZone,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
  const parts = formatter.formatToParts(date);

  return {
    year: parsePartNumber(parts, "year"),
    month: parsePartNumber(parts, "month"),
    day: parsePartNumber(parts, "day"),
    hour: normalizeHour(parsePartNumber(parts, "hour")),
    minute: parsePartNumber(parts, "minute"),
  };
}

function getDisplayDateTimeParts(date: Date, timeZone?: string): DisplayDateTimeParts {
  const formatter = new Intl.DateTimeFormat("en-US", {
    timeZone,
    weekday: "short",
    month: "short",
    day: "2-digit",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
  const parts = formatter.formatToParts(date);

  return {
    year: parsePartNumber(parts, "year"),
    month: monthTokenToNumber(parsePartText(parts, "month")) ?? 1,
    monthShort: parsePartText(parts, "month"),
    day: parsePartNumber(parts, "day"),
    dayText: parsePartText(parts, "day"),
    hour: normalizeHour(parsePartNumber(parts, "hour")),
    hourText: parsePartText(parts, "hour").padStart(2, "0"),
    minute: parsePartNumber(parts, "minute"),
    minuteText: parsePartText(parts, "minute").padStart(2, "0"),
    weekday: parsePartText(parts, "weekday"),
  };
}

function parsePartText(parts: Intl.DateTimeFormatPart[], type: Intl.DateTimeFormatPartTypes) {
  const value = parts.find((part) => part.type === type)?.value;
  if (!value) {
    throw new Error(`Missing date-time part: ${type}`);
  }
  return value;
}

function parsePartNumber(parts: Intl.DateTimeFormatPart[], type: Intl.DateTimeFormatPartTypes) {
  return Number.parseInt(parsePartText(parts, type), 10);
}

function normalizeHour(hour: number) {
  return hour === 24 ? 0 : hour;
}

function formatEventTimestampReplacement(
  eventMatch: EventTimestampMatch,
  eventDate: Date,
  options: { includeTimeZoneLabel: boolean; targetTimeZone?: string },
) {
  const display = getDisplayDateTimeParts(eventDate, options.targetTimeZone);
  const displayDay =
    eventMatch.dayText.length >= 2
      ? display.dayText.padStart(2, "0")
      : display.day.toString();
  const displayTime = `${display.hourText}:${display.minuteText}`;
  const timeZoneSuffix = options.includeTimeZoneLabel
    ? ` ${formatTimeZoneLabel(eventDate, options.targetTimeZone)}`
    : "";

  if (eventMatch.kind === "month-first") {
    return `${eventMatch.marker ?? ""}${display.monthShort} ${displayDay} ${displayTime}${timeZoneSuffix}`;
  }

  if (eventMatch.kind === "iso") {
    return `${display.year.toString().padStart(4, "0")}-${display.month.toString().padStart(2, "0")}-${display.day.toString().padStart(2, "0")}${eventMatch.separator ?? " "}${displayTime}${timeZoneSuffix}`;
  }

  if (eventMatch.kind === "day-month-year") {
    return `${display.day.toString().padStart(2, "0")}-${display.month.toString().padStart(2, "0")}-${display.year.toString().padStart(4, "0")}${eventMatch.separator ?? " "}${displayTime}${timeZoneSuffix}`;
  }

  return `${display.weekday} ${displayDay} ${display.monthShort} ${displayTime}${timeZoneSuffix}`;
}

function formatTimeZoneLabel(date: Date, timeZone?: string) {
  try {
    const formatter = new Intl.DateTimeFormat("en-US", {
      timeZone,
      timeZoneName: "shortOffset",
    });
    const label = formatter
      .formatToParts(date)
      .find((part) => part.type === "timeZoneName")
      ?.value;
    if (label) {
      return label;
    }
  } catch {
    // Fallback below.
  }

  const formatter = new Intl.DateTimeFormat("en-US", {
    timeZone,
    timeZoneName: "short",
  });
  return (
    formatter
      .formatToParts(date)
      .find((part) => part.type === "timeZoneName")
      ?.value ?? "local"
  );
}
