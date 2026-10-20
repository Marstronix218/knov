import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    restoreMocks: true,
    // Full-page App renders can exceed the 5s default when the suite runs under load.
    testTimeout: 15000,
    hookTimeout: 15000,
  },
});
