import { defineConfig } from "vitest/config";

// Run the type-level assertions in contracts.test.ts as part of `vitest run`,
// so contract drift fails the test command and not only the separate typecheck.
export default defineConfig({
  test: {
    typecheck: {
      enabled: true,
      include: ["src/**/*.test.ts"],
    },
  },
});
