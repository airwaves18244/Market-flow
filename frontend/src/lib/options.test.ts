import { describe, it, expect } from "vitest";
import { ipc } from "./ipc";

// Мок-режим (jsdom без Tauri): проверяем, что опционные команды дают
// осмысленные значения — фронт «Опционы» работает в браузере без бэкенда.

describe("options ipc (mock mode)", () => {
  it("prices an ATM call with sane greeks", async () => {
    const out = await ipc.optionPrice({
      forward: 100,
      strike: 100,
      t: 0.25,
      vol: 0.3,
      kind: "call",
    });
    expect(out.price).toBeGreaterThan(0);
    expect(out.greeks.delta).toBeGreaterThan(0);
    expect(out.greeks.delta).toBeLessThan(1);
    expect(out.greeks.vega).toBeGreaterThan(0);
  });

  it("recovers implied volatility from a priced option", async () => {
    const priced = await ipc.optionPrice({
      forward: 100,
      strike: 105,
      t: 0.5,
      vol: 0.25,
      kind: "put",
    });
    const iv = await ipc.optionImpliedVol({
      marketPrice: priced.price,
      forward: 100,
      strike: 105,
      t: 0.5,
      kind: "put",
    });
    expect(iv.iv).not.toBeNull();
    expect(Math.abs((iv.iv as number) - 0.25)).toBeLessThan(0.01);
  });

  it("lists smile models and fits a curve", async () => {
    const models = await ipc.listSmileModels();
    expect(models.map((m) => m.id)).toContain("svi");
    const fit = await ipc.smileFit({
      model: "svi",
      forward: 100,
      t: 0.25,
      points: [
        { strike: 90, iv: 0.3 },
        { strike: 100, iv: 0.27 },
        { strike: 110, iv: 0.29 },
      ],
    });
    expect(fit.curve.length).toBeGreaterThan(2);
    expect(fit.params.length).toBeGreaterThan(0);
    expect(fit.rmse).toBeGreaterThanOrEqual(0);
  });

  it("evaluates a long call: capped loss, unbounded profit, payoff curve", async () => {
    const res = await ipc.strategyEval({
      legs: [
        { kind: "call", side: "long", strike: 100, expiryT: 0.25, quantity: 1, entryPrice: 5 },
      ],
      priceLo: 80,
      priceHi: 130,
      forward: 100,
      vol: 0.3,
    });
    expect(res.payoff.length).toBeGreaterThan(2);
    // Убыток длинного колла ограничен премией; глубоко в деньгах прибыль растёт.
    expect(res.maxLoss).toBeLessThan(0);
    expect(res.maxProfit).toBeGreaterThan(0);
    expect(res.greeks.delta).toBeGreaterThan(0);
  });
});
