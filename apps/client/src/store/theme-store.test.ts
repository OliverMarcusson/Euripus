import { useThemeStore } from "@/store/theme-store";

describe("theme store", () => {
  beforeEach(() => {
    window.localStorage.clear();
    document.documentElement.classList.remove("dark");
    document.documentElement.removeAttribute("data-theme");
    document.documentElement.style.colorScheme = "";
    useThemeStore.setState({ preference: "system", resolvedTheme: "light" });
  });

  it("applies dark mode when selected", () => {
    useThemeStore.getState().setPreference("dark");

    expect(useThemeStore.getState().preference).toBe("dark");
    expect(useThemeStore.getState().resolvedTheme).toBe("dark");
    expect(window.localStorage.getItem("euripus-theme-preference")).toBe("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(document.documentElement.style.colorScheme).toBe("dark");
  });

  it("resolves the system theme when following the OS preference", () => {
    vi.mocked(window.matchMedia).mockImplementation((query: string) => ({
      matches: true,
      media: query,
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }));

    useThemeStore.getState().setPreference("system");
    useThemeStore.getState().syncResolvedTheme();

    expect(useThemeStore.getState().resolvedTheme).toBe("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });
});
