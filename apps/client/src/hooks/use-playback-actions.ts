import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { PlaybackSource, RemotePlaybackCommand } from "@euripus/shared";
import {
  seekRemotePlayback,
  startChannelPlayback,
  startEpisodePlayback,
  startOnDemandPlayback,
  startProgramPlayback,
  startRemoteChannelPlayback,
  startRemoteEpisodePlayback,
  startRemoteOnDemandPlayback,
  startRemoteProgramPlayback,
  updateOnDemandProgress,
} from "@/lib/api";
import { usePlayerStore } from "@/store/player-store";
import { useRemoteControllerStore } from "@/store/remote-controller-store";

type PlaybackResult = PlaybackSource | RemotePlaybackCommand;

function isRemotePlaybackCommand(value: PlaybackResult): value is RemotePlaybackCommand {
  return "targetDeviceId" in value;
}

export function useChannelPlaybackMutation() {
  const queryClient = useQueryClient();
  const target = useRemoteControllerStore((state) => state.target);
  const setLoading = usePlayerStore((state) => state.setLoading);
  const setPlayback = usePlayerStore((state) => state.setPlayback);

  return useMutation<PlaybackResult, Error, string>({
    mutationFn: (channelId: string) =>
      target ? startRemoteChannelPlayback(channelId) : startChannelPlayback(channelId),
    onMutate: () => {
      if (!target) setLoading(true);
    },
    onSuccess: (result, channelId) => {
      if (isRemotePlaybackCommand(result)) {
        void queryClient.invalidateQueries({ queryKey: ["remote"] });
        return;
      }
      setPlayback(result, { kind: "channel", id: channelId });
    },
    onSettled: () => {
      if (!target) setLoading(false);
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
  const setLoading = usePlayerStore((state) => state.setLoading);
  const setPlayback = usePlayerStore((state) => state.setPlayback);

  return useMutation<PlaybackResult, Error, OnDemandPlaybackSelection>({
    mutationFn: async ({ id, startAtSeconds, resetProgress }) => {
      if (resetProgress) {
        await updateOnDemandProgress(kind === "episode" ? "episode" : "movie", id, {
          positionSeconds: 0,
          durationSeconds: null,
        });
      }
      if (target) {
        const result = kind === "episode"
          ? await startRemoteEpisodePlayback(id)
          : await startRemoteOnDemandPlayback(id);
        if ((startAtSeconds ?? 0) > 0) {
          await seekRemotePlayback(startAtSeconds!);
        }
        return result;
      }
      return kind === "episode" ? startEpisodePlayback(id) : startOnDemandPlayback(id);
    },
    onMutate: () => {
      if (!target) setLoading(true);
    },
    onSuccess: (result, { id, startAtSeconds }) => {
      if (isRemotePlaybackCommand(result)) {
        void queryClient.invalidateQueries({ queryKey: ["remote"] });
      } else {
        setPlayback(result, { kind, id, startAtSeconds });
      }
    },
    onSettled: () => {
      if (!target) setLoading(false);
    },
  });
}

export function useProgramPlaybackMutation() {
  const queryClient = useQueryClient();
  const target = useRemoteControllerStore((state) => state.target);
  const setLoading = usePlayerStore((state) => state.setLoading);
  const setPlayback = usePlayerStore((state) => state.setPlayback);

  return useMutation<PlaybackResult, Error, string>({
    mutationFn: (programId: string) =>
      target ? startRemoteProgramPlayback(programId) : startProgramPlayback(programId),
    onMutate: () => {
      if (!target) setLoading(true);
    },
    onSuccess: (result, programId) => {
      if (isRemotePlaybackCommand(result)) {
        void queryClient.invalidateQueries({ queryKey: ["remote"] });
        return;
      }
      setPlayback(result, { kind: "program", id: programId });
    },
    onSettled: () => {
      if (!target) setLoading(false);
    },
  });
}
