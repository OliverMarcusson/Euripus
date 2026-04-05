import { create } from "zustand";
import { isTauriRuntime } from "@/lib/tauri";

const PLAYBACK_DEVICE_STORAGE_KEY = "euripus-playback-device";

type PersistedPlaybackDeviceState = {
  deviceKey: string;
  name: string;
  remoteTargetEnabled: boolean;
};

type PlaybackDeviceState = PersistedPlaybackDeviceState & {
  activeDeviceId: string | null;
  platform: string;
  formFactorHint: string | null;
  setName: (value: string) => void;
  setRemoteTargetEnabled: (value: boolean) => void;
  setActiveDeviceId: (value: string | null) => void;
};

function generateDeviceKey() {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }

  return `device-${Math.random().toString(36).slice(2, 10)}`;
}

function detectPlatform() {
  if (typeof window === "undefined") {
    return "web";
  }

  if (isTauriRuntime()) {
    return "tauri";
  }

  return window.matchMedia("(display-mode: standalone)").matches ? "pwa" : "web";
}

function detectFormFactorHint() {
  if (typeof window === "undefined") {
    return null;
  }

  const userAgent = window.navigator.userAgent.toLowerCase();
  if (/iphone|android.+mobile/.test(userAgent)) {
    return "phone";
  }

  if (/ipad|tablet/.test(userAgent)) {
    return "tablet";
  }

  if (window.innerWidth >= 960 || /android tv|googletv|google tv|bravia|shield|aft/.test(userAgent)) {
    return "large-screen";
  }

  return "desktop";
}

function defaultDeviceName() {
  if (typeof window === "undefined") {
    return "This device";
  }

  const userAgent = window.navigator.userAgent.toLowerCase();
  if (/android tv|googletv|google tv|bravia|shield|aft/.test(userAgent)) {
    return "Living room TV";
  }

  if (/iphone/.test(userAgent)) {
    return "iPhone";
  }

  if (/android/.test(userAgent)) {
    return "Android device";
  }

  if (isTauriRuntime()) {
    return "Desktop app";
  }

  return "Browser device";
}

function readPersistedState(): PersistedPlaybackDeviceState {
  if (typeof window === "undefined") {
    return {
      deviceKey: generateDeviceKey(),
      name: "This device",
      remoteTargetEnabled: false,
    };
  }

  const raw = window.localStorage.getItem(PLAYBACK_DEVICE_STORAGE_KEY);
  if (!raw) {
    return {
      deviceKey: generateDeviceKey(),
      name: defaultDeviceName(),
      remoteTargetEnabled: false,
    };
  }

  try {
    const parsed = JSON.parse(raw) as Partial<PersistedPlaybackDeviceState>;
    return {
      deviceKey: parsed.deviceKey || generateDeviceKey(),
      name: parsed.name || defaultDeviceName(),
      remoteTargetEnabled: parsed.remoteTargetEnabled ?? false,
    };
  } catch {
    return {
      deviceKey: generateDeviceKey(),
      name: defaultDeviceName(),
      remoteTargetEnabled: false,
    };
  }
}

function writePersistedState(state: PersistedPlaybackDeviceState) {
  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.setItem(PLAYBACK_DEVICE_STORAGE_KEY, JSON.stringify(state));
}

const initialPersistedState = readPersistedState();

export const usePlaybackDeviceStore = create<PlaybackDeviceState>((set, get) => ({
  ...initialPersistedState,
  activeDeviceId: null,
  platform: detectPlatform(),
  formFactorHint: detectFormFactorHint(),
  setName: (value) => {
    const name = value.trimStart();
    writePersistedState({
      deviceKey: get().deviceKey,
      name,
      remoteTargetEnabled: get().remoteTargetEnabled,
    });
    set({ name });
  },
  setRemoteTargetEnabled: (remoteTargetEnabled) => {
    writePersistedState({
      deviceKey: get().deviceKey,
      name: get().name,
      remoteTargetEnabled,
    });
    set({ remoteTargetEnabled });
  },
  setActiveDeviceId: (activeDeviceId) => set({ activeDeviceId }),
}));
