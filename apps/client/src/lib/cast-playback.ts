import type { PlaybackSource } from "@euripus/shared";
import {
  startChannelPlayback,
  startEpisodePlayback,
  startOnDemandPlayback,
  startProgramPlayback,
} from "@/lib/api";
import { loadGoogleCastMedia } from "@/lib/google-cast";
import type { PlaybackRequest } from "@/store/player-store";

export function resolveCastPlaybackSource(request: PlaybackRequest) {
  switch (request.kind) {
    case "channel":
      return startChannelPlayback(request.id, "cast");
    case "program":
      return startProgramPlayback(request.id, "cast");
    case "episode":
      return startEpisodePlayback(request.id, "cast");
    case "onDemand":
      return startOnDemandPlayback(request.id, "cast");
  }
}

export async function castPlaybackRequest(request: PlaybackRequest) {
  const source: PlaybackSource = await resolveCastPlaybackSource(request);
  await loadGoogleCastMedia(source);
  return source;
}
