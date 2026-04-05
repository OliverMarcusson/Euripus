import type { PlaybackSource, RemotePlaybackCommand } from "@euripus/shared";

export type RemoteDeviceEventPayload = {
  eventType: string;
  command: RemotePlaybackCommand;
  source: PlaybackSource;
};
