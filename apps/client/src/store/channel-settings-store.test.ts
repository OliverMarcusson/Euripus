import {
  PPV_DATE_FILTER_STORAGE_KEY,
  useChannelSettingsStore,
} from "@/store/channel-settings-store";

describe("channel settings store", () => {
  beforeEach(() => {
    window.localStorage.clear();
    useChannelSettingsStore.setState({ filterPpvByDate: false });
  });

  it("persists the PPV date filter", () => {
    useChannelSettingsStore.getState().setFilterPpvByDate(true);

    expect(useChannelSettingsStore.getState().filterPpvByDate).toBe(true);
    expect(window.localStorage.getItem(PPV_DATE_FILTER_STORAGE_KEY)).toBe("true");
  });
});
