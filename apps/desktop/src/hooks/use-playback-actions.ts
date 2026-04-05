import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { PlaybackSource, RemotePlaybackCommand } from "@euripus/shared";
import {
  startChannelPlayback,
  startProgramPlayback,
  startRemoteChannelPlayback,
  startRemoteProgramPlayback,
} from "@/lib/api";
import { usePlayerStore } from "@/store/player-store";
import { useRemoteControllerStore } from "@/store/remote-controller-store";

function isRemotePlaybackCommand(value: PlaybackSource | RemotePlaybackCommand): value is RemotePlaybackCommand {
  return "targetDeviceId" in value;
}

export function useChannelPlaybackMutation() {
  const queryClient = useQueryClient();
  const target = useRemoteControllerStore((state) => state.target);
  const setLoading = usePlayerStore((state) => state.setLoading);
  const setSource = usePlayerStore((state) => state.setSource);

  return useMutation<PlaybackSource | RemotePlaybackCommand, Error, string>({
    mutationFn: (channelId: string) =>
      target ? startRemoteChannelPlayback(channelId) : startChannelPlayback(channelId),
    onMutate: () => {
      if (!target) {
        setLoading(true);
      }
    },
    onSuccess: (result) => {
      if (isRemotePlaybackCommand(result)) {
        void queryClient.invalidateQueries({ queryKey: ["remote"] });
        return;
      }

      setSource(result);
    },
    onSettled: () => {
      if (!target) {
        setLoading(false);
      }
    },
  });
}

export function useProgramPlaybackMutation() {
  const queryClient = useQueryClient();
  const target = useRemoteControllerStore((state) => state.target);
  const setLoading = usePlayerStore((state) => state.setLoading);
  const setSource = usePlayerStore((state) => state.setSource);

  return useMutation<PlaybackSource | RemotePlaybackCommand, Error, string>({
    mutationFn: (programId: string) =>
      target ? startRemoteProgramPlayback(programId) : startProgramPlayback(programId),
    onMutate: () => {
      if (!target) {
        setLoading(true);
      }
    },
    onSuccess: (result) => {
      if (isRemotePlaybackCommand(result)) {
        void queryClient.invalidateQueries({ queryKey: ["remote"] });
        return;
      }

      setSource(result);
    },
    onSettled: () => {
      if (!target) {
        setLoading(false);
      }
    },
  });
}
