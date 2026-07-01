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
  fallback: boolean;
}

export interface KeyActivityRuleDto {
  id: string;
  name: string;
  weight: number;
}
