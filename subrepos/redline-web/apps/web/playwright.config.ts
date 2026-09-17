import { defineConfig, devices } from "@playwright/test";

import { serverBinaryPath } from "./src/lib/server-binary";

// Web e2e smoke: boot the REAL built binary (which embeds apps/web/dist) and
// drive the served UI + a live /api/query round-trip. The binary is built by
// ops/ci/e2e.sh before this config runs.
const PORT = 7801;
const BASE_URL = `http://127.0.0.1:${PORT}`;

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: [["list"], ["html", { open: "never" }]],
  outputDir: "test-results",
  use: {
    baseURL: BASE_URL,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
  ],
  webServer: {
    command: `"${serverBinaryPath(process.env.CARGO_TARGET_DIR)}" --target-bin "${process.env.REDLINE_WEB_TARGET_BIN || "../../../../target/release/redlinedb"}" --bind 127.0.0.1:${PORT}`,
    url: `${BASE_URL}/api/health`,
    timeout: 60_000,
    reuseExistingServer: !process.env.CI,
  },
});
