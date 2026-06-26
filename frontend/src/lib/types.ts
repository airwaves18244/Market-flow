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
