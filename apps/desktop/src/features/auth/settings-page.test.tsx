import { fireEvent, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { SettingsPage } from "@/features/auth/settings-page";
import { getRecents, getSessions } from "@/lib/api";
import { useThemeStore } from "@/store/theme-store";

vi.mock("@/lib/api", () => ({
  getRecents: vi.fn(),
  getSessions: vi.fn(),
  revokeSession: vi.fn(),
}));

const mockedGetRecents = vi.mocked(getRecents);
const mockedGetSessions = vi.mocked(getSessions);

describe("SettingsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedGetRecents.mockResolvedValue([]);
    mockedGetSessions.mockResolvedValue([]);
    useThemeStore.getState().setPreference("system");
  });

  it("updates the theme when a toggle is selected", async () => {
    render(
      <QueryClientProvider client={new QueryClient()}>
        <SettingsPage />
      </QueryClientProvider>,
    );

    fireEvent.click(screen.getByRole("radio", { name: /light/i }));

    expect(useThemeStore.getState().preference).toBe("light");
    expect(document.documentElement.dataset.theme).toBe("light");
  });
});
