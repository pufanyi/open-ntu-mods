import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      "/api": "http://localhost:3000",
      "/auth": "http://localhost:3000",
      "/openapi.json": "http://localhost:3000",
      "/health": "http://localhost:3000",
    },
  },
  test: {
    environment: "jsdom",
    setupFiles: "./vitest.setup.ts",
    exclude: ["src/e2e/**", "node_modules/**", "dist/**"],
  },
});
