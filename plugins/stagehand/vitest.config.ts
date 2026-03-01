import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    globals: true,
    testTimeout: 60_000,
    exclude: ["tests/qa/**", "node_modules/**"],
  },
});
