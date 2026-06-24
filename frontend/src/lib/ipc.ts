// Типизированная обёртка над Tauri `invoke`.
//
// Вне Tauri (обычный браузер / `npm run dev`) команды берут демо-данные из
// `mock.ts`, поэтому фронт собирается и запускается без Rust-ядра. Внутри
// Tauri вызываются реальные команды из `crates/app` (Фаза 3, § 3.2).

import type { Breadth, EquityDashboard, FlowPoint, RrgPoint } from "./types";
import { mockBreadth, mockEquityDashboard, mockFlowSeries, mockRrg } from "./mock";

function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function call<T>(
  command: string,
  args: Record<string, unknown>,
  fallback: () => T,
): Promise<T> {
  if (!inTauri()) {
    return fallback();
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

/** Снимок дашборда акций: обороты по секторам + топ-движения. */
export function equityDashboard(
  fromTs: number,
  toTs: number,
  moversLimit = 20,
): Promise<EquityDashboard> {
  return call(
    "equity_dashboard",
    { fromTs, toTs, moversLimit },
    () => mockEquityDashboard(fromTs, toTs),
  );
}

/** Временной ряд нетто-потока инструмента за период. */
export function flowSeries(symbol: string, fromTs: number, toTs: number): Promise<FlowPoint[]> {
  return call("flow_series", { symbol, fromTs, toTs }, () => mockFlowSeries());
}

/** Ширина рынка (A/D) за период. Бэкенд-команда — TODO (§ 3.2). */
export function breadth(fromTs: number, toTs: number): Promise<Breadth> {
  return call("breadth", { fromTs, toTs }, () => mockBreadth());
}

/** Секторная ротация RRG за период. Бэкенд-команда — TODO (§ 3.2). */
export function rrg(fromTs: number, toTs: number): Promise<RrgPoint[]> {
  return call("rrg", { fromTs, toTs }, () => mockRrg());
}
