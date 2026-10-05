import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  testMatch: "**/*.browser.ts",
  timeout: 60_000,
  fullyParallel: false,
  workers: 1,
  reporter: "line",
  use: {
    browserName: "chromium",
    baseURL: "http://127.0.0.1:4173",
    viewport: { width: 1600, height: 1000 },
    deviceScaleFactor: 1,
    screenshot: "only-on-failure",
  },
  webServer: {
    command: "pnpm exec vite preview --host 127.0.0.1 --port 4173 --strictPort",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: false,
    timeout: 30_000,
  },
});
