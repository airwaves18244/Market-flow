import { describe, it, expect, beforeEach } from "vitest";
import { DEFAULTS, loadSettings, saveSettings } from "./settings";

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
