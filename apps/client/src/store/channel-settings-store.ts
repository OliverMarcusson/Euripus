import { create } from "zustand";

const PPV_DATE_FILTER_STORAGE_KEY = "euripus-filter-ppv-by-date";

function readStoredPpvDateFilter() {
  if (typeof window === "undefined") {
    return false;
  }
  return window.localStorage?.getItem(PPV_DATE_FILTER_STORAGE_KEY) === "true";
}

type ChannelSettingsState = {
  filterPpvByDate: boolean;
  setFilterPpvByDate: (enabled: boolean) => void;
};

export const useChannelSettingsStore = create<ChannelSettingsState>((set) => ({
  filterPpvByDate: readStoredPpvDateFilter(),
  setFilterPpvByDate: (filterPpvByDate) => {
    if (typeof window !== "undefined") {
      window.localStorage?.setItem(
        PPV_DATE_FILTER_STORAGE_KEY,
        filterPpvByDate.toString(),
      );
    }
    set({ filterPpvByDate });
  },
}));

export { PPV_DATE_FILTER_STORAGE_KEY };
