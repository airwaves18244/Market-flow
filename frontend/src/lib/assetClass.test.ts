import { describe, it, expect } from "vitest";
import { assetLabel, assetColor } from "./assetClass";

describe("assetClass", () => {
  it("labels known classes in Russian", () => {
    expect(assetLabel("equity")).toBe("Акции");
    expect(assetLabel("future")).toBe("Фьючерсы");
    expect(assetLabel("bond")).toBe("Облигации");
  });

  it("falls back to the raw code for unknown classes", () => {
    expect(assetLabel("crypto")).toBe("crypto");
  });

  it("colors known classes distinctly", () => {
    const colors = new Set(["equity", "future", "bond"].map(assetColor));
    expect(colors.size).toBe(3);
  });

  it("falls back to a default color for unknown classes", () => {
    expect(assetColor("crypto")).toBe("#8b949e");
  });
});
