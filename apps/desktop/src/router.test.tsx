import { render, screen } from "@testing-library/react";
import { AuthEntry, IndexRedirect } from "@/router";
import { useAuthStore } from "@/store/auth-store";

vi.mock("@tanstack/react-router", async () => {
  const actual = await vi.importActual<typeof import("@tanstack/react-router")>("@tanstack/react-router");
  return {
    ...actual,
    Navigate: ({ to }: { to: string }) => <div data-testid="navigate">{to}</div>,
  };
});

describe("router session redirects", () => {
  beforeEach(() => {
    useAuthStore.setState({
      user: null,
      accessToken: null,
      expiresAt: null,
      hydrated: false,
    });
  });

  it("waits for hydration on the index route", () => {
    render(<IndexRedirect />);

    expect(screen.getByText("Loading session...")).toBeInTheDocument();
  });

  it("sends hydrated guests to auth from the index route", () => {
    useAuthStore.setState({ hydrated: true });

    render(<IndexRedirect />);

    expect(screen.getByTestId("navigate")).toHaveTextContent("/auth");
  });

  it("redirects hydrated users away from auth", () => {
    useAuthStore.setState({
      hydrated: true,
      user: {
        id: "user-1",
        username: "oliver",
        createdAt: new Date().toISOString(),
      },
    });

    render(<AuthEntry />);

    expect(screen.getByTestId("navigate")).toHaveTextContent("/guide");
  });
});
