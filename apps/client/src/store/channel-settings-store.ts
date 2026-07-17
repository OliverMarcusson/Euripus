import { create } from "zustand";

const PPV_DATE_FILTER_STORAGE_KEY = "euripus-filter-ppv-by-date";
const QUALITY_CHANNELS_FILTER_STORAGE_KEY = "euripus-quality-channels-only";
const ADMIN_TOOLS_STORAGE_KEY = "euripus-admin-tools-enabled";

function readStoredPpvDateFilter() {
  if (typeof window === "undefined") {
    return false;
  }
  return window.localStorage?.getItem(PPV_DATE_FILTER_STORAGE_KEY) === "true";
}

type ChannelSettingsState = {
  filterPpvByDate: boolean;
  qualityChannelsOnly: boolean;
  adminToolsEnabled: boolean;
  setFilterPpvByDate: (enabled: boolean) => void;
  setQualityChannelsOnly: (enabled: boolean) => void;
  setAdminToolsEnabled: (enabled: boolean) => void;
};

export const useChannelSettingsStore = create<ChannelSettingsState>((set) => ({
  filterPpvByDate: readStoredPpvDateFilter(),
  qualityChannelsOnly: typeof window !== "undefined"
    && window.localStorage?.getItem(QUALITY_CHANNELS_FILTER_STORAGE_KEY) === "true",
  adminToolsEnabled: typeof window !== "undefined"
    && window.localStorage?.getItem(ADMIN_TOOLS_STORAGE_KEY) === "true",
  setFilterPpvByDate: (filterPpvByDate) => {
    if (typeof window !== "undefined") {
      window.localStorage?.setItem(
        PPV_DATE_FILTER_STORAGE_KEY,
        filterPpvByDate.toString(),
      );
    }
    set({ filterPpvByDate });
  },
  setQualityChannelsOnly: (qualityChannelsOnly) => {
    if (typeof window !== "undefined") {
      window.localStorage?.setItem(QUALITY_CHANNELS_FILTER_STORAGE_KEY, qualityChannelsOnly.toString());
    }
    set({ qualityChannelsOnly });
  },
  setAdminToolsEnabled: (adminToolsEnabled) => {
    if (typeof window !== "undefined") {
      window.localStorage?.setItem(ADMIN_TOOLS_STORAGE_KEY, adminToolsEnabled.toString());
    }
    set({ adminToolsEnabled });
  },
}));

export { ADMIN_TOOLS_STORAGE_KEY, PPV_DATE_FILTER_STORAGE_KEY, QUALITY_CHANNELS_FILTER_STORAGE_KEY };
