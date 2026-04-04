import type { Program } from "@euripus/shared";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

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
  }).format(new Date(value));
}

export function formatTimeRange(startAt: string, endAt: string) {
  return `${formatTime(startAt)} - ${formatTime(endAt)}`;
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
