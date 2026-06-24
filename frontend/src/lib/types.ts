// DTO фронта. Формы совпадают с сериализуемыми типами Rust-ядра
// (`crates/app/src/api.rs` и `crates/storage/src/query.rs`).

/** Оборот и нетто-поток по сектору за период. */
export interface SectorTurnover {
  sector: string;
  turnover: number;
  net_flow: number;
}

/** Запись топ-движения за период. */
export interface Mover {
  symbol: string;
  name: string;
  /** Изменение из последнего снимка периода, доли (0.012 = +1.2%). */
  change: number;
  turnover: number;
}

/** Точка временного ряда нетто-потока. `ts` — UNIX-секунды UTC. */
export interface FlowPoint {
  ts: number;
  net_flow: number;
}

/** Снимок представления «Акции / секторы». */
export interface EquityDashboard {
  from_ts: number;
  to_ts: number;
  sectors: SectorTurnover[];
  top_movers: Mover[];
}

/** Ширина рынка (A/D) за период. */
export interface Breadth {
  advancers: number;
  decliners: number;
  unchanged: number;
  /** Доля растущих от общего числа (0..1). */
  pct_advancing: number;
}

/** Точка RRG: относительная сила сектора и её импульс (центр = 100). */
export interface RrgPoint {
  sector: string;
  rs_ratio: number;
  rs_momentum: number;
}
