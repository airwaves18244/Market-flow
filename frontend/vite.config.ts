import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Конфиг Vite. Порт фиксирован — Tauri (devUrl в tauri.conf.json) ждёт 5173.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "esnext", outDir: "dist", emptyOutDir: true },
});
