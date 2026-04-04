import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AuthPage } from "@/features/auth/auth-page";
import { getServerNetworkStatus, login, register } from "@/lib/api";

vi.mock("@tanstack/react-router", async () => {
  const actual = await vi.importActual<typeof import("@tanstack/react-router")>("@tanstack/react-router");
  return {
    ...actual,
    useNavigate: () => vi.fn(),
  };
});

vi.mock("@/lib/api", () => ({
  getServerNetworkStatus: vi.fn(),
  login: vi.fn(),
  register: vi.fn(),
}));

describe("AuthPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(getServerNetworkStatus).mockResolvedValue({
      serverStatus: "online",
      vpnActive: true,
      vpnProvider: "NordVPN",
      publicIp: "198.51.100.24",
      publicIpCheckedAt: "2026-04-05T10:00:00.000Z",
      publicIpError: null,
    });
    vi.mocked(login).mockResolvedValue({} as never);
    vi.mocked(register).mockResolvedValue({} as never);
  });

  it("renders auth controls", () => {
    render(
      <QueryClientProvider client={new QueryClient()}>
        <AuthPage />
      </QueryClientProvider>,
    );

    expect(screen.getByText("Welcome back")).toBeInTheDocument();
    expect(screen.getByLabelText("Username")).toBeInTheDocument();
    expect(screen.getByText("Server route")).toBeInTheDocument();
  });
});
