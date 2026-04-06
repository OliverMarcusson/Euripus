import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react-swc";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath, URL } from "node:url";

const apiProxyTarget = process.env.VITE_DEV_PROXY_TARGET ?? "http://127.0.0.1:8080";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  server: {
    host: "127.0.0.1",
    proxy: {
      "/api": apiProxyTarget,
      "/health": apiProxyTarget,
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: "./src/test/setup.ts",
  },
});
