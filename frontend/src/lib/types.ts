// Зеркало DTO из Rust-ядра (crates/app/src/dto.rs). Поля — camelCase,
// как сериализует serde с `rename_all = "camelCase"`.

export type AssetClass = "equity" | "future" | "bond" | "fx";
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
  fx: number;
}

export interface FlowEdgeDto {
  from: string;
  to: string;
  weight: number;
}

// ── Вкладка «Сводка» (Summary) — режим рынка по кросс-актив потокам ─────────

export type Regime = "riskOn" | "riskOff" | "neutral";

/** Направленный нетто-поток одного класса активов (₽млрд, знаковый). */
export interface ClassFlowDto {
  assetClass: string;
  /** > 0 — приток, < 0 — отток. */
  netFlow: number;
}

/** Сигнал режима рынка: режим + уверенность (0..100) + потоки по классам. */
export interface RegimeSignalDto {
  regime: Regime;
  conviction: number;
  classFlows: ClassFlowDto[];
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
