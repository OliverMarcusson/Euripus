import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { PlaybackSource, RemotePlaybackCommand } from "@euripus/shared";
import {
  startChannelPlayback,
  startEpisodePlayback,
  startOnDemandPlayback,
  startProgramPlayback,
  startRemoteChannelPlayback,
  startRemoteEpisodePlayback,
  startRemoteOnDemandPlayback,
  startRemoteProgramPlayback,
  seekRemotePlayback,
  updateOnDemandProgress,
} from "@/lib/api";
import { castPlaybackRequest } from "@/lib/cast-playback";
import { useGoogleCastStore } from "@/lib/google-cast";
import { usePlayerStore } from "@/store/player-store";
import { useRemoteControllerStore } from "@/store/remote-controller-store";

type CastPlaybackResult = { cast: true };
type PlaybackResult = PlaybackSource | RemotePlaybackCommand | CastPlaybackResult;

function isRemotePlaybackCommand(value: PlaybackResult): value is RemotePlaybackCommand {
  return "targetDeviceId" in value;
}

function isCastPlaybackResult(value: PlaybackResult): value is CastPlaybackResult {
  return "cast" in value;
}

export function useChannelPlaybackMutation() {
  const queryClient = useQueryClient();
  const target = useRemoteControllerStore((state) => state.target);
  const castConnected = useGoogleCastStore((state) => state.connected);
  const setLoading = usePlayerStore((state) => state.setLoading);
  const setPlayback = usePlayerStore((state) => state.setPlayback);
  const setSource = usePlayerStore((state) => state.setSource);

  return useMutation<PlaybackResult, Error, string>({
    mutationFn: async (channelId: string) => {
      if (castConnected) {
        await castPlaybackRequest({ kind: "channel", id: channelId });
        return { cast: true };
      }
      return target ? startRemoteChannelPlayback(channelId) : startChannelPlayback(channelId);
    },
    onMutate: () => {
      if (!target && !castConnected) {
        setLoading(true);
      }
    },
    onSuccess: (result, channelId) => {
      if (isCastPlaybackResult(result)) {
        setSource(null);
        return;
      }
      if (isRemotePlaybackCommand(result)) {
        void queryClient.invalidateQueries({ queryKey: ["remote"] });
        return;
      }

      setPlayback(result, { kind: "channel", id: channelId });
    },
    onSettled: () => {
      if (!target && !castConnected) {
        setLoading(false);
      }
    },
  });
}

export type OnDemandPlaybackSelection = {
  id: string;
  startAtSeconds?: number;
  resetProgress?: boolean;
};

export function useOnDemandPlaybackMutation(kind: "onDemand" | "episode") {
  const queryClient = useQueryClient();
  const target = useRemoteControllerStore((state) => state.target);
  const castConnected = useGoogleCastStore((state) => state.connected);
  const setLoading = usePlayerStore((state) => state.setLoading);
  const setPlayback = usePlayerStore((state) => state.setPlayback);
  const setSource = usePlayerStore((state) => state.setSource);

  return useMutation<PlaybackResult, Error, OnDemandPlaybackSelection>({
    mutationFn: async ({ id, startAtSeconds, resetProgress }) => {
      if (resetProgress) {
        await updateOnDemandProgress(kind === "episode" ? "episode" : "movie", id, {
          positionSeconds: 0,
          durationSeconds: null,
        });
      }
      if (castConnected) {
        await castPlaybackRequest({ kind, id, startAtSeconds });
        return { cast: true };
      }
      if (target) {
        const result = kind === "episode" ? await startRemoteEpisodePlayback(id) : await startRemoteOnDemandPlayback(id);
        if ((startAtSeconds ?? 0) > 0) await seekRemotePlayback(startAtSeconds!);
        return result;
      }
      return kind === "episode" ? startEpisodePlayback(id) : startOnDemandPlayback(id);
    },
    onMutate: () => { if (!target && !castConnected) setLoading(true); },
    onSuccess: (result, { id, startAtSeconds }) => {
      if (isCastPlaybackResult(result)) {
        setSource(null);
      } else if (isRemotePlaybackCommand(result)) {
        void queryClient.invalidateQueries({ queryKey: ["remote"] });
      } else {
        setPlayback(result, { kind, id, startAtSeconds });
      }
    },
    onSettled: () => { if (!target && !castConnected) setLoading(false); },
  });
}

export function useProgramPlaybackMutation() {
  const queryClient = useQueryClient();
  const target = useRemoteControllerStore((state) => state.target);
  const castConnected = useGoogleCastStore((state) => state.connected);
  const setLoading = usePlayerStore((state) => state.setLoading);
  const setPlayback = usePlayerStore((state) => state.setPlayback);
  const setSource = usePlayerStore((state) => state.setSource);

  return useMutation<PlaybackResult, Error, string>({
    mutationFn: async (programId: string) => {
      if (castConnected) {
        await castPlaybackRequest({ kind: "program", id: programId });
        return { cast: true };
      }
      return target ? startRemoteProgramPlayback(programId) : startProgramPlayback(programId);
    },
    onMutate: () => {
      if (!target && !castConnected) {
        setLoading(true);
      }
    },
    onSuccess: (result, programId) => {
      if (isCastPlaybackResult(result)) {
        setSource(null);
        return;
      }
      if (isRemotePlaybackCommand(result)) {
        void queryClient.invalidateQueries({ queryKey: ["remote"] });
        return;
      }

      setPlayback(result, { kind: "program", id: programId });
    },
    onSettled: () => {
      if (!target && !castConnected) {
        setLoading(false);
      }
    },
  });
}
