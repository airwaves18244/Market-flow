// Типизированный клиент IPC к Rust-ядру.
//
// В среде Tauri вызывает реальные команды через `@tauri-apps/api`; в обычном
// браузере (разработка/сборка без бэкенда) — отдаёт мок-данные. Аргументы
// именуются camelCase: Tauri преобразует их в snake_case параметры команд.

import type {
  AccountDto,
  AlertEventDto,
  AlertRuleInput,
  BacktestConfigInput,
  BacktestReportDto,
  BarPoint,
  BondIssuerDto,
  BreadthDto,
  CrossAssetSummaryDto,
  FillEventDto,
  FlowEdgeDto,
  FootprintBarDto,
  FutureGroupDto,
  InstrumentDto,
  OrderBookDto,
  OrderDto,
  OrderInput,
  PositionDto,
  RobotConfigInput,
  RobotSignalDto,
  RrgSectorDto,
  SectorEntryDto,
  SectorRow,
  StrategyDescriptorDto,
  SubmitResultDto,
  TimeFrame,
  TopMoverDto,
  TradeDto,
  TurnoverByClassPoint,
  TurnoverPoint,
  YieldCurvePoint,
} from "./types";
import * as mock from "./mock";

function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri()) {
    return mock.handle<T>(cmd, args);
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export const ipc = {
  instruments: () => invoke<InstrumentDto[]>("instruments"),

  bars: (symbol: string, timeframe: TimeFrame, fromTs: number, toTs: number) =>
    invoke<BarPoint[]>("bars", { symbol, timeframe, fromTs, toTs }),

  turnoverSeries: (symbol: string, fromTs: number, toTs: number) =>
    invoke<TurnoverPoint[]>("turnover_series", { symbol, fromTs, toTs }),

  sectorRollup: (fromTs: number, toTs: number) =>
    invoke<SectorRow[]>("sector_rollup", { fromTs, toTs }),

  sectorMap: () => invoke<SectorEntryDto[]>("sector_map"),

  breadthData: (fromTs: number, toTs: number) =>
    invoke<BreadthDto>("breadth_data", { fromTs, toTs }),

  topMovers: (fromTs: number, toTs: number, limit?: number) =>
    invoke<TopMoverDto[]>("top_movers", { fromTs, toTs, limit }),

  rrgSectors: (fromTs: number, toTs: number) =>
    invoke<RrgSectorDto[]>("rrg_sectors", { fromTs, toTs }),

  futuresRollup: (fromTs: number, toTs: number) =>
    invoke<FutureGroupDto[]>("futures_rollup", { fromTs, toTs }),

  bondsRollup: (fromTs: number, toTs: number) =>
    invoke<BondIssuerDto[]>("bonds_rollup", { fromTs, toTs }),

  yieldCurve: () =>
    invoke<YieldCurvePoint[]>("yield_curve"),

  crossAssetSummary: (fromTs: number, toTs: number) =>
    invoke<CrossAssetSummaryDto>("cross_asset_summary", { fromTs, toTs }),

  turnoverTimeline: (fromTs: number, toTs: number) =>
    invoke<TurnoverByClassPoint[]>("turnover_timeline", { fromTs, toTs }),

  flowSankey: (fromTs: number, toTs: number) =>
    invoke<FlowEdgeDto[]>("flow_sankey", { fromTs, toTs }),

  // ── Фаза 7 — live-панели ────────────────────────────────────────────────
  // Time&Sales и DOM в боевом режиме приходят live-push событиями
  // (`trade:tick` / `orderbook:tick`); в мок-режиме отдаются снимком.
  latestTrades: (symbol: string, limit?: number) =>
    invoke<TradeDto[]>("latest_trades", { symbol, limit }),

  orderBook: (symbol: string, depth?: number) =>
    invoke<OrderBookDto>("order_book", { symbol, depth }),

  alertsScan: (rules: AlertRuleInput[], fromTs: number, toTs: number) =>
    invoke<AlertEventDto[]>("alerts_scan", { rules, fromTs, toTs }),

  // ── V2 / Бэктестер ────────────────────────────────────────────────────────
  listStrategies: () => invoke<StrategyDescriptorDto[]>("list_strategies"),

  runBacktest: (
    symbol: string,
    timeframe: TimeFrame,
    fromTs: number,
    toTs: number,
    strategyId: string,
    params: Record<string, number>,
    config: BacktestConfigInput,
  ) =>
    invoke<BacktestReportDto>("run_backtest", {
      symbol,
      timeframe,
      fromTs,
      toTs,
      strategyId,
      params,
      config,
    }),

  // ── V2 / Delta ────────────────────────────────────────────────────────────
  deltaFootprint: (
    symbol: string,
    timeframe: TimeFrame,
    fromTs: number,
    toTs: number,
    tickSize: number,
  ) =>
    invoke<FootprintBarDto[]>("delta_footprint", {
      symbol,
      timeframe,
      fromTs,
      toTs,
      tickSize,
    }),

  robotScan: (symbol: string, fromTs: number, toTs: number, config: RobotConfigInput) =>
    invoke<RobotSignalDto[]>("robot_scan", { symbol, fromTs, toTs, config }),

  // ── V2 / Trade ────────────────────────────────────────────────────────────
  submitOrder: (order: OrderInput) => invoke<SubmitResultDto>("submit_order", { order }),
  cancelOrder: (id: number) => invoke<OrderDto>("cancel_order", { id }),
  orderBlotter: () => invoke<OrderDto[]>("order_blotter"),
  positions: () => invoke<PositionDto[]>("positions"),
  account: () => invoke<AccountDto>("account"),
};

// Подписки на live-push события (каналы `trade:tick` / `orderbook:tick`).
// В браузере (мок-режим) — no-op: данные отдаются первичным снимком из `ipc`.
// Возвращают функцию отписки.

type Unlisten = () => void;

export async function onTrade(cb: (t: TradeDto) => void): Promise<Unlisten> {
  if (!inTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<TradeDto>("trade:tick", (e) => cb(e.payload));
}

export async function onOrderBook(cb: (b: OrderBookDto) => void): Promise<Unlisten> {
  if (!inTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<OrderBookDto>("orderbook:tick", (e) => cb(e.payload));
}

// Исполнения симулятора (канал `fill:tick`). В браузере — no-op.
export async function onFill(cb: (f: FillEventDto) => void): Promise<Unlisten> {
  if (!inTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<FillEventDto>("fill:tick", (e) => cb(e.payload));
}
