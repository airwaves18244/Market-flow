import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { DEFAULTS, loadSettings, saveSettings, syncSettingsWithCore } from "./settings";
import { handle, resetCoreMockForTests } from "./mock";

const SETTINGS_KEY = "market-terminal:settings";
const MIGRATED_KEY = "market-terminal:settings-core-migrated";

describe("settings", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("returns defaults when nothing is stored", () => {
    expect(loadSettings()).toEqual(DEFAULTS);
  });

  it("round-trips a saved value", () => {
    const custom = { ...DEFAULTS, tapeLimit: 100, domDepth: 20, topMoversLimit: 5 };
    saveSettings(custom);
    expect(loadSettings()).toEqual(custom);
  });

  it("fills in missing nested passport/LLM keys with defaults", () => {
    localStorage.setItem(
      "market-terminal:settings",
      JSON.stringify({ markets: { fx: true } }),
    );
    const loaded = loadSettings();
    expect(loaded.markets).toEqual({ ...DEFAULTS.markets, fx: true });
    expect(loaded.watchlist).toEqual(DEFAULTS.watchlist);
    expect(loaded.llmProvider).toBe(DEFAULTS.llmProvider);
  });

  it("fills in missing keys with defaults", () => {
    localStorage.setItem("market-terminal:settings", JSON.stringify({ tapeLimit: 7 }));
    expect(loadSettings()).toEqual({ ...DEFAULTS, tapeLimit: 7 });
  });

  it("falls back to defaults on corrupt JSON", () => {
    localStorage.setItem("market-terminal:settings", "{not json");
    expect(loadSettings()).toEqual(DEFAULTS);
  });
});

// ── T3 — Персист в ядро: миграция localStorage → ядро (мок IPC) ──────────────
// `syncSettingsWithCore()` — no-op без Tauri (см. `saveSettings`/`loadSettings`
// тесты выше, которые бегают в обычном jsdom без `__TAURI_INTERNALS__`). Здесь
// эмулируем окружение Tauri, чтобы упражнять реальный путь миграции поверх
// мок-IPC (`lib/mock.ts`) — так же, как это делает браузерная dev-сборка.

describe("settings core sync (Tauri mock IPC)", () => {
  beforeEach(() => {
    localStorage.clear();
    resetCoreMockForTests();
    // `ipc.ts` в режиме Tauri вызывает настоящий `@tauri-apps/api/core.js`,
    // который делегирует в `window.__TAURI_INTERNALS__.invoke(cmd, args)` —
    // подставляем туда мок-диспетчер, чтобы получить и "under Tauri" ветку
    // кода в `settings.ts`, и реальные мок-обработчики (без настоящего бэкенда).
    (window as unknown as { __TAURI_INTERNALS__: { invoke: typeof handle } }).__TAURI_INTERNALS__ =
      { invoke: handle };
  });

  afterEach(() => {
    delete (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  });

  it("migrates existing localStorage state into core on first run", async () => {
    const custom = { ...DEFAULTS, tapeLimit: 123, domDepth: 7 };
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(custom));

    await syncSettingsWithCore();

    expect(localStorage.getItem(MIGRATED_KEY)).toBe("1");
    // Локальный кэш после синка отражает то, что осело в ядре после миграции.
    expect(loadSettings().tapeLimit).toBe(123);
    expect(loadSettings().domDepth).toBe(7);
  });

  it("seeds core with defaults when nothing is in localStorage yet", async () => {
    await syncSettingsWithCore();

    expect(localStorage.getItem(MIGRATED_KEY)).toBe("1");
    expect(loadSettings()).toEqual(DEFAULTS);
  });

  it("does not re-run the migration on a second sync — core stays source of truth", async () => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ ...DEFAULTS, tapeLimit: 1 }));
    await syncSettingsWithCore();

    // Изменение приходит "из ядра" (saveSettings под Tauri пишет туда же,
    // куда читает syncSettingsWithCore) — второй синк должен подтянуть его,
    // а не повторно мигрировать локальный кэш поверх.
    saveSettings({ ...DEFAULTS, tapeLimit: 999 });
    await syncSettingsWithCore();

    expect(loadSettings().tapeLimit).toBe(999);
  });

  it("saveSettings under Tauri writes through to core (visible after a fresh sync)", async () => {
    await syncSettingsWithCore(); // первичная миграция — до кастомного сохранения
    saveSettings({ ...DEFAULTS, llmProvider: "anthropic", rate: 2.5 });

    // Симулируем перезапуск: локальный кэш стирается, остаётся только "ядро".
    localStorage.removeItem(SETTINGS_KEY);
    localStorage.removeItem(MIGRATED_KEY);
    await syncSettingsWithCore();

    expect(loadSettings().llmProvider).toBe("anthropic");
    expect(loadSettings().rate).toBe(2.5);
  });
});
