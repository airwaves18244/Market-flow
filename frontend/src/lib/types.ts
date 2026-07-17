// Зеркало DTO из Rust-ядра (crates/app/src/dto.rs). Поля — camelCase,
// как сериализует serde с `rename_all = "camelCase"`.

export type AssetClass = "equity" | "future" | "bond";
export type TimeFrame = "m1" | "m5" | "m15" | "h1" | "d1";

export interface InstrumentDto {
  symbol: string;
  ticker: string;
  name: string;
  assetClass: string;
  sector: string | null;
}

export interface BarPoint {
  ts: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

export interface TurnoverPoint {
  ts: number;
  turnover: number;
  netFlow: number;
  change: number;
}

export interface SectorRow {
  sector: string;
  instruments: number;
  turnover: number;
  netFlow: number;
  weightedChange: number;
}

export interface SectorEntryDto {
  key: string;
  sector: string;
  isIsin: boolean;
}

export interface BreadthDto {
  advancers: number;
  decliners: number;
  unchanged: number;
  pctAdvancing: number | null;
  adRatio: number | null;
}

export interface TopMoverDto {
  symbol: string;
  ticker: string;
  name: string;
  sector: string | null;
  change: number;
  lastClose: number;
}

export interface RrgSectorDto {
  sector: string;
  rsRatio: number;
  rsMomentum: number;
  quadrant: "leading" | "weakening" | "lagging" | "improving";
}

export interface FutureGroupDto {
  group: string;
  contracts: number;
  turnover: number;
  netFlow: number;
  weightedChange: number;
  openInterest: number;
}

export interface BondIssuerDto {
  issuer: string;
  bonds: number;
  turnover: number;
  netFlow: number;
  avgYield: number;
  weightedDuration: number;
}

export interface YieldCurvePoint {
  maturityYears: number;
  yieldPct: number;
}

export interface AssetClassShareDto {
  assetClass: string;
  turnover: number;
  share: number;
}

export interface CrossAssetSummaryDto {
  total: number;
  shares: AssetClassShareDto[];
}

export interface TurnoverByClassPoint {
  ts: number;
  equity: number;
  future: number;
  bond: number;
}

export interface FlowEdgeDto {
  from: string;
  to: string;
  weight: number;
}

// ── Фаза 7 — live-панели (Time&Sales / DOM / алёрты) ──────────────────────

export interface TradeDto {
  ts: number;
  price: number;
  size: number;
  /** true — покупка (агрессор-бид), false — продажа, null — сторона неизвестна. */
  buyerInitiated: boolean | null;
}

export interface BookLevelDto {
  price: number;
  size: number;
}

export interface OrderBookDto {
  ts: number;
  /** Биды по убыванию цены (лучший — первый). */
  bids: BookLevelDto[];
  /** Аски по возрастанию цены (лучший — первый). */
  asks: BookLevelDto[];
}

export interface AlertEventDto {
  symbol: string;
  ts: number;
  price: number;
  /** Дневное изменение в долях (0.01 = +1%). */
  change: number;
  message: string;
}

export type AlertKind = "priceAbove" | "priceBelow" | "changeAbove" | "changeBelow";

/** Правило алёрта, отправляемое в ядро (вход IPC). */
export interface AlertRuleInput {
  symbol: string;
  kind: AlertKind;
  threshold: number;
}

// ── V2 / Бэктестер ─────────────────────────────────────────────────────────

export interface StrategyParamDto {
  name: string;
  label: string;
  default: number;
}

export interface StrategyDescriptorDto {
  id: string;
  label: string;
  params: StrategyParamDto[];
}

export type FillTiming = "nextOpen" | "thisClose";

export interface BacktestConfigInput {
  initialCapital: number;
  commission: number;
  slippage: number;
  fillTiming?: FillTiming;
}

export interface SimTradeDto {
  ts: number;
  /** buy|sell */
  side: string;
  qty: number;
  price: number;
  realizedPnl: number;
}

export interface EquityPointDto {
  ts: number;
  equity: number;
}

export interface PerfMetricsDto {
  netPnl: number;
  returnPct: number;
  trades: number;
  wins: number;
  losses: number;
  winRate: number;
  profitFactor: number;
  maxDrawdown: number;
  sharpe: number;
  avgWin: number;
  avgLoss: number;
}

export interface BacktestReportDto {
  trades: SimTradeDto[];
  equityCurve: EquityPointDto[];
  metrics: PerfMetricsDto;
}

// ── V2 / Delta (footprint + роботы) ─────────────────────────────────────────

export interface FootprintCellDto {
  price: number;
  bidVolume: number;
  askVolume: number;
  delta: number;
}

export interface FootprintBarDto {
  ts: number;
  cells: FootprintCellDto[];
  bidTotal: number;
  askTotal: number;
  delta: number;
  cumulativeDelta: number;
}

export type RobotKind = "same_lot" | "iceberg" | "absorption";

export interface RobotSignalDto {
  /** same_lot|iceberg|absorption */
  kind: string;
  ts: number;
  price: number;
  strength: number;
  note: string;
}

export interface RobotConfigInput {
  sameLotEnabled?: boolean;
  sameLotRun?: number;
  lotTolerance?: number;
  icebergEnabled?: boolean;
  icebergVolumeMult?: number;
  absorptionEnabled?: boolean;
  absorptionMinDelta?: number;
  absorptionMaxMove?: number;
}

// ── V2 / Trade (симулятор исполнения) ───────────────────────────────────────

export type OrderSide = "buy" | "sell";
export type OrderKind = "market" | "limit" | "stop";
export type Tif = "gtc" | "day" | "ioc";

/** Заявка, отправляемая в ядро (вход IPC). */
export interface OrderInput {
  symbol: string;
  side: OrderSide;
  qty: number;
  kind: OrderKind;
  price?: number | null;
  tif?: Tif | null;
}

export interface OrderDto {
  id: number;
  symbol: string;
  side: string;
  qty: number;
  filled: number;
  price: number | null;
  kind: string;
  status: string;
}

export interface FillEventDto {
  orderId: number;
  ts: number;
  side: string;
  qty: number;
  price: number;
  realizedPnl: number;
}

export interface PositionDto {
  symbol: string;
  qty: number;
  avgPrice: number;
}

export interface AccountDto {
  cash: number;
  realizedPnl: number;
}

export interface SubmitResultDto {
  order: OrderDto;
  fills: FillEventDto[];
}

// ── Фаза 12 — Опционы ────────────────────────────────────────────────────────

export type OptionKind = "call" | "put";
export type PricingModel = "black76" | "bachelier";
export type LegKind = "call" | "put" | "underlying";
export type LegSide = "long" | "short";

export interface GreeksDto {
  delta: number;
  gamma: number;
  vega: number;
  theta: number;
  rho: number;
}

export interface OptionPriceInput {
  forward: number;
  strike: number;
  t: number;
  vol: number;
  rate?: number | null;
  kind: OptionKind;
  model?: PricingModel | null;
}

export interface OptionPriceDto {
  price: number;
  greeks: GreeksDto;
}

export interface ImpliedVolInput {
  marketPrice: number;
  forward: number;
  strike: number;
  t: number;
  rate?: number | null;
  kind: OptionKind;
  model?: PricingModel | null;
}

export interface ImpliedVolDto {
  iv: number | null;
}

export interface SmilePointInput {
  strike: number;
  iv: number;
  weight?: number | null;
}

export interface SmileFitInput {
  model: string;
  points: SmilePointInput[];
  forward: number;
  t: number;
  curveLo?: number | null;
  curveHi?: number | null;
  curveSteps?: number | null;
}

export interface SmileParamDto {
  name: string;
  value: number;
}

export interface SmileCurvePoint {
  strike: number;
  iv: number;
}

export interface SmileFitDto {
  model: string;
  params: SmileParamDto[];
  rmse: number;
  curve: SmileCurvePoint[];
}

export interface SmileModelDto {
  id: string;
  name: string;
}

// ── Фаза 12.4 — Опционная доска MOEX ─────────────────────────────────────────

export interface OptionQuoteDto {
  secid: string;
  underlying: string;
  /** Дата экспирации серии, unix-секунды UTC. */
  expirationTs: number;
  strike: number;
  kind: OptionKind;
  bid: number | null;
  ask: number | null;
  last: number | null;
  iv: number | null;
  oi: number | null;
  theorPrice: number | null;
}

export interface OptionBoardInput {
  /** Код базового актива (например, фьючерса). */
  underlying: string;
  /** Экспирация серии для точек улыбки; по умолчанию — ближайшая на доске. */
  expirationTs?: number | null;
  /** Форвард-фолбэк, если доска не определила цену базового актива. */
  forwardHint?: number | null;
  /** Время до экспирации в годах. */
  t: number;
  rate?: number | null;
}

export interface OptionBoardDto {
  quotes: OptionQuoteDto[];
  forward: number | null;
  expirationTs: number | null;
  /** Готовые рыночные точки улыбки (вход `smile_fit`). */
  smilePoints: SmilePointInput[];
}

export interface StrategyLegInput {
  kind: LegKind;
  side: LegSide;
  strike: number;
  expiryT: number;
  quantity: number;
  entryPrice: number;
}

export interface StrategyEvalInput {
  legs: StrategyLegInput[];
  priceLo: number;
  priceHi: number;
  steps?: number | null;
  forward: number;
  vol: number;
  rate?: number | null;
  model?: PricingModel | null;
}

export interface StrategyPayoffPoint {
  price: number;
  pnlExpiry: number;
  pnlNow: number;
}

export interface StrategyEvalDto {
  breakevens: number[];
  maxProfit: number | null;
  maxLoss: number | null;
  netCost: number;
  payoff: StrategyPayoffPoint[];
  greeks: GreeksDto;
}

// ── Фаза 10 — MOEX ALGO: Key Activity ────────────────────────────────────────

export type KeyActivityPeriod = "1h" | "1d" | "1w" | "1m" | "3m";

export interface KeyActivitySampleInput {
  secid: string;
  ts: number;
  volume?: number;
  volumeZ?: number;
  disb?: number;
  oiChange?: number;
  hi2?: number;
  spread?: number;
  priceChange?: number;
}

export interface KeyActivityRowDto {
  secid: string;
  ruleId: string;
  ruleName: string;
  metric: string;
  value: number;
  ts: number;
  importance: number;
}

export interface KeyActivitySummaryDto {
  text: string;
  period: string;
  rowCount: number;
  /** Локальный свод (true) vs. ответ LLM (false). Эквивалент source !== "llm". */
  fallback: boolean;
  /** Источник текста: "llm" — живой ответ провайдера, "local" — локальный свод. */
  source: "llm" | "local";
}

export interface KeyActivityRuleDto {
  id: string;
  name: string;
  weight: number;
}

// ── Фаза 11 — Историзация: датасеты ──────────────────────────────────────────

export type DataSource = "finam" | "moex_algo";

export interface DatasetMetaDto {
  source: string;
  secid: string;
  tf: string;
  fromTs: number;
  toTs: number;
  bars: number;
  updatedTs: number;
  looksComplete: boolean;
}

export interface TimeRangeDto {
  from: number;
  till: number;
}

export interface HistoryPlanInput {
  covered: TimeRangeDto[];
  requestedFrom: number;
  requestedTill: number;
}

export interface DatasetIdInput {
  source: string;
  secid: string;
  tf: string;
}

// ── T10 — Историзация: загрузка (вход) и события хода (`history:*`) ───────────

export interface HistoryLoadInput {
  source: DataSource;
  tickers: string[];
  timeframes: string[];
  /** Начало окна, unix-секунды (включительно). */
  from: number;
  /** Конец окна, unix-секунды (исключительно, полуоткрытый `[from, till)`). */
  till: number;
  /** Рынок ALGOPACK для `moex_algo` (`eq|fo|fx`, дефолт `eq`); для `finam` игнорируется. */
  market?: AlgoMarket;
}

export interface HistoryTaskDto {
  taskId: number;
}

/** Событие `history:progress`: прогресс задачи (тикер × ТФ), `0..=100`. */
export interface HistoryProgressEvent {
  taskId: number;
  ticker: string;
  tf: string;
  percent: number;
}

/** Событие `history:done`: завершение задачи (`ticker` задан) или всей загрузки (`ticker` = null). */
export interface HistoryDoneEvent {
  taskId: number;
  ticker: string | null;
  tf: string | null;
  bars: number;
  summary: string;
}

/** Событие `history:error`: ошибка задачи (не прерывает остальные). */
export interface HistoryErrorEvent {
  taskId: number;
  ticker: string | null;
  tf: string | null;
  message: string;
}

// ── T3 — Персист настроек и правил Key Activity в ядро ───────────────────────
// (10.5.3 / S.2.2 / 10.8.* / 11.6.1 / 12.8.1)
//
// `SettingsDto` — документ, который `app::settings::SettingsStore` хранит в
// JSON-файле ОС-config-директории (единый источник истины вместо
// localStorage). Поля намеренно совпадают с `lib/settings.ts::Settings`
// (только перечисления здесь — простой `string`, а не литеральный union: см.
// конвертеры `toDto`/`fromDto` в `settings.ts`). Секретов здесь нет — только
// флаг `llmHasKey`.

export interface MarketsDto {
  eq: boolean;
  fo: boolean;
  fx: boolean;
}

export interface SettingsDto {
  tapeLimit: number;
  domDepth: number;
  topMoversLimit: number;
  markets: MarketsDto;
  watchlist: Record<string, boolean>;
  llmProvider: string;
  llmModel: string;
  llmHasKey: boolean;
  llmTokenLimit: number;
  llmAuto: boolean;
  defaultPeriod: string;
  dataSource: string;
  dataDir: string;
  concurrency: number;
  pricingModel: string;
  rate: number;
  defaultSmile: string;
}

// ── T11 — MOEX ALGO: датасеты ALGOPACK (Super Candles/FUTOI/HI2/Mega Alerts) ─

/** Рынок ALGOPACK: `eq` (акции), `fo` (срочный), `fx` (валютный). */
export type AlgoMarket = "eq" | "fo" | "fx";

/** Свеча Super Candles (датасет `tradestats`) — зеркало `dto::TradestatsDto`. */
export interface TradestatsDto {
  secid: string;
  ts: number;
  prOpen: number;
  prHigh: number;
  prLow: number;
  prClose: number;
  prStd: number;
  vol: number;
  val: number;
  trades: number;
  prVwap: number;
  prChange: number;
  volB: number;
  volS: number;
  valB: number;
  valS: number;
  tradesB: number;
  tradesS: number;
  /** Дисбаланс потока (−1..1). */
  disb: number;
  prVwapB: number;
  prVwapS: number;
  /** Индекс агрессии покупателей (0..1). */
  buyPressure: number;
  /** `true`, если объём этой свечи — аномальный выброс (z-score объёма
   * относительно окна предыдущих свечей ≥ порога на бэкенде; тот же детектор,
   * что и `volume_spike` в Mega Alerts). Проставляется на бэкенде/в моке —
   * фронт не пересчитывает эвристику сам. */
  isAnomVol: boolean;
}

/** Точка FUTOI (открытый интерес физ/юр лиц) — зеркало `dto::FutoiDto`. */
export interface FutoiDto {
  secid: string;
  ts: number;
  /** `fiz|yur`. */
  clgroup: "fiz" | "yur";
  pos: number;
  posLong: number;
  posShort: number;
  posLongNum: number;
  posShortNum: number;
  net: number;
  longShare: number;
}

/** Точка HI2 (индекс концентрации участников) — зеркало `dto::Hi2Dto`. */
export interface Hi2Dto {
  ts: number;
  secid: string;
  concentration: number;
  /** `distributed|moderate|concentrated|dominated`. */
  level: "distributed" | "moderate" | "concentrated" | "dominated";
  spike: boolean;
}

/** Пороги детекторов Mega Alerts (вход IPC) — зеркало `dto::MegaThresholdsInput`. */
export interface MegaThresholdsInput {
  volZ?: number;
  disb?: number;
  spread?: number;
  oiJump?: number;
  hi2?: number;
}

/** Тип Mega-сигнала — коды `domain::algo::mega_alerts::MegaAlertKind`. */
export type MegaAlertKind =
  | "volume_spike"
  | "buy_imbalance"
  | "sell_imbalance"
  | "spread_widening"
  | "oi_jump"
  | "concentration_rise";

/** Сработавший Mega-сигнал — зеркало `dto::MegaAlertDto`. */
export interface MegaAlertDto {
  secid: string;
  ts: number;
  kind: MegaAlertKind;
  value: number;
  message: string;
}
