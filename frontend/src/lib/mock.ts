// Мок-данные для запуска фронта в обычном браузере (без Tauri-бэкенда).
// Позволяют разрабатывать и собирать UI до интеграции с ядром.

import type {
  BarPoint,
  BreadthDto,
  InstrumentDto,
  RrgSectorDto,
  SectorEntryDto,
  SectorRow,
  TopMoverDto,
  TurnoverPoint,
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
    default:
      throw new Error(`mock: неизвестная команда ${cmd}`);
  }
}
