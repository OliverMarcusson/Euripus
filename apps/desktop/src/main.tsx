import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "@tanstack/react-router";
import { Toaster } from "sonner";
import { router } from "@/router";
import { useBootstrapSession } from "@/hooks/use-bootstrap-session";
import "./index.css";

const queryClient = new QueryClient();

function Bootstrapper() {
  useBootstrapSession();
  return <RouterProvider router={router} />;
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <Bootstrapper />
      <Toaster richColors position="top-right" />
    </QueryClientProvider>
  </React.StrictMode>,
);

