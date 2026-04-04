import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AuthPage } from "@/features/auth/auth-page";

vi.mock("@tanstack/react-router", async () => {
  const actual = await vi.importActual<typeof import("@tanstack/react-router")>("@tanstack/react-router");
  return {
    ...actual,
    useNavigate: () => vi.fn(),
  };
});

describe("AuthPage", () => {
  it("renders auth controls", () => {
    render(
      <QueryClientProvider client={new QueryClient()}>
        <AuthPage />
      </QueryClientProvider>,
    );

    expect(screen.getByText("Welcome back")).toBeInTheDocument();
    expect(screen.getByLabelText("Username")).toBeInTheDocument();
  });
});
