import type { RemoteControllerTarget } from "@euripus/shared";
import { create } from "zustand";

export type RemoteControllerTargetSelection = {
  id: string;
  name: string;
};

type RemoteControllerState = {
  target: RemoteControllerTargetSelection | null;
  selectedAt: string | null;
  setTargetSelection: (selection: RemoteControllerTarget) => void;
  clearTarget: () => void;
};

export const useRemoteControllerStore = create<RemoteControllerState>((set) => ({
  target: null,
  selectedAt: null,
  setTargetSelection: (selection) =>
    set((current) => {
      const nextTarget = selection?.device
        ? {
            id: selection.device.id,
            name: selection.device.name,
          }
        : null;
      const nextSelectedAt = selection?.selectedAt ?? null;

      if (
        current.selectedAt === nextSelectedAt &&
        current.target?.id === nextTarget?.id &&
        current.target?.name === nextTarget?.name
      ) {
        return current;
      }

      return {
        target: nextTarget,
        selectedAt: nextSelectedAt,
      };
    }),
  clearTarget: () =>
    set((current) => {
      if (!current.target && !current.selectedAt) {
        return current;
      }

      return {
        target: null,
        selectedAt: null,
      };
    }),
}));
