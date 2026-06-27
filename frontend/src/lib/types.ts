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
