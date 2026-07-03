import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Tauri expects the frontend at localhost:1420 with build output in dist/
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Don't watch the Rust backend (avoids redundant reloads)
      ignored: ["**/src-tauri/**", "**/crates/**"],
    },
  },
  build: {
    target: "es2021",
    outDir: "dist",
  },
});
