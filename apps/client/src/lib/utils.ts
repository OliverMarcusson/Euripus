import type { Program } from "@euripus/shared";
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
  "(?:ET|EST|EDT|CT|CST|CDT|MT|MST|MDT|PT|PST|PDT|CET|CEST|EET|EEST|BST|GMT|UTC|(?:UTC|GMT)[+-]\\d{1,2}(?::?\\d{2})?)";
const TIME_OF_DAY_TOKEN_PATTERN =
  String.raw`(?<time>\d{1,2}:\d{2})(?:\s*(?<meridiem>AM|PM))?`;
const MONTH_FIRST_EVENT_PATTERN = new RegExp(
  String.raw`(?<marker>@\s*)?(?<month>${MONTH_TOKEN_PATTERN})\s+(?<day>\d{1,2})\s+${TIME_OF_DAY_TOKEN_PATTERN}(?:\s+(?<timeZone>${TIME_ZONE_TOKEN_PATTERN}))?`,
  "i",
);
const WEEKDAY_DAY_MONTH_EVENT_PATTERN = new RegExp(
  String.raw`(?<weekday>Mon|Tue|Wed|Thu|Fri|Sat|Sun)\s+(?<day>\d{1,2})\s+(?<month>${MONTH_TOKEN_PATTERN})\s+${TIME_OF_DAY_TOKEN_PATTERN}(?:\s+(?<timeZone>${TIME_ZONE_TOKEN_PATTERN}))?`,
  "i",
);

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
  kind: "month-first" | "weekday-day-month";
  match: RegExpExecArray;
  marker?: string;
  month: number;
  day: number;
  dayText: string;
  hour: number;
  minute: number;
  meridiem: "AM" | "PM" | null;
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
  const monthFirstMatch = MONTH_FIRST_EVENT_PATTERN.exec(title);
  if (monthFirstMatch?.groups) {
    const month = monthTokenToNumber(monthFirstMatch.groups.month);
    const day = Number.parseInt(monthFirstMatch.groups.day, 10);
    const [hour, minute] = parseTimeToken(
      monthFirstMatch.groups.time,
      monthFirstMatch.groups.meridiem,
    );
    if (month && Number.isFinite(day) && hour !== null && minute !== null) {
      return {
        kind: "month-first",
        match: monthFirstMatch,
        marker: monthFirstMatch.groups.marker,
        month,
        day,
        dayText: monthFirstMatch.groups.day,
        hour,
        minute,
        meridiem: normalizeMeridiem(monthFirstMatch.groups.meridiem),
        timeZoneToken: monthFirstMatch.groups.timeZone ?? null,
      };
    }
  }

  const weekdayDayMonthMatch = WEEKDAY_DAY_MONTH_EVENT_PATTERN.exec(title);
  if (weekdayDayMonthMatch?.groups) {
    const month = monthTokenToNumber(weekdayDayMonthMatch.groups.month);
    const day = Number.parseInt(weekdayDayMonthMatch.groups.day, 10);
    const [hour, minute] = parseTimeToken(
      weekdayDayMonthMatch.groups.time,
      weekdayDayMonthMatch.groups.meridiem,
    );
    if (month && Number.isFinite(day) && hour !== null && minute !== null) {
      return {
        kind: "weekday-day-month",
        match: weekdayDayMonthMatch,
        month,
        day,
        dayText: weekdayDayMonthMatch.groups.day,
        hour,
        minute,
        meridiem: normalizeMeridiem(weekdayDayMonthMatch.groups.meridiem),
        timeZoneToken: weekdayDayMonthMatch.groups.timeZone ?? null,
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

  if (!Number.isFinite(hour) || !Number.isFinite(minute)) {
    return [null, null];
  }

  const meridiem = normalizeMeridiem(meridiemToken);
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

  const offsetMatch = /^(?:UTC|GMT)([+-])(\d{1,2})(?::?(\d{2}))?$/i.exec(normalizedToken);
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
  const years = [now.getUTCFullYear() - 1, now.getUTCFullYear(), now.getUTCFullYear() + 1];
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
