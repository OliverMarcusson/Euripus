import type { AuthSession, User } from "@euripus/shared";
import { create } from "zustand";

type AuthState = {
  user: User | null;
  accessToken: string | null;
  expiresAt: string | null;
  hydrated: boolean;
  setSession: (session: AuthSession) => void;
  setHydrated: (value: boolean) => void;
  clearSession: () => void;
};

export const useAuthStore = create<AuthState>((set) => ({
  user: null,
  accessToken: null,
  expiresAt: null,
  hydrated: false,
  setSession: (session) =>
    set({
      user: session.user,
      accessToken: session.accessToken,
      expiresAt: session.expiresAt,
      hydrated: true,
    }),
  setHydrated: (value) => set({ hydrated: value }),
  clearSession: () =>
    set({
      user: null,
      accessToken: null,
      expiresAt: null,
      hydrated: true,
    }),
}));

