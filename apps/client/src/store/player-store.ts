import type { PlaybackSource } from "@euripus/shared";
import { create } from "zustand";

export type PlaybackRequest =
  | { kind: "channel"; id: string }
  | { kind: "program"; id: string };

type PlayerState = {
  currentRequest: PlaybackRequest | null;
  source: PlaybackSource | null;
  loading: boolean;
  setLoading: (value: boolean) => void;
  setPlayback: (source: PlaybackSource, request: PlaybackRequest) => void;
  setSource: (value: PlaybackSource | null) => void;
};

export const usePlayerStore = create<PlayerState>((set) => ({
  currentRequest: null,
  source: null,
  loading: false,
  setLoading: (loading) => set({ loading }),
  setPlayback: (source, currentRequest) => set({ source, currentRequest, loading: false }),
  setSource: (source) =>
    set((state) => ({
      source,
      currentRequest: source ? state.currentRequest : null,
      loading: false,
    })),
}));
