import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react-swc";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath, URL } from "node:url";
import type { IncomingMessage } from "node:http";

const apiProxyTarget = process.env.VITE_DEV_PROXY_TARGET ?? "http://127.0.0.1:8080";

function forwardedProtoForRequest(request: IncomingMessage) {
  const encrypted = (request.socket as IncomingMessage["socket"] & { encrypted?: boolean })
    .encrypted;
  return encrypted ? "https" : "http";
}

function configureDevApiProxy(proxy: any) {
  proxy.on("proxyReq", (proxyReq: any, request: IncomingMessage) => {
    const host = request.headers.host;
    if (host) {
      proxyReq.setHeader("x-forwarded-host", host);
      proxyReq.setHeader("x-forwarded-proto", forwardedProtoForRequest(request));
    }
  });
}

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  server: {
    host: "0.0.0.0",
    proxy: {
      "/api": {
        target: apiProxyTarget,
        configure: configureDevApiProxy,
      },
      "/health": {
        target: apiProxyTarget,
        configure: configureDevApiProxy,
      },
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: "./src/test/setup.ts",
  },
});
