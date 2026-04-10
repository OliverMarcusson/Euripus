import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AdminPage } from "@/features/admin/admin-page";
import {
  adminLogin,
  adminLogout,
  createAdminPatternGroup,
  deleteAllAdminPatternGroups,
  deleteAdminPatternGroup,
  getAdminPatternGroups,
  importAdminPatternGroups,
  testAdminSearchPatterns,
  testAdminSearchQuery,
  updateAdminPatternGroup,
} from "@/lib/api";

vi.mock("@/lib/api", () => ({
  adminLogin: vi.fn(),
  adminLogout: vi.fn(),
  createAdminPatternGroup: vi.fn(),
  deleteAllAdminPatternGroups: vi.fn(),
  deleteAdminPatternGroup: vi.fn(),
  getAdminPatternGroups: vi.fn(),
  importAdminPatternGroups: vi.fn(),
  testAdminSearchPatterns: vi.fn(),
  testAdminSearchQuery: vi.fn(),
  updateAdminPatternGroup: vi.fn(),
  getAdminImportErrors: vi.fn(() => []),
}));

const mockedGetAdminPatternGroups = vi.mocked(getAdminPatternGroups);
const mockedImportAdminPatternGroups = vi.mocked(importAdminPatternGroups);
const mockedAdminLogin = vi.mocked(adminLogin);
const mockedAdminLogout = vi.mocked(adminLogout);
const mockedCreateAdminPatternGroup = vi.mocked(createAdminPatternGroup);
const mockedDeleteAllAdminPatternGroups = vi.mocked(deleteAllAdminPatternGroups);
const mockedDeleteAdminPatternGroup = vi.mocked(deleteAdminPatternGroup);
const mockedTestAdminSearchPatterns = vi.mocked(testAdminSearchPatterns);
const mockedTestAdminSearchQuery = vi.mocked(testAdminSearchQuery);
const mockedUpdateAdminPatternGroup = vi.mocked(updateAdminPatternGroup);

describe("AdminPage JSON import", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedGetAdminPatternGroups.mockResolvedValue([]);
    mockedImportAdminPatternGroups.mockResolvedValue([]);
    mockedAdminLogin.mockResolvedValue();
    mockedAdminLogout.mockResolvedValue();
    mockedCreateAdminPatternGroup.mockResolvedValue({
      id: "group-1",
      kind: "country",
      value: "se",
      normalizedValue: "se",
      matchTarget: "channel_or_category",
      matchMode: "prefix",
      priority: 10,
      enabled: true,
      patternsText: "SE:",
      countryCodesText: "",
      countryCodes: [],
      patterns: [{ id: "pattern-1", pattern: "SE:" }],
    });
    mockedDeleteAllAdminPatternGroups.mockResolvedValue();
    mockedDeleteAdminPatternGroup.mockResolvedValue();
    mockedUpdateAdminPatternGroup.mockResolvedValue({
      id: "group-1",
      kind: "country",
      value: "se",
      normalizedValue: "se",
      matchTarget: "channel_or_category",
      matchMode: "prefix",
      priority: 10,
      enabled: true,
      patternsText: "SE:",
      countryCodesText: "",
      countryCodes: [],
      patterns: [{ id: "pattern-1", pattern: "SE:" }],
    });
    mockedTestAdminSearchPatterns.mockResolvedValue({
      countryCode: null,
      providerName: null,
      isPpv: false,
      isVip: false,
      forceHasEpg: false,
    });
    mockedTestAdminSearchQuery.mockResolvedValue({
      search: "",
      countries: [],
      providers: [],
      ppv: null,
      vip: null,
      requireEpg: false,
    });
  });

  function renderAdminPage() {
    const queryClient = new QueryClient({
      defaultOptions: {
        queries: {
          retry: false,
        },
      },
    });

    return render(
      <QueryClientProvider client={queryClient}>
        <AdminPage />
      </QueryClientProvider>,
    );
  }

  it("opens the Add JSON modal", async () => {
    renderAdminPage();

    fireEvent.click(await screen.findByRole("button", { name: /add json/i }));

    expect(
      screen.getByRole("heading", { name: /import pattern groups from json/i }),
    ).toBeInTheDocument();
  });

  it("shows a client-side parse error for invalid JSON", async () => {
    renderAdminPage();
    fireEvent.click(await screen.findByRole("button", { name: /add json/i }));

    fireEvent.change(screen.getByLabelText(/json input/i), {
      target: { value: "{bad json" },
    });
    fireEvent.click(screen.getByRole("button", { name: /import rules/i }));

    expect(await screen.findByText(/expected property name/i)).toBeInTheDocument();
    expect(mockedImportAdminPatternGroups).not.toHaveBeenCalled();
  });

  it("submits valid JSON imports as a batch", async () => {
    renderAdminPage();
    fireEvent.click(await screen.findByRole("button", { name: /add json/i }));

    fireEvent.change(screen.getByLabelText(/json input/i), {
      target: {
        value: JSON.stringify([
          {
            kind: "provider",
            value: "viaplay",
            matchTarget: "channel_or_category",
            matchMode: "contains",
            patterns: ["VIAPLAY"],
            countryCodes: ["se", "uk"],
          },
        ]),
      },
    });
    fireEvent.click(screen.getByRole("button", { name: /import rules/i }));

    await waitFor(() => expect(mockedImportAdminPatternGroups).toHaveBeenCalled());
    expect(mockedImportAdminPatternGroups.mock.calls[0]?.[0]).toEqual({
      groups: [
        {
          kind: "provider",
          value: "viaplay",
          matchTarget: "channel_or_category",
          matchMode: "contains",
          patterns: ["VIAPLAY"],
          countryCodes: ["se", "uk"],
        },
      ],
    });
  });

  it("shows and submits related countries for provider rules", async () => {
    renderAdminPage();

    fireEvent.change((await screen.findAllByRole("combobox"))[0], {
      target: { value: "provider" },
    });
    const [valueInput, patternsInput, countryCodesInput] = screen.getAllByRole("textbox");
    fireEvent.change(valueInput, {
      target: { value: "viaplay" },
    });
    fireEvent.change(patternsInput, {
      target: { value: "VIAPLAY" },
    });
    fireEvent.change(countryCodesInput, {
      target: { value: "se,uk" },
    });

    fireEvent.click(screen.getByRole("button", { name: /create group/i }));

    await waitFor(() => expect(mockedCreateAdminPatternGroup).toHaveBeenCalled());
    expect(mockedCreateAdminPatternGroup.mock.calls[0]?.[0]).toMatchObject({
      kind: "provider",
      value: "viaplay",
      patternsText: "VIAPLAY",
      countryCodesText: "se,uk",
    });
  });

  it("deletes all rules after confirmation", async () => {
    mockedGetAdminPatternGroups.mockResolvedValue([
      {
        id: "group-1",
        kind: "country",
        value: "se",
        normalizedValue: "se",
        matchTarget: "channel_or_category",
        matchMode: "prefix",
        priority: 10,
        enabled: true,
        patternsText: "SE:",
        countryCodesText: "",
        countryCodes: [],
        patterns: [{ id: "pattern-1", pattern: "SE:" }],
      },
    ]);
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);

    renderAdminPage();

    fireEvent.click(await screen.findByRole("button", { name: /delete all/i }));

    await waitFor(() => expect(mockedDeleteAllAdminPatternGroups).toHaveBeenCalled());
    expect(confirmSpy).toHaveBeenCalled();

    confirmSpy.mockRestore();
  });

  it("renders rule sections collapsed by default and expands a section on click", async () => {
    mockedGetAdminPatternGroups.mockResolvedValue([
      {
        id: "group-1",
        kind: "provider",
        value: "viaplay",
        normalizedValue: "viaplay",
        matchTarget: "channel_or_category",
        matchMode: "contains",
        priority: 10,
        enabled: true,
        patternsText: "VIAPLAY",
        countryCodesText: "se",
        countryCodes: ["se"],
        patterns: [{ id: "pattern-1", pattern: "VIAPLAY" }],
      },
    ]);

    renderAdminPage();

    const providersToggle = await screen.findByRole("button", { name: /expand providers/i });
    expect(providersToggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("button", { name: /expand viaplay/i })).not.toBeInTheDocument();

    fireEvent.click(providersToggle);

    expect(await screen.findByRole("button", { name: /collapse providers/i })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(await screen.findByRole("button", { name: /expand viaplay/i })).toBeInTheDocument();
  });

  it("renders saved rule cards collapsed by default and expands on click", async () => {
    mockedGetAdminPatternGroups.mockResolvedValue([
      {
        id: "group-1",
        kind: "provider",
        value: "viaplay",
        normalizedValue: "viaplay",
        matchTarget: "channel_or_category",
        matchMode: "contains",
        priority: 10,
        enabled: true,
        patternsText: "VIAPLAY,Viaplay SE",
        countryCodesText: "se,uk",
        countryCodes: ["se", "uk"],
        patterns: [
          { id: "pattern-1", pattern: "VIAPLAY" },
          { id: "pattern-2", pattern: "Viaplay SE" },
        ],
      },
    ]);

    renderAdminPage();

    fireEvent.click(await screen.findByRole("button", { name: /expand providers/i }));

    const toggle = await screen.findByRole("button", { name: /expand viaplay/i });
    expect(toggle).toHaveAttribute("aria-expanded", "false");

    fireEvent.click(toggle);

    expect(await screen.findByRole("button", { name: /collapse viaplay/i })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
  });
});
