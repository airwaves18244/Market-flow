// Мок-данные для запуска фронта в обычном браузере (без Tauri-бэкенда).
// Позволяют разрабатывать и собирать UI до интеграции с ядром.

import type {
  AccountDto,
  AlertEventDto,
  AlertRuleInput,
  BacktestReportDto,
  BarPoint,
  BondIssuerDto,
  BreadthDto,
  CrossAssetSummaryDto,
  DatasetMetaDto,
  FlowEdgeDto,
  FootprintBarDto,
  FutoiDto,
  FutureGroupDto,
  Hi2Dto,
  ImpliedVolDto,
  ImpliedVolInput,
  InstrumentDto,
  KeyActivityRowDto,
  KeyActivityRuleDto,
  KeyActivitySampleInput,
  KeyActivitySummaryDto,
  MegaAlertDto,
  MegaAlertKind,
  OptionBoardDto,
  OptionBoardInput,
  OptionKind,
  OptionPriceDto,
  OptionPriceInput,
  OptionQuoteDto,
  OrderBookDto,
  OrderDto,
  OrderInput,
  PositionDto,
  RobotSignalDto,
  RrgSectorDto,
  SectorEntryDto,
  SectorRow,
  SettingsDto,
  SmileFitDto,
  SmileFitInput,
  SmileModelDto,
  SmilePointInput,
  StrategyDescriptorDto,
  StrategyEvalDto,
  StrategyEvalInput,
  SubmitResultDto,
  TimeRangeDto,
  TopMoverDto,
  TradeDto,
  TradestatsDto,
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

// ── Фаза 7 — live-панели (Time&Sales / DOM / алёрты) ──────────────────────

// Опорная цена символа для лент сделок и стакана.
function basePrice(symbol: string): number {
  if (symbol.startsWith("LKOH")) return 6850;
  if (symbol.startsWith("GAZP")) return 160;
  if (symbol.startsWith("GMKN")) return 175_000;
  if (symbol.startsWith("YDEX")) return 3200;
  return 305; // SBER и прочее
}

// Лента обезличенных сделок (самые свежие — первыми).
function genTrades(symbol: string, limit: number): TradeDto[] {
  const out: TradeDto[] = [];
  const base = basePrice(symbol);
  const now = Math.floor(Date.now() / 1000);
  let price = base;
  for (let i = 0; i < limit; i++) {
    price = Math.max(1, price + (Math.random() - 0.5) * base * 0.001);
    out.push({
      ts: now - i,
      price: Number(price.toFixed(2)),
      size: Math.ceil(Math.random() * 50),
      buyerInitiated: Math.random() > 0.5,
    });
  }
  return out;
}

// Снимок стакана: симметричная лесенка вокруг середины.
function genOrderBook(symbol: string, depth: number): OrderBookDto {
  const base = basePrice(symbol);
  const tick = Math.max(0.01, base * 0.0005);
  const bids = [];
  const asks = [];
  for (let i = 0; i < depth; i++) {
    bids.push({
      price: Number((base - tick * (i + 1)).toFixed(2)),
      size: Math.ceil((depth - i) * (5 + Math.random() * 20)),
    });
    asks.push({
      price: Number((base + tick * (i + 1)).toFixed(2)),
      size: Math.ceil((depth - i) * (5 + Math.random() * 20)),
    });
  }
  return { ts: Math.floor(Date.now() / 1000), bids, asks };
}

// Прогон правил по мок-барам (edge-triggered, как в доменном движке).
function scanAlerts(rules: AlertRuleInput[]): AlertEventDto[] {
  const out: AlertEventDto[] = [];
  for (const rule of rules) {
    const seed = basePrice(rule.symbol);
    const bars = genBars(seed);
    let active = false;
    for (const b of bars) {
      const change = b.open !== 0 ? (b.close - b.open) / b.open : 0;
      let holds = false;
      let label = "";
      switch (rule.kind) {
        case "priceAbove":
          holds = b.close > rule.threshold;
          label = `цена выше ${rule.threshold}`;
          break;
        case "priceBelow":
          holds = b.close < rule.threshold;
          label = `цена ниже ${rule.threshold}`;
          break;
        case "changeAbove":
          holds = change > rule.threshold;
          label = `изменение выше ${(rule.threshold * 100).toFixed(2)}%`;
          break;
        case "changeBelow":
          holds = change < rule.threshold;
          label = `изменение ниже ${(rule.threshold * 100).toFixed(2)}%`;
          break;
      }
      if (holds && !active) {
        active = true;
        out.push({ symbol: rule.symbol, ts: b.ts, price: b.close, change, message: label });
      } else if (!holds) {
        active = false;
      }
    }
  }
  out.sort((a, b) => a.ts - b.ts);
  return out;
}

// ── V2 / Бэктестер (мок) ───────────────────────────────────────────────────

const strategyDescriptors: StrategyDescriptorDto[] = [
  {
    id: "ma_cross",
    label: "Пересечение скользящих (MA cross)",
    params: [
      { name: "fast", label: "Быстрая MA", default: 5 },
      { name: "slow", label: "Медленная MA", default: 20 },
      { name: "lot", label: "Лот", default: 1 },
    ],
  },
  {
    id: "same_lot",
    label: "Равные лоты (пробой)",
    params: [
      { name: "lot", label: "Лот", default: 1 },
      { name: "lookback", label: "Окно пробоя", default: 10 },
    ],
  },
  {
    id: "iceberg",
    label: "Айсберг (набор равными клипами)",
    params: [
      { name: "clip", label: "Клип", default: 1 },
      { name: "clips", label: "Число клипов", default: 5 },
      { name: "period", label: "Период тренда", default: 20 },
    ],
  },
  {
    id: "cvd_momentum",
    label: "Импульс дельты объёма (CVD)",
    params: [
      { name: "lot", label: "Лот", default: 1 },
      { name: "period", label: "Окно дельты", default: 14 },
    ],
  },
];

// Простой бэктест по мок-барам: лонг при close>prev, иначе вне рынка; исполнение
// по закрытию; считаем кривую капитала, сделки и метрики (как в Rust-отчёте).
function mockBacktest(symbol: string, initialCapital: number): BacktestReportDto {
  const bars = genBars(basePrice(symbol));
  let cash = initialCapital;
  let pos = 0;
  let avg = 0;
  const trades: BacktestReportDto["trades"] = [];
  const equityCurve: BacktestReportDto["equityCurve"] = [];
  let prev = bars[0]?.close ?? 0;

  const fill = (side: "buy" | "sell", qty: number, price: number, ts: number) => {
    const signed = side === "buy" ? qty : -qty;
    let realized = 0;
    if (pos === 0 || Math.sign(pos) === Math.sign(signed)) {
      const np = pos + signed;
      avg = (avg * Math.abs(pos) + price * qty) / Math.abs(np);
      pos = np;
    } else {
      const closing = Math.min(qty, Math.abs(pos));
      realized = closing * (price - avg) * Math.sign(pos);
      pos += signed;
      if (pos !== 0 && Math.sign(pos) === Math.sign(signed)) avg = price;
    }
    cash -= signed * price;
    trades.push({ ts, side, qty, price, realizedPnl: realized });
  };

  for (let i = 1; i < bars.length; i++) {
    const b = bars[i];
    const target = b.close > prev ? 1 : 0;
    const delta = target - pos;
    if (Math.abs(delta) > 1e-9) fill(delta > 0 ? "buy" : "sell", Math.abs(delta), b.close, b.ts);
    equityCurve.push({ ts: b.ts, equity: cash + pos * b.close });
    prev = b.close;
  }

  let grossWin = 0;
  let grossLoss = 0;
  let wins = 0;
  let losses = 0;
  for (const t of trades) {
    if (t.realizedPnl > 0) {
      wins++;
      grossWin += t.realizedPnl;
    } else if (t.realizedPnl < 0) {
      losses++;
      grossLoss += -t.realizedPnl;
    }
  }
  const finalEq = equityCurve.at(-1)?.equity ?? initialCapital;
  const netPnl = finalEq - initialCapital;
  let peak = -Infinity;
  let maxDd = 0;
  for (const p of equityCurve) {
    if (p.equity > peak) peak = p.equity;
    if (peak > 0) maxDd = Math.max(maxDd, (peak - p.equity) / peak);
  }
  return {
    trades,
    equityCurve,
    metrics: {
      netPnl,
      returnPct: initialCapital ? netPnl / initialCapital : 0,
      trades: trades.length,
      wins,
      losses,
      winRate: wins + losses > 0 ? wins / (wins + losses) : 0,
      profitFactor: grossLoss > 0 ? grossWin / grossLoss : grossWin > 0 ? Infinity : 0,
      maxDrawdown: maxDd,
      sharpe: 0,
      avgWin: wins ? grossWin / wins : 0,
      avgLoss: losses ? grossLoss / losses : 0,
    },
  };
}

// ── V2 / Delta (мок) ───────────────────────────────────────────────────────

function mockFootprint(symbol: string): FootprintBarDto[] {
  const bars = genBars(basePrice(symbol));
  const tick = Math.max(0.01, basePrice(symbol) * 0.0005);
  let cum = 0;
  return bars.map((b) => {
    const cells = [];
    let bidTotal = 0;
    let askTotal = 0;
    const mid = Math.round(b.close / tick) * tick;
    for (let k = -2; k <= 2; k++) {
      const ask = Math.ceil(Math.random() * 40);
      const bid = Math.ceil(Math.random() * 40);
      bidTotal += bid;
      askTotal += ask;
      cells.push({
        price: Number((mid + k * tick).toFixed(2)),
        bidVolume: bid,
        askVolume: ask,
        delta: ask - bid,
      });
    }
    const delta = askTotal - bidTotal;
    cum += delta;
    return { ts: b.ts, cells, bidTotal, askTotal, delta, cumulativeDelta: cum };
  });
}

function mockRobotSignals(symbol: string): RobotSignalDto[] {
  const bars = genBars(basePrice(symbol));
  const out: RobotSignalDto[] = [];
  const kinds: RobotSignalDto["kind"][] = ["same_lot", "iceberg", "absorption"];
  for (let i = 8; i < bars.length; i += 17) {
    const kind = kinds[i % kinds.length];
    out.push({
      kind,
      ts: bars[i].ts,
      price: Number(bars[i].close.toFixed(2)),
      strength: 3 + (i % 5),
      note:
        kind === "same_lot"
          ? "серия равных лотов"
          : kind === "iceberg"
            ? "доливка айсберга"
            : "поглощение дельты",
    });
  }
  return out;
}

// ── V2 / Trade (мок-симулятор) ─────────────────────────────────────────────

interface MockSim {
  cash: number;
  realizedPnl: number;
  positions: Map<string, { qty: number; avg: number }>;
  orders: OrderDto[];
  nextId: number;
}
const sim: MockSim = {
  cash: 1_000_000,
  realizedPnl: 0,
  positions: new Map(),
  orders: [],
  nextId: 0,
};

function simApplyFill(symbol: string, side: "buy" | "sell", qty: number, price: number): number {
  const p = sim.positions.get(symbol) ?? { qty: 0, avg: 0 };
  const signed = side === "buy" ? qty : -qty;
  let realized = 0;
  if (p.qty === 0 || Math.sign(p.qty) === Math.sign(signed)) {
    const np = p.qty + signed;
    p.avg = (p.avg * Math.abs(p.qty) + price * qty) / Math.abs(np);
    p.qty = np;
  } else {
    const closing = Math.min(qty, Math.abs(p.qty));
    realized = closing * (price - p.avg) * Math.sign(p.qty);
    p.qty += signed;
    if (p.qty !== 0 && Math.sign(p.qty) === Math.sign(signed)) p.avg = price;
  }
  sim.cash -= signed * price;
  sim.realizedPnl += realized;
  sim.positions.set(symbol, p);
  return realized;
}

function mockSubmit(input: OrderInput): SubmitResultDto {
  sim.nextId += 1;
  const id = sim.nextId;
  const side = input.side;
  const base = basePrice(input.symbol);
  const order: OrderDto = {
    id,
    symbol: input.symbol,
    side,
    qty: input.qty,
    filled: 0,
    price: input.price ?? null,
    kind: input.kind,
    status: "new",
  };
  const fills = [];
  if (input.kind === "market") {
    // Рынок: исполняем сразу около опорной цены (имитация прохода стакана).
    const px = side === "buy" ? base * 1.0005 : base * 0.9995;
    const realized = simApplyFill(input.symbol, side, input.qty, px);
    order.filled = input.qty;
    order.status = "filled";
    fills.push({
      orderId: id,
      ts: Math.floor(Date.now() / 1000),
      side,
      qty: input.qty,
      price: Number(px.toFixed(2)),
      realizedPnl: realized,
    });
  } else {
    // Лимит/стоп: встают в блоттер.
    sim.orders.push(order);
  }
  return { order, fills };
}

// ── Фаза 12 — Опционы (компактный Блэк-76 для браузерного мок-режима) ─────────
// Зеркалит доменную математику (`domain::options`) достаточно, чтобы UI
// выглядел «как настоящий» без бэкенда. Точные значения даёт Rust-ядро в Tauri.

function erf(x: number): number {
  const t = 1 / (1 + 0.3275911 * Math.abs(x));
  const y =
    1 -
    ((((1.061405429 * t - 1.453152027) * t + 1.421413741) * t - 0.284496736) * t + 0.254829592) *
      t *
      Math.exp(-x * x);
  return x >= 0 ? y : -y;
}
const normCdf = (x: number) => 0.5 * (1 + erf(x / Math.SQRT2));
const normPdf = (x: number) => Math.exp(-0.5 * x * x) / Math.sqrt(2 * Math.PI);

function black76(
  forward: number,
  strike: number,
  t: number,
  vol: number,
  kind: "call" | "put",
  rate = 0,
): { price: number; delta: number; gamma: number; vega: number; theta: number; rho: number } {
  const df = Math.exp(-rate * t);
  const sign = kind === "call" ? 1 : -1;
  if (t <= 0 || vol <= 0) {
    const intrinsic = df * Math.max(sign * (forward - strike), 0);
    return { price: intrinsic, delta: 0, gamma: 0, vega: 0, theta: 0, rho: 0 };
  }
  const sqrtT = Math.sqrt(t);
  const d1 = (Math.log(forward / strike) + 0.5 * vol * vol * t) / (vol * sqrtT);
  const d2 = d1 - vol * sqrtT;
  const price = df * sign * (forward * normCdf(sign * d1) - strike * normCdf(sign * d2));
  const delta = df * sign * normCdf(sign * d1);
  const gamma = (df * normPdf(d1)) / (forward * vol * sqrtT);
  const vega = df * forward * normPdf(d1) * sqrtT;
  const theta =
    -(df * forward * normPdf(d1) * vol) / (2 * sqrtT) - rate * price * (rate === 0 ? 0 : 1);
  const rho = -t * price;
  return { price, delta, gamma, vega, theta, rho };
}

function mockOptionPrice(input: OptionPriceInput): OptionPriceDto {
  const g = black76(
    input.forward,
    input.strike,
    input.t,
    input.vol,
    input.kind,
    input.rate ?? 0,
  );
  return {
    price: g.price,
    greeks: { delta: g.delta, gamma: g.gamma, vega: g.vega, theta: g.theta, rho: g.rho },
  };
}

function mockImpliedVol(input: ImpliedVolInput): ImpliedVolDto {
  const df = Math.exp(-(input.rate ?? 0) * input.t);
  const sign = input.kind === "call" ? 1 : -1;
  const intrinsic = df * Math.max(sign * (input.forward - input.strike), 0);
  if (input.t <= 0 || input.marketPrice < intrinsic - 1e-9) return { iv: null };
  // Бисекция по волатильности.
  let lo = 1e-4;
  let hi = 5;
  for (let i = 0; i < 100; i++) {
    const mid = 0.5 * (lo + hi);
    const p = black76(input.forward, input.strike, input.t, mid, input.kind, input.rate ?? 0).price;
    if (p > input.marketPrice) hi = mid;
    else lo = mid;
  }
  return { iv: 0.5 * (lo + hi) };
}

function mockSmileFit(input: SmileFitInput): SmileFitDto {
  const pts = input.points;
  const f = input.forward;
  // Квадратичная подгонка IV по лог-моней­ности (наглядная «улыбка»).
  const xs = pts.map((p) => Math.log(p.strike / f));
  const ys = pts.map((p) => p.iv);
  const n = xs.length || 1;
  const mean = (a: number[]) => a.reduce((s, v) => s + v, 0) / (a.length || 1);
  const mx = mean(xs);
  const my = mean(ys);
  let sxx = 0;
  let sxy = 0;
  let sx2y = 0;
  let sx2x2 = 0;
  let sx2 = 0;
  for (let i = 0; i < xs.length; i++) {
    const dx = xs[i] - mx;
    sxx += dx * dx;
    sxy += dx * (ys[i] - my);
    const x2 = xs[i] * xs[i];
    sx2 += x2;
    sx2y += x2 * ys[i];
    sx2x2 += x2 * x2;
  }
  const skew = sxx > 0 ? sxy / sxx : 0;
  const curv = sx2x2 > 0 ? Math.max(0, (sx2y - (sx2 / n) * my * n) / sx2x2) : 0.2;
  const s0 = my;
  const ivAt = (strike: number) => {
    const k = Math.log(strike / f);
    return Math.max(0.01, s0 + skew * k + curv * k * k);
  };
  const lo = input.curveLo ?? Math.min(...pts.map((p) => p.strike));
  const hi = input.curveHi ?? Math.max(...pts.map((p) => p.strike));
  const steps = input.curveSteps ?? 41;
  const curve = Array.from({ length: steps }, (_, i) => {
    const strike = lo + ((hi - lo) * i) / (steps - 1);
    return { strike, iv: ivAt(strike) };
  });
  const rmse = Math.sqrt(mean(pts.map((p) => (ivAt(p.strike) - p.iv) ** 2)));
  const params =
    input.model === "svi"
      ? [
          { name: "a", value: s0 * s0 * input.t },
          { name: "b", value: curv },
          { name: "rho", value: Math.max(-0.99, Math.min(0.99, skew)) },
          { name: "m", value: mx },
          { name: "sigma", value: 0.1 },
        ]
      : [
          { name: "s0", value: s0 },
          { name: "skew", value: skew },
          { name: "curv", value: curv },
        ];
  return { model: input.model, params, rmse, curve };
}

function mockStrategyEval(input: StrategyEvalInput): StrategyEvalDto {
  const rate = input.rate ?? 0;
  const legSign = (side: string) => (side === "long" ? 1 : -1);
  const intrinsic = (kind: string, strike: number, spot: number) =>
    kind === "call" ? Math.max(spot - strike, 0) : kind === "put" ? Math.max(strike - spot, 0) : spot;
  const payoffAt = (spot: number) =>
    input.legs.reduce(
      (s, l) => s + legSign(l.side) * l.quantity * (intrinsic(l.kind, l.strike, spot) - l.entryPrice),
      0,
    );
  const markAt = (spot: number) =>
    input.legs.reduce((s, l) => {
      const val =
        l.kind === "underlying"
          ? spot
          : black76(spot, l.strike, l.expiryT, input.vol, l.kind as "call" | "put", rate).price;
      return s + legSign(l.side) * l.quantity * (val - l.entryPrice);
    }, 0);
  const steps = input.steps ?? 61;
  const payoff = Array.from({ length: steps }, (_, i) => {
    const price = input.priceLo + ((input.priceHi - input.priceLo) * i) / (steps - 1);
    return { price, pnlExpiry: payoffAt(price), pnlNow: markAt(price) };
  });
  // Безубытки — смены знака payoff на экспирацию.
  const breakevens: number[] = [];
  for (let i = 1; i < payoff.length; i++) {
    const a = payoff[i - 1];
    const b = payoff[i];
    if (a.pnlExpiry === 0) breakevens.push(a.price);
    else if (a.pnlExpiry * b.pnlExpiry < 0) {
      const w = a.pnlExpiry / (a.pnlExpiry - b.pnlExpiry);
      breakevens.push(a.price + w * (b.price - a.price));
    }
  }
  const pnls = payoff.map((p) => p.pnlExpiry);
  const netCost = input.legs.reduce((s, l) => s + legSign(l.side) * l.quantity * l.entryPrice, 0);
  const g = input.legs.reduce(
    (acc, l) => {
      if (l.kind === "underlying") {
        acc.delta += legSign(l.side) * l.quantity;
        return acc;
      }
      const bg = black76(input.forward, l.strike, l.expiryT, input.vol, l.kind as "call" | "put", rate);
      const s = legSign(l.side) * l.quantity;
      acc.delta += s * bg.delta;
      acc.gamma += s * bg.gamma;
      acc.vega += s * bg.vega;
      acc.theta += s * bg.theta;
      acc.rho += s * bg.rho;
      return acc;
    },
    { delta: 0, gamma: 0, vega: 0, theta: 0, rho: 0 },
  );
  return {
    breakevens,
    maxProfit: Math.max(...pnls),
    maxLoss: Math.min(...pnls),
    netCost,
    payoff,
    greeks: g,
  };
}

const smileModels: SmileModelDto[] = [
  { id: "moex", name: "MOEX (параметрическая)" },
  { id: "sabr", name: "SABR (Hagan)" },
  { id: "svi", name: "SVI (Gatheral)" },
  { id: "kalenkovich", name: "Каленкович" },
];

// ── Фаза 12.4 — Опционная доска MOEX (детерминированная синтетика) ────────────
// Зеркалит контракт `option_board` Rust-ядра: котировки одной серии вокруг
// форварда + готовые точки улыбки. Без Math.random — тесты и UI стабильны.

function mockOptionBoard(input: OptionBoardInput): OptionBoardDto {
  const forward = input.forwardHint ?? basePrice(input.underlying);
  const expirationTs =
    input.expirationTs ?? Math.floor(Date.UTC(2026, 2, 20) / 1000); // фикс. серия
  const t = input.t > 0 ? input.t : 30 / 365;
  const quotes: OptionQuoteDto[] = [];
  const smilePoints: SmilePointInput[] = [];
  // Лог-моней­ности страйков и параметры демо-улыбки (skew + крылья).
  const ks = [-0.15, -0.1, -0.05, 0, 0.05, 0.1, 0.15];
  for (const k of ks) {
    const strike = Number((forward * Math.exp(k)).toFixed(2));
    const iv = Math.max(0.05, 0.3 - 0.15 * k + 0.9 * k * k);
    const kind: OptionKind = k < 0 ? "put" : "call";
    const theor = black76(forward, strike, t, iv, kind).price;
    // OI-колокол вокруг ATM — вес точки для калибратора.
    const oi = Math.round(1200 * Math.exp(-((k / 0.09) ** 2)));
    quotes.push({
      secid: `${input.underlying}-${strike}${kind === "call" ? "C" : "P"}`,
      underlying: input.underlying,
      expirationTs,
      strike,
      kind,
      bid: Number((theor * 0.98).toFixed(4)),
      ask: Number((theor * 1.02).toFixed(4)),
      last: Number(theor.toFixed(4)),
      iv,
      oi,
      theorPrice: Number(theor.toFixed(4)),
    });
    smilePoints.push({ strike, iv, weight: oi });
  }
  // Неликвидная строка: присутствует в котировках, но не в точках улыбки —
  // как и в Rust-маппинге (`board_to_smile_points` отбрасывает без bid/ask/OI).
  quotes.push({
    secid: `${input.underlying}-FARC`,
    underlying: input.underlying,
    expirationTs,
    strike: Number((forward * 1.25).toFixed(2)),
    kind: "call",
    bid: null,
    ask: null,
    last: null,
    iv: null,
    oi: null,
    theorPrice: null,
  });
  return { quotes, forward, expirationTs, smilePoints };
}

// ── Фаза 10 — MOEX ALGO: Key Activity (упрощённые правила для мок-режима) ─────
// Зеркалит доменный `default_rules()` достаточно, чтобы таблица «Ключевая
// активность» и панель «ИТОГО» работали без бэкенда.

const keyActivityRules: KeyActivityRuleDto[] = [
  { id: "anomalous_volume", name: "Аномальный объём", weight: 1 },
  { id: "flow_imbalance", name: "Сильный дисбаланс потока", weight: 0.9 },
  { id: "concentration_spike", name: "Всплеск концентрации HI2", weight: 0.8 },
  { id: "price_move", name: "Резкое движение цены", weight: 0.7 },
];

function mockSampleSet(): KeyActivitySampleInput[] {
  return [
    { secid: "SBER", ts: 4, volume: 5200, volumeZ: 3.8, disb: 0.55, hi2: 0.22, priceChange: 0.031 },
    { secid: "GAZP", ts: 4, volume: 900, volumeZ: 0.6, disb: -0.62, hi2: 0.71, priceChange: -0.008 },
    { secid: "LKOH", ts: 4, volume: 2100, volumeZ: 1.2, disb: 0.15, hi2: 0.34, priceChange: 0.026 },
    { secid: "GMKN", ts: 4, volume: 1500, volumeZ: 2.9, disb: 0.05, hi2: 0.28, priceChange: -0.004 },
  ];
}

function evalKeyActivity(samples: KeyActivitySampleInput[]): KeyActivityRowDto[] {
  const rows: KeyActivityRowDto[] = [];
  for (const s of samples) {
    if ((s.volumeZ ?? 0) >= 3) {
      rows.push(row(s, "anomalous_volume", "Аномальный объём", "z-score объёма", s.volumeZ ?? 0, 1));
    }
    if (Math.abs(s.disb ?? 0) >= 0.4) {
      rows.push(row(s, "flow_imbalance", "Сильный дисбаланс потока", "дисбаланс", s.disb ?? 0, 0.9));
    }
    if ((s.hi2 ?? 0) >= 0.6) {
      rows.push(row(s, "concentration_spike", "Всплеск концентрации HI2", "концентрация HI2", s.hi2 ?? 0, 0.8));
    }
    if (Math.abs(s.priceChange ?? 0) >= 0.02) {
      rows.push(row(s, "price_move", "Резкое движение цены", "изменение цены", s.priceChange ?? 0, 0.7));
    }
  }
  return rows.sort((a, b) => b.importance - a.importance || a.secid.localeCompare(b.secid));
}

function row(
  s: KeyActivitySampleInput,
  ruleId: string,
  ruleName: string,
  metric: string,
  value: number,
  importance: number,
): KeyActivityRowDto {
  return { secid: s.secid, ruleId, ruleName, metric, value, ts: s.ts, importance };
}

function mockKeyActivitySummary(
  samples: KeyActivitySampleInput[],
  period: string,
): KeyActivitySummaryDto {
  const rows = evalKeyActivity(samples);
  const lines = rows
    .slice(0, 8)
    .map((r) => `• ${r.secid}: ${r.ruleName.toLowerCase()} (${r.metric} = ${r.value.toFixed(3)})`);
  const text =
    rows.length === 0
      ? `За период ${period} значимых активностей не выявлено.`
      : `Ключевая активность за ${period} (${rows.length} сигналов):\n${lines.join("\n")}`;
  return { text, period, rowCount: rows.length, fallback: true, source: "local" };
}

// ── T11 — MOEX ALGO: датасеты ALGOPACK (мок IPC) ─────────────────────────────
//
// Перенесено из `algoMock.ts` (демо-генераторы вкладки «MOEX ALGO», Фаза 10):
// генераторы теперь отдают DTO-совместимые структуры (camelCase, как из ядра)
// через мок-команды `algo_tradestats`/`algo_futoi`/`algo_hi2`/`algo_mega_alerts`.
// Детерминированный PRNG (FNV-hash + mulberry32) — воспроизводимые числа без
// сети, как и у остальных генераторов этого файла.

/** Якорь оси времени условной торговой сессии (unix-секунды UTC). */
const ALGO_BASE_TS = 1_717_400_000;

function algoHash(s: string): number {
  let h = 2166136261 >>> 0;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

function algoRng(seed: number): () => number {
  let a = seed >>> 0;
  return function () {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Инструмент вселенной вкладки «MOEX ALGO» (тулбар/пикер тикера). */
export interface AlgoTicker {
  ticker: string;
  name: string;
  sector: string;
}

const ALGO_UNIVERSE: AlgoTicker[] = [
  { ticker: "SBER", name: "Сбербанк", sector: "Финансы" },
  { ticker: "GAZP", name: "Газпром", sector: "Нефтегаз" },
  { ticker: "LKOH", name: "Лукойл", sector: "Нефтегаз" },
  { ticker: "GMKN", name: "ГМК Норникель", sector: "Металлургия" },
  { ticker: "ROSN", name: "Роснефть", sector: "Нефтегаз" },
  { ticker: "VTBR", name: "ВТБ", sector: "Финансы" },
  { ticker: "YDEX", name: "Яндекс", sector: "IT" },
  { ticker: "MGNT", name: "Магнит", sector: "Ритейл" },
  { ticker: "NVTK", name: "Новатэк", sector: "Нефтегаз" },
  { ticker: "TATN", name: "Татнефть", sector: "Нефтегаз" },
  { ticker: "MOEX", name: "Мосбиржа", sector: "Финансы" },
  { ticker: "PLZL", name: "Полюс", sector: "Металлургия" },
];

const ALGO_BASE_PRICE: Record<string, number> = {
  SBER: 302.4,
  GAZP: 128.6,
  LKOH: 7120,
  GMKN: 112.8,
  ROSN: 565.2,
  VTBR: 96.3,
  YDEX: 4210,
  MGNT: 6840,
  NVTK: 1042,
  TATN: 712.5,
  MOEX: 198.4,
  PLZL: 11840,
};

/** Вселенная тикеров вкладки «MOEX ALGO» (не IPC-команда — статический
 * справочник для тулбара/пикера, как и остальной инструментарий этого файла). */
export function algoTickers(): AlgoTicker[] {
  return ALGO_UNIVERSE;
}

/** Свечи Super Candles (`algo_tradestats`): 78 пятиминуток условной сессии. */
function algoCandles(secid: string): TradestatsDto[] {
  const base = ALGO_BASE_PRICE[secid] ?? 100;
  const r = algoRng(algoHash(secid) + 3);
  let px = base;
  let cumPv = 0;
  let cumV = 0;
  const out: TradestatsDto[] = [];
  for (let i = 0; i < 78; i++) {
    const drift = (r() - 0.48) * base * 0.0045;
    const o = px;
    const c = px + drift;
    const h = Math.max(o, c) + r() * base * 0.0022;
    const l = Math.min(o, c) - r() * base * 0.0022;
    const anomalous = i % 17 === 5 || i % 23 === 3;
    const vol = Math.round((1800 + r() * 6500) * (anomalous ? 3.4 : 1));
    const val = vol * ((o + c) / 2);
    cumPv += ((h + l + c) / 3) * vol;
    cumV += vol;
    const vwap = cumV > 0 ? cumPv / cumV : c;
    const disb = Math.max(-1, Math.min(1, (r() - 0.5) * 2 + ((drift * 4) / base) * 100));
    const volB = vol * (0.5 + disb / 2);
    const volS = vol * (0.5 - disb / 2);
    out.push({
      secid,
      ts: ALGO_BASE_TS + i * 300,
      prOpen: o,
      prHigh: h,
      prLow: l,
      prClose: c,
      prStd: Math.abs(h - l) / 4,
      vol,
      val,
      trades: Math.round(vol / 8),
      prVwap: vwap,
      prChange: o !== 0 ? (c - o) / o : 0,
      volB,
      volS,
      valB: val * (vol > 0 ? volB / vol : 0.5),
      valS: val * (vol > 0 ? volS / vol : 0.5),
      tradesB: Math.round((vol / 8) * (vol > 0 ? volB / vol : 0.5)),
      tradesS: Math.round((vol / 8) * (vol > 0 ? volS / vol : 0.5)),
      disb,
      prVwapB: vwap,
      prVwapS: vwap,
      buyPressure: vol > 0 ? volB / vol : 0.5,
    });
    px = c;
  }
  return out;
}

function algoFutoiPoint(
  secid: string,
  ts: number,
  clgroup: "fiz" | "yur",
  long: number,
  short: number,
): FutoiDto {
  const total = long + short;
  return {
    secid,
    ts,
    clgroup,
    pos: total,
    posLong: long,
    posShort: short,
    posLongNum: long / 1000,
    posShortNum: short / 1000,
    net: long - short,
    longShare: total > 0 ? long / total : 0.5,
  };
}

/** Точки FUTOI (`algo_futoi`): 8 часовых отметок × 2 группы (физ/юр). */
function algoFutoi(secid: string): FutoiDto[] {
  const r = algoRng(algoHash(secid) + 55);
  const out: FutoiDto[] = [];
  for (let h = 0; h < 8; h++) {
    const ts = ALGO_BASE_TS + h * 3600;
    out.push(
      algoFutoiPoint(secid, ts, "fiz", Math.round(120 + r() * 40) * 1000, Math.round(150 + r() * 40) * 1000),
    );
    out.push(
      algoFutoiPoint(secid, ts, "yur", Math.round(180 + r() * 40) * 1000, Math.round(140 + r() * 40) * 1000),
    );
  }
  return out;
}

/** Уровень концентрации по порогам — как `domain::algo::hi2::ConcentrationLevel`. */
function algoHi2Level(c: number): Hi2Dto["level"] {
  if (c >= 0.5) return "dominated";
  if (c >= 0.25) return "concentrated";
  if (c >= 0.15) return "moderate";
  return "distributed";
}

/** Точки HI2 (`algo_hi2`): 48 десятиминуток условной сессии. */
function algoHi2(secid: string): Hi2Dto[] {
  const r = algoRng(algoHash(secid) + 88);
  let x = 0.2;
  const out: Hi2Dto[] = [];
  for (let i = 0; i < 48; i++) {
    x = Math.max(0.05, Math.min(0.5, x + (r() - 0.5) * 0.05));
    const concentration = +x.toFixed(3);
    out.push({
      ts: ALGO_BASE_TS + i * 600,
      secid,
      concentration,
      level: algoHi2Level(concentration),
      spike: concentration > 0.3,
    });
  }
  return out;
}

const MEGA_KINDS: MegaAlertKind[] = [
  "volume_spike",
  "buy_imbalance",
  "sell_imbalance",
  "spread_widening",
  "oi_jump",
  "concentration_rise",
];

const MEGA_MESSAGES: Record<MegaAlertKind, string> = {
  volume_spike: "всплеск объёма",
  buy_imbalance: "перевес покупок",
  sell_imbalance: "перевес продаж",
  spread_widening: "расширение спреда",
  oi_jump: "скачок открытого интереса",
  concentration_rise: "рост концентрации",
};

/** Лента Mega Alerts (`algo_mega_alerts`) по инструментам `secids`. */
function algoMegaAlerts(secids: string[]): MegaAlertDto[] {
  const pool = secids.length > 0 ? secids : ALGO_UNIVERSE.map((t) => t.ticker);
  const r = algoRng(19);
  const out: MegaAlertDto[] = [];
  let ts = ALGO_BASE_TS + 12 * 60;
  for (let i = 0; i < 18; i++) {
    const kind = MEGA_KINDS[Math.floor(r() * MEGA_KINDS.length)];
    const secid = pool[Math.floor(r() * pool.length)];
    let value: number;
    switch (kind) {
      case "volume_spike":
        value = r() * 4 + 2.5;
        break;
      case "buy_imbalance":
        value = r() * 0.6 + 0.3;
        break;
      case "sell_imbalance":
        value = -(r() * 0.6 + 0.3);
        break;
      case "spread_widening":
        value = (r() * 3 + 1) / 10_000;
        break;
      case "oi_jump":
        value = (r() > 0.5 ? 1 : -1) * (r() * 60_000 + 15_000);
        break;
      default:
        value = r() * 0.25 + 0.3;
    }
    ts += Math.round(r() * 14 + 3) * 60;
    out.push({ secid, ts, kind, value, message: MEGA_MESSAGES[kind] });
  }
  return out.reverse();
}

// ── Фаза 11 — Историзация: демо-каталог датасетов ────────────────────────────
const DAY = 86_400;
const mockDatasets: DatasetMetaDto[] = [
  { source: "finam", secid: "SBER", tf: "d1", fromTs: 0, toTs: DAY * 365, bars: 365, updatedTs: DAY * 365, looksComplete: true },
  { source: "finam", secid: "GAZP", tf: "h1", fromTs: 0, toTs: DAY * 90, bars: 90 * 9, updatedTs: DAY * 90, looksComplete: true },
  { source: "moex_algo", secid: "SBER", tf: "m5", fromTs: 0, toTs: DAY * 30, bars: 30 * 78, updatedTs: DAY * 30, looksComplete: false },
];

function mockHistoryPlan(input: {
  covered: { from: number; till: number }[];
  requestedFrom: number;
  requestedTill: number;
}): TimeRangeDto[] {
  // Нормализуем покрытие и вычитаем из запрошенного окна (как domain::missing_ranges).
  const covered = [...input.covered].sort((a, b) => a.from - b.from);
  const gaps: TimeRangeDto[] = [];
  let cursor = input.requestedFrom;
  for (const c of covered) {
    if (c.till <= cursor) continue;
    if (c.from > cursor) gaps.push({ from: cursor, till: Math.min(c.from, input.requestedTill) });
    cursor = Math.max(cursor, c.till);
    if (cursor >= input.requestedTill) break;
  }
  if (cursor < input.requestedTill) gaps.push({ from: cursor, till: input.requestedTill });
  return gaps.filter((g) => g.till > g.from);
}

// ── T3 — Настройки и правила Key Activity (мок ядра для браузера) ────────────
// Имитирует персист в core JSON-файл в памяти вкладки: браузерный мок-режим
// не переживает перезагрузку страницы — там источником истины остаётся
// localStorage (см. `lib/settings.ts`). Здесь — только чтобы `settings_get`/
// `settings_set`/`key_activity_rules_*` были рабочими командами в dev-режиме
// без Tauri (нужно для тестов миграции и локальной разработки UI).

function defaultMockSettings(): SettingsDto {
  return {
    tapeLimit: 50,
    domDepth: 10,
    topMoversLimit: 10,
    markets: { eq: true, fo: true, fx: false },
    watchlist: { SBER: true, GAZP: true, LKOH: true, GMKN: false, ROSN: true, VTBR: false },
    llmProvider: "openrouter",
    llmModel: "anthropic/claude-3.5-sonnet",
    llmHasKey: false,
    llmTokenLimit: 2000,
    llmAuto: true,
    defaultPeriod: "1h",
    dataSource: "finam",
    dataDir: "~/.market-terminal/history",
    concurrency: 4,
    pricingModel: "black76",
    rate: 0,
    defaultSmile: "moex",
  };
}

let mockSettings: SettingsDto | null = null;
let mockKeyActivityRules: KeyActivityRuleDto[] = [];

/** Сбросить мок-состояние «ядра» (настройки/правила Key Activity) между
 * тестами — модульные переменные иначе переживают отдельные `it()` в одном
 * файле. Только для тестов. */
export function resetCoreMockForTests(): void {
  mockSettings = null;
  mockKeyActivityRules = [];
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
    case "latest_trades": {
      const sym = String(args?.symbol ?? "SBER@MISX");
      const limit = Number(args?.limit ?? 50);
      return genTrades(sym, limit) as unknown as T;
    }
    case "order_book": {
      const sym = String(args?.symbol ?? "SBER@MISX");
      const depth = Number(args?.depth ?? 10);
      return genOrderBook(sym, depth) as unknown as T;
    }
    case "alerts_scan": {
      const rules = (args?.rules ?? []) as AlertRuleInput[];
      return scanAlerts(rules) as unknown as T;
    }
    // ── V2 ──────────────────────────────────────────────────────────────────
    case "list_strategies":
      return strategyDescriptors as unknown as T;
    case "run_backtest": {
      const sym = String(args?.symbol ?? "SBER@MISX");
      const cfg = (args?.config ?? {}) as { initialCapital?: number };
      return mockBacktest(sym, cfg.initialCapital ?? 100_000) as unknown as T;
    }
    case "delta_footprint": {
      const sym = String(args?.symbol ?? "SBER@MISX");
      return mockFootprint(sym) as unknown as T;
    }
    case "robot_scan": {
      const sym = String(args?.symbol ?? "SBER@MISX");
      return mockRobotSignals(sym) as unknown as T;
    }
    case "submit_order":
      return mockSubmit(args?.order as OrderInput) as unknown as T;
    case "cancel_order": {
      const id = Number(args?.id);
      const idx = sim.orders.findIndex((o) => o.id === id);
      if (idx < 0) throw new Error("заявка не найдена");
      const [o] = sim.orders.splice(idx, 1);
      return { ...o, status: "cancelled" } as unknown as T;
    }
    case "order_blotter":
      // Снимок, а не живая ссылка: реальный Tauri IPC всегда отдаёт
      // десериализованную копию, мок должен вести себя так же.
      return [...sim.orders] as unknown as T;
    case "positions":
      return Array.from(sim.positions.entries())
        .filter(([, p]) => p.qty !== 0)
        .map(([symbol, p]) => ({ symbol, qty: p.qty, avgPrice: p.avg })) as unknown as T;
    case "account":
      return { cash: sim.cash, realizedPnl: sim.realizedPnl } as unknown as T;

    // ── Фаза 12 / Опционы ──────────────────────────────────────────────────────
    case "list_smile_models":
      return smileModels as unknown as T;
    case "option_price":
      return mockOptionPrice(args?.input as OptionPriceInput) as unknown as T;
    case "option_implied_vol":
      return mockImpliedVol(args?.input as ImpliedVolInput) as unknown as T;
    case "smile_fit":
      return mockSmileFit(args?.input as SmileFitInput) as unknown as T;
    case "strategy_eval":
      return mockStrategyEval(args?.input as StrategyEvalInput) as unknown as T;
    case "option_board":
      return mockOptionBoard(args?.input as OptionBoardInput) as unknown as T;

    // ── Фаза 10 / MOEX ALGO: Key Activity ───────────────────────────────────────
    case "key_activity": {
      const samples = (args?.samples as KeyActivitySampleInput[]) ?? mockSampleSet();
      return evalKeyActivity(samples) as unknown as T;
    }
    case "key_activity_summary": {
      const samples = (args?.samples as KeyActivitySampleInput[]) ?? mockSampleSet();
      const period = String(args?.period ?? "1h");
      return mockKeyActivitySummary(samples, period) as unknown as T;
    }
    case "key_activity_rules":
      return keyActivityRules as unknown as T;

    // ── T11 / MOEX ALGO: датасеты ALGOPACK ──────────────────────────────────────
    case "algo_tradestats": {
      const secid = String(args?.secid ?? "SBER");
      return algoCandles(secid) as unknown as T;
    }
    case "algo_futoi": {
      const secid = String(args?.secid ?? "SBER");
      return algoFutoi(secid) as unknown as T;
    }
    case "algo_hi2": {
      const secid = String(args?.secid ?? "SBER");
      return algoHi2(secid) as unknown as T;
    }
    case "algo_mega_alerts": {
      const secids = (args?.secids as string[]) ?? [];
      return algoMegaAlerts(secids) as unknown as T;
    }

    // ── Фаза 11 / Историзация ────────────────────────────────────────────────────
    case "history_datasets":
      return mockDatasets as unknown as T;
    case "history_delete": {
      const id = args?.id as { source: string; secid: string; tf: string };
      const idx = mockDatasets.findIndex(
        (d) => d.source === id.source && d.secid === id.secid && d.tf === id.tf,
      );
      if (idx >= 0) mockDatasets.splice(idx, 1);
      return (idx >= 0) as unknown as T;
    }
    case "history_plan": {
      const inp = args?.input as {
        covered: { from: number; till: number }[];
        requestedFrom: number;
        requestedTill: number;
      };
      return mockHistoryPlan(inp) as unknown as T;
    }

    // ── T3 / Настройки и правила Key Activity ───────────────────────────────────
    case "settings_get":
      if (!mockSettings) mockSettings = defaultMockSettings();
      return mockSettings as unknown as T;
    case "settings_set":
      mockSettings = args?.doc as SettingsDto;
      return undefined as unknown as T;
    case "key_activity_rules_get":
      return mockKeyActivityRules as unknown as T;
    case "key_activity_rules_set": {
      const rulesJson = String(args?.rulesJson ?? "[]");
      let parsed: { id?: string; name?: string; weight?: number }[];
      try {
        parsed = JSON.parse(rulesJson);
      } catch {
        throw new Error("невалидные правила Key Activity: битый JSON");
      }
      mockKeyActivityRules = parsed.map((r) => ({
        id: r.id ?? "",
        name: r.name ?? "",
        weight: r.weight ?? 0,
      }));
      return mockKeyActivityRules as unknown as T;
    }

    default:
      throw new Error(`mock: неизвестная команда ${cmd}`);
  }
}
