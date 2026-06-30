/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Конфиг Vite. Порт фиксирован — Tauri (devUrl в tauri.conf.json) ждёт 5173.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
  },
  build: {
    target: "esnext",
    outDir: "dist",
    emptyOutDir: true,
    // Выносим тяжёлые графические библиотеки в отдельные чанки: они меняются
    // редко и кешируются браузером независимо от кода приложения.
    rollupOptions: {
      output: {
        manualChunks: {
          echarts: ["echarts"],
          "lightweight-charts": ["lightweight-charts"],
        },
      },
    },
  },
});
