import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// base: './' so the built page works from any static host and from a
// subdirectory. Hash routing means no server rewrites are needed.
export default defineConfig({
  base: "./",
  plugins: [react()],
  build: {
    outDir: "dist",
    target: "es2020",
    sourcemap: false,
  },
  test: {
    environment: "jsdom",
    globals: true,
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
