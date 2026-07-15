import { describe, it, expect } from "vitest";
import { ipc } from "./ipc";
import { handle } from "./mock";

// Эти тесты бегают в jsdom без `__TAURI_INTERNALS__`, поэтому `ipc` всегда
// падает в мок-режим — ровно то, что использует браузерная разработка/сборка.

describe("ipc (mock mode)", () => {
  it("lists instruments", async () => {
    const instruments = await ipc.instruments();
    expect(instruments.length).toBeGreaterThan(0);
    expect(instruments[0]).toHaveProperty("symbol");
  });

  it("generates bars for a symbol", async () => {
    const bars = await ipc.bars("SBER@MISX", "d1", 0, 0);
    expect(bars.length).toBeGreaterThan(0);
    for (const b of bars) {
      expect(b.high).toBeGreaterThanOrEqual(b.low);
    }
  });

  it("lists backtest strategies with their params", async () => {
    const strategies = await ipc.listStrategies();
    expect(strategies.map((s) => s.id)).toContain("ma_cross");
  });

  it("runs a mock backtest and returns consistent metrics", async () => {
    const report = await ipc.runBacktest(
      "SBER@MISX",
      "d1",
      0,
      0,
      "ma_cross",
      {},
      { initialCapital: 100_000, commission: 0, slippage: 0 },
    );
    expect(report.metrics.trades).toBe(report.trades.length);
    expect(report.metrics.wins + report.metrics.losses).toBeLessThanOrEqual(report.metrics.trades);
    expect(report.equityCurve.length).toBe(report.trades.length > 0 ? report.equityCurve.length : 0);
  });

  it("rejects an unknown IPC command", async () => {
    await expect(handle("not_a_real_command")).rejects.toThrow();
  });

  it("submits a market order, fills it immediately and updates account/positions", async () => {
    const accountBefore = await ipc.account();
    const result = await ipc.submitOrder({ symbol: "SBER@MISX", side: "buy", qty: 10, kind: "market" });

    expect(result.order.status).toBe("filled");
    expect(result.order.filled).toBe(10);
    expect(result.fills).toHaveLength(1);

    const positions = await ipc.positions();
    const sber = positions.find((p) => p.symbol === "SBER@MISX");
    expect(sber).toBeDefined();
    expect(sber!.qty).toBeGreaterThan(0);

    const accountAfter = await ipc.account();
    expect(accountAfter.cash).toBeLessThan(accountBefore.cash);
  });

  it("rests a limit order in the blotter and cancels it", async () => {
    const before = await ipc.orderBlotter();
    const result = await ipc.submitOrder({
      symbol: "GAZP@MISX",
      side: "buy",
      qty: 5,
      kind: "limit",
      price: 1,
    });
    expect(result.order.status).toBe("new");
    expect(result.fills).toHaveLength(0);

    const afterSubmit = await ipc.orderBlotter();
    expect(afterSubmit.length).toBe(before.length + 1);

    const cancelled = await ipc.cancelOrder(result.order.id);
    expect(cancelled.status).toBe("cancelled");

    const afterCancel = await ipc.orderBlotter();
    expect(afterCancel.length).toBe(before.length);
  });

  it("raises when cancelling an order that does not exist", async () => {
    await expect(ipc.cancelOrder(999_999)).rejects.toThrow();
  });

  // ── T11 — MOEX ALGO: датасеты ALGOPACK ─────────────────────────────────────

  it("generates Super Candles (tradestats) with buy pressure in 0..1", async () => {
    const candles = await ipc.algoTradestats("eq", "SBER", 0, 9_999_999_999);
    expect(candles.length).toBeGreaterThan(0);
    for (const c of candles) {
      expect(c.secid).toBe("SBER");
      expect(c.buyPressure).toBeGreaterThanOrEqual(0);
      expect(c.buyPressure).toBeLessThanOrEqual(1);
      expect(c.prHigh).toBeGreaterThanOrEqual(c.prLow);
    }
  });

  it("generates FUTOI points split by client group", async () => {
    const points = await ipc.algoFutoi("fo", "SBER", 0, 9_999_999_999);
    expect(points.length).toBeGreaterThan(0);
    expect(points.some((p) => p.clgroup === "fiz")).toBe(true);
    expect(points.some((p) => p.clgroup === "yur")).toBe(true);
    for (const p of points) {
      expect(p.net).toBeCloseTo(p.posLong - p.posShort);
    }
  });

  it("generates HI2 points with a consistent level classification", async () => {
    const points = await ipc.algoHi2("eq", "SBER", 0, 9_999_999_999);
    expect(points.length).toBeGreaterThan(0);
    for (const p of points) {
      if (p.concentration >= 0.5) expect(p.level).toBe("dominated");
      else if (p.concentration >= 0.25) expect(p.level).toBe("concentrated");
      else if (p.concentration >= 0.15) expect(p.level).toBe("moderate");
      else expect(p.level).toBe("distributed");
    }
  });

  it("generates Mega Alerts scoped to the requested instruments", async () => {
    const alerts = await ipc.algoMegaAlerts("eq", ["SBER", "GAZP"], 0, 9_999_999_999);
    expect(alerts.length).toBeGreaterThan(0);
    for (const a of alerts) {
      expect(["SBER", "GAZP"]).toContain(a.secid);
      expect(a.message.length).toBeGreaterThan(0);
    }
  });

  // ── T10 — Историзация: загрузка/отмена/превью ──────────────────────────────

  it("starts a history load, registering datasets and returning a task id", async () => {
    const before = await ipc.historyDatasets();
    const hadKey = before.some((d) => d.source === "finam" && d.secid === "TESTX" && d.tf === "d1");
    expect(hadKey).toBe(false);

    const task = await ipc.historyLoad({
      source: "finam",
      tickers: ["TESTX"],
      timeframes: ["d1", "h1"],
      from: 0,
      till: 86_400 * 10,
    });
    expect(task.taskId).toBeGreaterThan(0);

    const after = await ipc.historyDatasets();
    expect(after.some((d) => d.secid === "TESTX" && d.tf === "d1")).toBe(true);
    expect(after.some((d) => d.secid === "TESTX" && d.tf === "h1")).toBe(true);

    // Второй запуск выдаёт монотонно больший id задачи.
    const task2 = await ipc.historyLoad({
      source: "finam",
      tickers: ["TESTX"],
      timeframes: ["d1"],
      from: 0,
      till: 86_400 * 10,
    });
    expect(task2.taskId).toBeGreaterThan(task.taskId);
  });

  it("cancels a history load without error in mock mode", async () => {
    await expect(ipc.historyCancel()).resolves.toBe(0);
    await expect(ipc.historyCancel(1)).resolves.toBe(0);
  });

  it("previews a dataset with a bounded number of candles", async () => {
    const bars = await ipc.historyPreview("finam", "SBER", "d1", 50);
    expect(bars.length).toBeGreaterThan(0);
    expect(bars.length).toBeLessThanOrEqual(50);
    for (const b of bars) {
      expect(b.high).toBeGreaterThanOrEqual(b.low);
    }
  });
});
