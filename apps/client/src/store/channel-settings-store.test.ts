import {
  ADMIN_TOOLS_STORAGE_KEY,
  PPV_DATE_FILTER_STORAGE_KEY,
  useChannelSettingsStore,
} from "@/store/channel-settings-store";

describe("channel settings store", () => {
  beforeEach(() => {
    window.localStorage.clear();
    useChannelSettingsStore.setState({ filterPpvByDate: false, adminToolsEnabled: false });
  });

  it("persists the PPV date filter", () => {
    useChannelSettingsStore.getState().setFilterPpvByDate(true);

    expect(useChannelSettingsStore.getState().filterPpvByDate).toBe(true);
    expect(window.localStorage.getItem(PPV_DATE_FILTER_STORAGE_KEY)).toBe("true");
  });

  it("persists the admin tools preference", () => {
    useChannelSettingsStore.getState().setAdminToolsEnabled(true);

    expect(useChannelSettingsStore.getState().adminToolsEnabled).toBe(true);
    expect(window.localStorage.getItem(ADMIN_TOOLS_STORAGE_KEY)).toBe("true");
  });
});
