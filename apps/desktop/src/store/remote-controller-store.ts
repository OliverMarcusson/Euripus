import type { PlaybackDevice, RemoteControllerTarget } from "@euripus/shared";
import { create } from "zustand";

type RemoteControllerState = {
  target: PlaybackDevice | null;
  selectedAt: string | null;
  setTargetSelection: (selection: RemoteControllerTarget) => void;
  clearTarget: () => void;
};

export const useRemoteControllerStore = create<RemoteControllerState>((set) => ({
  target: null,
  selectedAt: null,
  setTargetSelection: (selection) =>
    set({
      target: selection?.device ?? null,
      selectedAt: selection?.selectedAt ?? null,
    }),
  clearTarget: () =>
    set({
      target: null,
      selectedAt: null,
    }),
}));
