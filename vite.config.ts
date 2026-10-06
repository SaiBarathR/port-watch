/// <reference types="vitest/config" />
import path from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  test: {
    // Component tests ask for jsdom at the top of their file.
    environment: "node",
    setupFiles: ["src/test/setup.ts"],
    testTimeout: 20_000,
    // Reported by `npm run test:coverage` and in CI, and not enforced.
    coverage: {
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/**/*.test.{ts,tsx}",
        "src/**/*.d.ts",
        "src/test/**",
        "src/main.tsx",
      ],
      reporter: ["text"],
    },
  },
  build: {
    // The one chunk is about 535 kB (165 kB gzipped). Rollup warns at 500 kB
    // with the web in mind; the app's webview reads this file from disk, so
    // splitting it would add requests and save nothing.
    chunkSizeWarningLimit: 700,
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
