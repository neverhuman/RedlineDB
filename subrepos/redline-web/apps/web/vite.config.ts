import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// vitest ships its config types against its own (vite 5) copy of vite, which
// clashes with this project's vite 6 plugin types. Augment the vite 6 UserConfig
// locally so the inline `test` block below type-checks; vitest reads it at runtime.
declare module "vite" {
  interface UserConfig {
    test?: import("vitest/node").InlineConfig;
  }
}

// The Rust backend embeds web/dist via rust-embed and serves it at "/".
// During dev, proxy /api and /metrics to the running backend on :7788.
export default defineConfig({
  plugins: [react()],
  base: "/",
  build: {
    outDir: "dist",
    emptyOutDir: true,
    sourcemap: true,
  },
  server: {
    port: 5173,
    proxy: {
      "/api": "http://127.0.0.1:7788",
      "/metrics": "http://127.0.0.1:7788",
    },
  },
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: ["./test/setup.ts"],
    css: false,
    // Unit/component tests only. The Playwright e2e specs (e2e/*.spec.ts) are
    // driven by `playwright test`, not vitest.
    include: ["src/**/*.test.{ts,tsx}"],
    exclude: ["e2e/**", "node_modules/**", "dist/**"],
  },
});
