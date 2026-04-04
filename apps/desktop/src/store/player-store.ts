import type { PlaybackSource } from "@euripus/shared";
import { create } from "zustand";

type PlayerState = {
  source: PlaybackSource | null;
  loading: boolean;
  setLoading: (value: boolean) => void;
  setSource: (value: PlaybackSource | null) => void;
};

export const usePlayerStore = create<PlayerState>((set) => ({
  source: null,
  loading: false,
  setLoading: (loading) => set({ loading }),
  setSource: (source) => set({ source, loading: false }),
}));

