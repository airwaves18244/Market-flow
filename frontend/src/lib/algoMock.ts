// Демо-генераторы для модулей вкладки «MOEX ALGO» (Фаза 10): супер-свечи,
// FUTOI, концентрация HI2, мега-алёрты.
//
// В боевом режиме эти ряды собираются из датасетов ALGOPACK (tradestats /
// futoi / hi2) через `data::moex`; аналитика уже реализована в `domain::algo`.
// Пока боевой транспорт не подключён, вкладка работает на детерминированных
// (сид-based) генераторах — воспроизводимые числа без сети, ровно как
// `mock.ts` обслуживает IPC-команды в браузерной сборке.

export interface AlgoTicker {
  ticker: string;
  name: string;
  sector: string;
  base: number;
  change: number;
  turnover: number;
}

export interface SuperBar {
  min: number;
  o: number;
  h: number;
  l: number;
  c: number;
  vol: number;
  val: number;
  vwap: number;
  disb: number;
  trades: number;
  anomalous: boolean;
}

export interface FutoiRow {
  time: string;
  group: "Физлица" | "Юрлица";
  long: number;
  short: number;
  net: number;
  sharePct: number;
  doi: number;
}

export interface FutoiSeries {
  times: string[];
  fizL: number[];
  fizS: number[];
  yurL: number[];
  yurS: number[];
}

export interface Hi2Row {
  ticker: string;
  hi2: number;
  level: "распределённая" | "умеренная" | "доминирование";
  spike: boolean;
}

export interface MegaAlert {
  time: string;
  ticker: string;
  type: string;
  typeId: string;
  metric: string;
  value: string;
  severity: "high" | "med" | "low";
  up: boolean;
}

// ── детерминированный PRNG (FNV-hash + mulberry32) ───────────────────────────
function hash(s: string): number {
  let h = 2166136261 >>> 0;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}
function rng(seed: number): () => number {
  let a = seed >>> 0;
  return function () {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
function hhmm(min: number): string {
  return (
    String(Math.floor(min / 60)).padStart(2, "0") +
    ":" +
    String(Math.round(min % 60)).padStart(2, "0")
  );
}

// ── вселенная инструментов (стабильная, сид 42) ──────────────────────────────
const UNIVERSE: Omit<AlgoTicker, "change" | "turnover">[] = [
  { ticker: "SBER", name: "Сбербанк", sector: "Финансы", base: 302.4 },
  { ticker: "GAZP", name: "Газпром", sector: "Нефтегаз", base: 128.6 },
  { ticker: "LKOH", name: "Лукойл", sector: "Нефтегаз", base: 7120 },
  { ticker: "GMKN", name: "ГМК Норникель", sector: "Металлургия", base: 112.8 },
  { ticker: "ROSN", name: "Роснефть", sector: "Нефтегаз", base: 565.2 },
  { ticker: "VTBR", name: "ВТБ", sector: "Финансы", base: 96.3 },
  { ticker: "YDEX", name: "Яндекс", sector: "IT", base: 4210 },
  { ticker: "MGNT", name: "Магнит", sector: "Ритейл", base: 6840 },
  { ticker: "NVTK", name: "Новатэк", sector: "Нефтегаз", base: 1042 },
  { ticker: "TATN", name: "Татнефть", sector: "Нефтегаз", base: 712.5 },
  { ticker: "MOEX", name: "Мосбиржа", sector: "Финансы", base: 198.4 },
  { ticker: "PLZL", name: "Полюс", sector: "Металлургия", base: 11840 },
];

export function tickers(): AlgoTicker[] {
  const r = rng(42);
  return UNIVERSE.map((t) => ({
    ...t,
    change: (r() - 0.45) * 0.06,
    turnover: (r() * 30 + 4) * 1e9,
  }));
}

export function tickerInfo(ticker: string): AlgoTicker {
  const all = tickers();
  return all.find((t) => t.ticker === ticker) ?? all[0];
}

// ── супер-свечи: 5-мин бары + VWAP + дисбаланс ──────────────────────────────
export function candles(ticker: string): SuperBar[] {
  const info = tickerInfo(ticker);
  const base = info.base;
  const r = rng(hash(ticker) + 3);
  let px = base;
  let cumPV = 0;
  let cumV = 0;
  const start = 10 * 60;
  const bars: SuperBar[] = [];
  for (let i = 0; i < 78; i++) {
    const drift = (r() - 0.48) * base * 0.0045;
    const o = px;
    const c = px + drift;
    const h = Math.max(o, c) + r() * base * 0.0022;
    const l = Math.min(o, c) - r() * base * 0.0022;
    const anomalous = i % 17 === 5 || i % 23 === 3;
    const vol = Math.round((1800 + r() * 6500) * (anomalous ? 3.4 : 1));
    const val = vol * ((o + c) / 2) * 10;
    cumPV += ((h + l + c) / 3) * vol;
    cumV += vol;
    const vwap = cumPV / cumV;
    const disb = Math.max(-1, Math.min(1, (r() - 0.5) * 2 + (drift * 4) / base * 100));
    bars.push({
      min: start + i * 5,
      o,
      h,
      l,
      c,
      vol,
      val,
      vwap,
      disb,
      trades: Math.round(vol / 8),
      anomalous,
    });
    px = c;
  }
  return bars;
}

// ── FUTOI: открытые позиции физ/юр ──────────────────────────────────────────
export function futoiSeries(ticker: string): FutoiSeries {
  const r = rng(hash(ticker) + 55);
  const times: string[] = [];
  const fizL: number[] = [];
  const fizS: number[] = [];
  const yurL: number[] = [];
  const yurS: number[] = [];
  for (let h = 0; h < 8; h++) {
    times.push(hhmm((10 + h) * 60));
    fizL.push(Math.round(120 + r() * 40));
    fizS.push(Math.round(150 + r() * 40));
    yurL.push(Math.round(180 + r() * 40));
    yurS.push(Math.round(140 + r() * 40));
  }
  return { times, fizL, fizS, yurL, yurS };
}

export function futoiTable(ticker: string): FutoiRow[] {
  const r = rng(hash(ticker) + 55);
  const rows: FutoiRow[] = [];
  for (let h = 0; h < 8; h++) {
    (["Физлица", "Юрлица"] as const).forEach((group) => {
      const fiz = group === "Физлица";
      const long = Math.round((fiz ? 120 : 180) + r() * 40) * 1000;
      const short = Math.round((fiz ? 150 : 140) + r() * 40) * 1000;
      const net = long - short;
      const sharePct = (long / (long + short)) * 100;
      const doi = Math.round((r() - 0.5) * 40) * 1000;
      rows.push({ time: hhmm((10 + h) * 60), group, long, short, net, sharePct, doi });
    });
  }
  return rows.reverse();
}

// ── HI2: временной ряд концентрации + ранжирование ──────────────────────────
export function hi2Timeline(ticker: string): { times: string[]; vals: number[] } {
  const r = rng(hash(ticker) + 88);
  const times: string[] = [];
  const vals: number[] = [];
  let x = 0.2;
  for (let i = 0; i < 48; i++) {
    x = Math.max(0.05, Math.min(0.5, x + (r() - 0.5) * 0.05));
    times.push(hhmm(10 * 60 + i * 10));
    vals.push(+x.toFixed(3));
  }
  return { times, vals };
}

export function hi2Ranking(): Hi2Row[] {
  const r = rng(77);
  return tickers()
    .map((t) => ({ ticker: t.ticker, hi2: r() * 0.35 + 0.12 }))
    .sort((a, b) => b.hi2 - a.hi2)
    .slice(0, 10)
    .map(({ ticker, hi2 }) => ({
      ticker,
      hi2,
      level: hi2 > 0.3 ? "доминирование" : hi2 > 0.18 ? "умеренная" : "распределённая",
      spike: hi2 > 0.3,
    }));
}

// ── мега-алёрты ─────────────────────────────────────────────────────────────
export function megaAlerts(): MegaAlert[] {
  const types: [string, string, MegaAlert["severity"]][] = [
    ["Всплеск объёма", "объём", "high"],
    ["Дисбаланс покупок", "disb", "high"],
    ["Расширение спреда", "спред", "med"],
    ["Скачок OI", "ΔOI", "med"],
    ["Концентрация HI2", "HI2", "low"],
  ];
  const tk = UNIVERSE.map((t) => t.ticker);
  const r = rng(19);
  const rows: MegaAlert[] = [];
  let m = 10 * 60 + 12;
  const fmt = (n: number, d = 1) =>
    n.toLocaleString("ru-RU", { minimumFractionDigits: d, maximumFractionDigits: d });
  const fmtInt = (n: number) => Math.round(n).toLocaleString("ru-RU");
  for (let i = 0; i < 18; i++) {
    const t = types[Math.floor(r() * types.length)];
    const ticker = tk[Math.floor(r() * tk.length)];
    const up = r() > 0.42;
    let value: string;
    if (t[1] === "объём") value = "×" + fmt(r() * 4 + 2.5);
    else if (t[1] === "disb") value = (up ? "+" : "−") + fmt(r() * 0.6 + 0.3, 2);
    else if (t[1] === "спред") value = "+" + fmt(r() * 3 + 1) + " бп";
    else if (t[1] === "ΔOI") value = (up ? "+" : "−") + fmtInt(r() * 60 + 15) + "k";
    else value = fmt(r() * 0.25 + 0.3, 2);
    m += Math.round(r() * 14 + 3);
    rows.push({
      time: hhmm(m),
      ticker,
      type: t[0],
      typeId: t[1],
      metric: t[0],
      value,
      severity: t[2],
      up,
    });
  }
  return rows.reverse();
}

// ── форматтеры ──────────────────────────────────────────────────────────────
export const fmt = (n: number, d = 2) =>
  Number(n).toLocaleString("ru-RU", { minimumFractionDigits: d, maximumFractionDigits: d });
export const fmtInt = (n: number) => Math.round(n).toLocaleString("ru-RU");
export { hhmm };
