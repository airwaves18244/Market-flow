import { describe, expect, it } from "vitest";
import { fmtFixed, fmtInt, fmtRu, fmtRuFixed } from "./format";

describe("fmtFixed", () => {
  it("formats with fixed decimals, no grouping", () => {
    expect(fmtFixed(1234.5, 2)).toBe("1234.50");
  });

  it("defaults to 2 decimals", () => {
    expect(fmtFixed(1.5)).toBe("1.50");
  });

  it("renders null as the infinity symbol", () => {
    expect(fmtFixed(null)).toBe("∞");
  });
});

describe("fmtRu", () => {
  it("groups thousands and trims trailing zeros within the cap", () => {
    expect(fmtRu(1234.5)).toBe("1 234,5");
  });

  it("caps fraction digits without padding", () => {
    expect(fmtRu(1)).toBe("1");
  });
});

describe("fmtRuFixed", () => {
  it("pads to exactly the requested decimals", () => {
    expect(fmtRuFixed(1234.5, 2)).toBe("1 234,50");
  });

  it("defaults to 2 decimals", () => {
    expect(fmtRuFixed(1)).toBe("1,00");
  });
});

describe("fmtInt", () => {
  it("rounds and groups thousands", () => {
    expect(fmtInt(1234.6)).toBe("1 235");
  });
});
