// Мок-данные для запуска фронта в обычном браузере (без Tauri-бэкенда).
// Позволяют разрабатывать и собирать UI до интеграции с ядром.

import type {
  BarPoint,
  BondIssuerDto,
  BreadthDto,
  CrossAssetSummaryDto,
  FlowEdgeDto,
  FutureGroupDto,
  InstrumentDto,
  OrderBookDto,
  ReplayStateDto,
  RrgSectorDto,
  SectorEntryDto,
  SectorRow,
  TapeEntryDto,
  TimeAndSalesDto,
  TopMoverDto,
  TriggeredAlertDto,
  TurnoverByClassPoint,
  TurnoverPoint,
  YieldCurvePoint,
} from "./types";

const instruments: InstrumentDto[] = [
  { symbol: "SBER@MISX", ticker: "SBER", name: "Сбербанк", assetClass: "equity", sector: "Финансы" },
  { symbol: "LKOH@MISX", ticker: "LKOH", name: "Лукойл", assetClass: "equity", sector: "Нефтегаз" },
  { symbol: "GAZP@MISX", ticker: "GAZP", name: "Газпром", assetClass: "equity", sector: "Нефтегаз" },
  { symbol: "GMKN@MISX", ticker: "GMKN", name: "Норникель", assetClass: "equity", sector: "Металлы" },
  { symbol: "YDEX@MISX", ticker: "YDEX", name: "Яндекс", assetClass: "equity", sector: "IT" },
];

const sectorRows: SectorRow[] = [
  { sector: "Нефтегаз", instruments: 2, turnover: 24_000_000, netFlow: 1_200_000, weightedChange: 0.018 },
  { sector: "Финансы", instruments: 1, turnover: 9_500_000, netFlow: -400_000, weightedChange: -0.009 },
  { sector: "Металлы", instruments: 1, turnover: 6_200_000, netFlow: 300_000, weightedChange: 0.012 },
  { sector: "IT", instruments: 1, turnover: 3_100_000, netFlow: 150_000, weightedChange: 0.025 },
];

const sectorEntries: SectorEntryDto[] = [
  { key: "SBER", sector: "Финансы", isIsin: false },
  { key: "LKOH", sector: "Нефтегаз", isIsin: false },
  { key: "GAZP", sector: "Нефтегаз", isIsin: false },
];

const breadth: BreadthDto = {
  advancers: 4,
  decliners: 1,
  unchanged: 0,
  pctAdvancing: 0.8,
  adRatio: 4.0,
};

const topMovers: TopMoverDto[] = [
  { symbol: "YDEX@MISX", ticker: "YDEX", name: "Яндекс", sector: "IT", change: 0.045, lastClose: 3200 },
  { symbol: "GAZP@MISX", ticker: "GAZP", name: "Газпром", sector: "Нефтегаз", change: 0.032, lastClose: 185 },
  { symbol: "GMKN@MISX", ticker: "GMKN", name: "Норникель", sector: "Металлы", change: 0.025, lastClose: 175000 },
  { symbol: "LKOH@MISX", ticker: "LKOH", name: "Лукойл", sector: "Нефтегаз", change: 0.018, lastClose: 6850 },
];

const rrgSectors: RrgSectorDto[] = [
  { sector: "Нефтегаз", rsRatio: 126, rsMomentum: 115, quadrant: "leading" },
  { sector: "Финансы", rsRatio: 95, rsMomentum: 92, quadrant: "lagging" },
  { sector: "Металлы", rsRatio: 118, rsMomentum: 98, quadrant: "weakening" },
  { sector: "IT", rsRatio: 105, rsMomentum: 125, quadrant: "improving" },
];

const futureGroups: FutureGroupDto[] = [
  { group: "Si", contracts: 12, turnover: 45_000_000, netFlow: 2_000_000, weightedChange: 0.012, openInterest: 280000 },
  { group: "Ri", contracts: 8, turnover: 28_000_000, netFlow: 1_200_000, weightedChange: 0.018, openInterest: 150000 },
  { group: "ED", contracts: 5, turnover: 15_000_000, netFlow: 500_000, weightedChange: -0.005, openInterest: 85000 },
  { group: "Gd", contracts: 4, turnover: 12_000_000, netFlow: -300_000, weightedChange: -0.008, openInterest: 55000 },
];

const bondIssuers: BondIssuerDto[] = [
  { issuer: "OFZ", bonds: 18, turnover: 8_500_000, netFlow: 200_000, avgYield: 5.2, weightedDuration: 3.5 },
  { issuer: "Gaz", bonds: 12, turnover: 6_200_000, netFlow: -100_000, avgYield: 5.8, weightedDuration: 2.8 },
  { issuer: "Luk", bonds: 9, turnover: 4_100_000, netFlow: 150_000, avgYield: 6.1, weightedDuration: 4.2 },
  { issuer: "Ber", bonds: 6, turnover: 2_800_000, netFlow: 50_000, avgYield: 6.5, weightedDuration: 5.1 },
];

const yields: YieldCurvePoint[] = [
  { maturityYears: 0.25, yieldPct: 4.5 },
  { maturityYears: 0.5, yieldPct: 4.7 },
  { maturityYears: 1.0, yieldPct: 5.1 },
  { maturityYears: 2.0, yieldPct: 5.6 },
  { maturityYears: 3.0, yieldPct: 5.9 },
  { maturityYears: 5.0, yieldPct: 6.2 },
  { maturityYears: 7.0, yieldPct: 6.4 },
  { maturityYears: 10.0, yieldPct: 6.5 },
];

const crossAssetSummary: CrossAssetSummaryDto = {
  total: 142_800_000,
  shares: [
    { assetClass: "equity", turnover: 42_800_000, share: 0.3 },
    { assetClass: "future", turnover: 85_700_000, share: 0.6 },
    { assetClass: "bond", turnover: 14_300_000, share: 0.1 },
  ],
};

function genTimeline(): TurnoverByClassPoint[] {
  const out: TurnoverByClassPoint[] = [];
  const start = Math.floor(Date.UTC(2026, 0, 1) / 1000);
  const day = 86_400;
  for (let i = 0; i < 60; i++) {
    // Доля фьючерсов растёт со временем, акций — снижается (виден переток).
    const t = i / 60;
    out.push({
      ts: start + i * day,
      equity: 50_000_000 * (1 - 0.4 * t) + Math.random() * 4_000_000,
      future: 60_000_000 * (1 + 0.5 * t) + Math.random() * 4_000_000,
      bond: 14_000_000 + Math.random() * 2_000_000,
    });
  }
  return out;
}

const flowSankey: FlowEdgeDto[] = [
  { from: "equity", to: "future", weight: 0.12 },
  { from: "bond", to: "future", weight: 0.03 },
];

function genBars(seed = 300): BarPoint[] {
  const out: BarPoint[] = [];
  let price = seed;
  const start = Math.floor(Date.UTC(2026, 0, 1) / 1000);
  const day = 86_400;
  for (let i = 0; i < 90; i++) {
    const open = price;
    const drift = Math.sin(i / 6) * seed * 0.01;
    const close = Math.max(1, open + drift + (Math.random() - 0.5) * seed * 0.01);
    const high = Math.max(open, close) * (1 + Math.random() * 0.006);
    const low = Math.min(open, close) * (1 - Math.random() * 0.006);
    out.push({ ts: start + i * day, open, high, low, close, volume: 800 + Math.random() * 1200 });
    price = close;
  }
  return out;
}

function genTurnover(): TurnoverPoint[] {
  const out: TurnoverPoint[] = [];
  const start = Math.floor(Date.UTC(2026, 0, 1) / 1000);
  const day = 86_400;
  for (let i = 0; i < 90; i++) {
    out.push({
      ts: start + i * day,
      turnover: 8_000_000 + Math.random() * 4_000_000,
      netFlow: (Math.random() - 0.5) * 2_000_000,
      change: (Math.random() - 0.5) * 0.04,
    });
  }
  return out;
}

// ── Фаза 7 — live-функции (детерминированные генераторы, как scaffold в Rust) ─

// Якорная цена по символу — та же логика, что у genBars.
function anchorFor(symbol: string): number {
  if (symbol.startsWith("LKOH")) return 7000;
  if (symbol.startsWith("GAZP")) return 160;
  if (symbol.startsWith("Si")) return 90000;
  return 300;
}

function genOrderBook(symbol: string, depth: number): OrderBookDto {
  const anchor = anchorFor(symbol);
  const tick = Math.max(anchor * 0.001, 0.01);
  const n = Math.min(Math.max(depth, 1), 25);
  const bids = [];
  const asks = [];
  let bidCum = 0;
  let askCum = 0;
  let bidVol = 0;
  let askVol = 0;
  for (let i = 1; i <= n; i++) {
    const size = (n - i + 1) * 10;
    bidCum += size;
    bidVol += size;
    const askSize = size * 0.9;
    askCum += askSize;
    askVol += askSize;
    bids.push({ price: anchor - tick * i, size, cumulative: bidCum });
    asks.push({ price: anchor + tick * i, size: askSize, cumulative: askCum });
  }
  return {
    symbol,
    bids,
    asks,
    mid: anchor,
    spread: tick * 2,
    imbalance: (bidVol - askVol) / (bidVol + askVol),
  };
}

function genTimeAndSales(symbol: string, limit: number): TimeAndSalesDto {
  const anchor = anchorFor(symbol);
  const tick = Math.max(anchor * 0.0005, 0.01);
  const n = Math.min(Math.max(limit, 1), 200);
  const entries: TapeEntryDto[] = [];
  let buyVolume = 0;
  let sellVolume = 0;
  let turnover = 0;
  let prevPrice: number | null = null;
  let prevSide: "buy" | "sell" = "buy";
  for (let i = 0; i < n; i++) {
    const price = anchor + tick * ((i % 7) - 3);
    const size = 1 + (i % 5);
    const side: "buy" | "sell" =
      prevPrice === null ? "buy" : price > prevPrice ? "buy" : price < prevPrice ? "sell" : prevSide;
    entries.push({ ts: i, price, size, side });
    if (side === "buy") buyVolume += size;
    else sellVolume += size;
    turnover += price * size;
    prevPrice = price;
    prevSide = side;
  }
  const volume = buyVolume + sellVolume;
  return {
    symbol,
    entries,
    stats: {
      trades: n,
      buyVolume,
      sellVolume,
      cvd: buyVolume - sellVolume,
      vwap: volume > 0 ? turnover / volume : null,
      lastPrice: entries.length ? entries[entries.length - 1].price : null,
    },
  };
}

const triggeredAlerts: TriggeredAlertDto[] = [
  { ruleId: "YDEX@MISX:move", symbol: "YDEX@MISX", message: "изменение +4.50% (порог 1.00%)", severity: "info" },
  { ruleId: "GAZP@MISX:vol", symbol: "GAZP@MISX", message: "объём 5200 ≥ 1.5× среднего (1800)", severity: "critical" },
  { ruleId: "SBER@MISX:price", symbol: "SBER@MISX", message: "цена 305.00 ≥ 300.00", severity: "warning" },
];

function genReplayState(played: number): ReplayStateDto {
  const frames = 90;
  const pos = Math.min(Math.max(played, 0), frames);
  const start = Math.floor(Date.UTC(2026, 0, 1) / 1000);
  return {
    frames,
    pos,
    currentTs: pos === 0 ? null : start + (pos - 1) * 86_400,
    progress: pos / frames,
    atEnd: pos >= frames,
  };
}

export async function handle<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  switch (cmd) {
    case "instruments":
      return instruments as unknown as T;
    case "sector_rollup":
      return sectorRows as unknown as T;
    case "sector_map":
      return sectorEntries as unknown as T;
    case "bars": {
      const sym = String(args?.symbol ?? "SBER@MISX");
      const seed = sym.startsWith("LKOH") ? 7000 : sym.startsWith("GAZP") ? 160 : 300;
      return genBars(seed) as unknown as T;
    }
    case "turnover_series":
      return genTurnover() as unknown as T;
    case "breadth_data":
      return breadth as unknown as T;
    case "top_movers":
      return topMovers as unknown as T;
    case "rrg_sectors":
      return rrgSectors as unknown as T;
    case "futures_rollup":
      return futureGroups as unknown as T;
    case "bonds_rollup":
      return bondIssuers as unknown as T;
    case "yield_curve":
      return yields as unknown as T;
    case "cross_asset_summary":
      return crossAssetSummary as unknown as T;
    case "turnover_timeline":
      return genTimeline() as unknown as T;
    case "flow_sankey":
      return flowSankey as unknown as T;
    case "order_book": {
      const sym = String(args?.symbol ?? "SBER@MISX");
      const depth = Number(args?.depth ?? 10);
      return genOrderBook(sym, depth) as unknown as T;
    }
    case "time_and_sales": {
      const sym = String(args?.symbol ?? "SBER@MISX");
      const limit = Number(args?.limit ?? 50);
      return genTimeAndSales(sym, limit) as unknown as T;
    }
    case "active_alerts":
      return triggeredAlerts as unknown as T;
    case "replay_state": {
      const played = Number(args?.played ?? 0);
      return genReplayState(played) as unknown as T;
    }
    default:
      throw new Error(`mock: неизвестная команда ${cmd}`);
  }
}
