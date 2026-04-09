import type { ReceiverDevice, ReceiverPlaybackState } from "@euripus/shared";
import { formatRelativeTime } from "@/lib/utils";

export function formatReceiverPlaybackSummary(
  device: Pick<ReceiverDevice, "currentPlayback" | "online" | "platform" | "lastSeenAt" | "playbackStateStale">,
) {
  const playback = device.currentPlayback;
  if (playback) {
    if (playback.errorMessage) {
      return `Playback issue: ${playback.errorMessage}`;
    }
    if (playback.buffering) {
      return `Buffering ${playback.title}`;
    }
    if (playback.paused) {
      return `Paused ${playback.title}`;
    }
    return `Now playing ${playback.title}`;
  }
  if (device.playbackStateStale) {
    return `Playback state expired ${formatRelativeTime(device.lastSeenAt)}`;
  }
  return device.online ? device.platform : `Last seen ${formatRelativeTime(device.lastSeenAt)}`;
}

export function receiverPlaybackBadgeLabel(playback: ReceiverPlaybackState) {
  if (playback.errorMessage) {
    return "Error";
  }
  if (playback.buffering) {
    return "Buffering";
  }
  if (playback.paused) {
    return playback.live ? "Paused Live" : "Paused";
  }
  return playback.live ? "Live" : "Archive";
}
