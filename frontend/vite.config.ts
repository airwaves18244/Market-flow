import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Конфигурация под Tauri: фиксированный порт дев-сервера, без очистки экрана,
// чтобы логи Rust-ядра не затирались.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
});
