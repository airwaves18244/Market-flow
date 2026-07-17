// Типизированный клиент IPC к Rust-ядру.
//
// В среде Tauri вызывает реальные команды через `@tauri-apps/api`; в обычном
// браузере (разработка/сборка без бэкенда) — отдаёт мок-данные. Аргументы
// именуются camelCase: Tauri преобразует их в snake_case параметры команд.

import type {
  AccountDto,
  AlertEventDto,
  AlertRuleInput,
  AlgoMarket,
  BacktestConfigInput,
  BacktestReportDto,
  BarPoint,
  BondIssuerDto,
  BreadthDto,
  CrossAssetSummaryDto,
  DatasetIdInput,
  DatasetMetaDto,
  FillEventDto,
  FlowEdgeDto,
  FootprintBarDto,
  FutoiDto,
  FutureGroupDto,
  Hi2Dto,
  HistoryDoneEvent,
  HistoryErrorEvent,
  HistoryLoadInput,
  HistoryPlanInput,
  HistoryProgressEvent,
  HistoryTaskDto,
  ImpliedVolDto,
  ImpliedVolInput,
  InstrumentDto,
  KeyActivityPeriod,
  KeyActivityRowDto,
  KeyActivityRuleDto,
  KeyActivitySampleInput,
  KeyActivitySummaryDto,
  MegaAlertDto,
  MegaThresholdsInput,
  OptionBoardDto,
  OptionBoardInput,
  OptionPriceDto,
  OptionPriceInput,
  OrderBookDto,
  OrderDto,
  OrderInput,
  PositionDto,
  RobotConfigInput,
  RobotSignalDto,
  RrgSectorDto,
  SectorEntryDto,
  SectorRow,
  SettingsDto,
  SmileFitDto,
  SmileFitInput,
  SmileModelDto,
  StrategyDescriptorDto,
  StrategyEvalDto,
  StrategyEvalInput,
  SubmitResultDto,
  TimeFrame,
  TimeRangeDto,
  TopMoverDto,
  TradeDto,
  TradestatsDto,
  TurnoverByClassPoint,
  TurnoverPoint,
  YieldCurvePoint,
} from "./types";
import * as mock from "./mock";

export function inTauri(): boolean {
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

  // ── Фаза 12 / Опционы ───────────────────────────────────────────────────────
  listSmileModels: () => invoke<SmileModelDto[]>("list_smile_models"),
  optionPrice: (input: OptionPriceInput) => invoke<OptionPriceDto>("option_price", { input }),
  optionImpliedVol: (input: ImpliedVolInput) =>
    invoke<ImpliedVolDto>("option_implied_vol", { input }),
  smileFit: (input: SmileFitInput) => invoke<SmileFitDto>("smile_fit", { input }),
  strategyEval: (input: StrategyEvalInput) => invoke<StrategyEvalDto>("strategy_eval", { input }),
  // Опционная доска MOEX (фаза 12.4). В Tauri-сборке без фичи `moex` команда
  // отсутствует и вызов отклоняется — вызывающая сторона обязана иметь фолбэк
  // (SmileView возвращается к демо-точкам).
  optionBoard: (input: OptionBoardInput) => invoke<OptionBoardDto>("option_board", { input }),

  // ── Фаза 10 / MOEX ALGO: Key Activity ───────────────────────────────────────
  keyActivity: (samples: KeyActivitySampleInput[], period?: KeyActivityPeriod) =>
    invoke<KeyActivityRowDto[]>("key_activity", { samples, period }),
  keyActivitySummary: (samples: KeyActivitySampleInput[], period?: KeyActivityPeriod) =>
    invoke<KeyActivitySummaryDto>("key_activity_summary", { samples, period }),
  keyActivityRules: () => invoke<KeyActivityRuleDto[]>("key_activity_rules"),

  // ── T11 / MOEX ALGO: датасеты ALGOPACK ───────────────────────────────────────
  algoTradestats: (market: AlgoMarket, secid: string, fromTs: number, toTs: number) =>
    invoke<TradestatsDto[]>("algo_tradestats", { market, secid, fromTs, toTs }),
  algoFutoi: (market: AlgoMarket, secid: string, fromTs: number, toTs: number) =>
    invoke<FutoiDto[]>("algo_futoi", { market, secid, fromTs, toTs }),
  algoHi2: (market: AlgoMarket, secid: string, fromTs: number, toTs: number) =>
    invoke<Hi2Dto[]>("algo_hi2", { market, secid, fromTs, toTs }),
  // Батч «последние точки HI2» по списку тикеров: сортировка по концентрации
  // и топ-`limit` — на стороне ядра (без полной истории ×N тикеров).
  algoHi2Ranking: (market: AlgoMarket, secids: string[], limit: number) =>
    invoke<Hi2Dto[]>("algo_hi2_ranking", { market, secids, limit }),
  algoMegaAlerts: (
    market: AlgoMarket,
    secids: string[],
    fromTs: number,
    toTs: number,
    thresholds?: MegaThresholdsInput,
  ) => invoke<MegaAlertDto[]>("algo_mega_alerts", { market, secids, fromTs, toTs, thresholds }),

  // ── Фаза 11 / Историзация ────────────────────────────────────────────────────
  historyDatasets: () => invoke<DatasetMetaDto[]>("history_datasets"),
  historyDelete: (id: DatasetIdInput) => invoke<boolean>("history_delete", { id }),
  historyPlan: (input: HistoryPlanInput) => invoke<TimeRangeDto[]>("history_plan", { input }),
  // T10 — фоновая загрузка истории: старт (возвращает id задачи) и отмена
  // (без id — все активные). Прогресс приходит событиями `history:*` (см. ниже).
  historyLoad: (input: HistoryLoadInput) => invoke<HistoryTaskDto>("history_load", { input }),
  historyCancel: (taskId?: number) => invoke<number>("history_cancel", { taskId }),
  // Превью загруженного датасета свечами (11.4.4).
  historyPreview: (source: string, secid: string, tf: string, limit?: number) =>
    invoke<BarPoint[]>("history_preview", { source, secid, tf, limit }),

  // ── T3 / Настройки и правила Key Activity (10.5.3/S.2.2) ────────────────────
  settingsGet: () => invoke<SettingsDto>("settings_get"),
  settingsSet: (doc: SettingsDto) => invoke<void>("settings_set", { doc }),
  keyActivityRulesGet: () => invoke<KeyActivityRuleDto[]>("key_activity_rules_get"),
  // `rulesJson` — JSON-массив в формате доменной модели `domain::keyactivity::Rule`
  // (сериализуется вызывающей стороной; см. `SettingsTab.svelte`).
  keyActivityRulesSet: (rulesJson: string) =>
    invoke<KeyActivityRuleDto[]>("key_activity_rules_set", { rulesJson }),
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

// ── T10 — Историзация: события хода загрузки (`history:*`) ────────────────────
// В браузере (мок-режим) — no-op: там прогресс симулируется во вкладке
// `HistoryTab`. В Tauri — реальные события фонового загрузчика.

export async function onHistoryProgress(
  cb: (p: HistoryProgressEvent) => void,
): Promise<Unlisten> {
  if (!inTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<HistoryProgressEvent>("history:progress", (e) => cb(e.payload));
}

export async function onHistoryDone(cb: (d: HistoryDoneEvent) => void): Promise<Unlisten> {
  if (!inTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<HistoryDoneEvent>("history:done", (e) => cb(e.payload));
}

export async function onHistoryError(cb: (err: HistoryErrorEvent) => void): Promise<Unlisten> {
  if (!inTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<HistoryErrorEvent>("history:error", (e) => cb(e.payload));
}
