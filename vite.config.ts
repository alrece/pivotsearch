import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Tauri 期望前端在 localhost:1420，且构建产物在 dist/
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // 不监听 Rust 后端变化（避免重复刷新）
      ignored: ["**/src-tauri/**", "**/crates/**"],
    },
  },
  build: {
    target: "es2021",
    outDir: "dist",
  },
});
