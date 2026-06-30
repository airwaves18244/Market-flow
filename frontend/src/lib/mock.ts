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
  FlowEdgeDto,
  FootprintBarDto,
  FutureGroupDto,
  InstrumentDto,
  OrderBookDto,
  OrderDto,
  OrderInput,
  PositionDto,
  RobotSignalDto,
  RrgSectorDto,
  SectorEntryDto,
  SectorRow,
  StrategyDescriptorDto,
  SubmitResultDto,
  TopMoverDto,
  TradeDto,
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
    default:
      throw new Error(`mock: неизвестная команда ${cmd}`);
  }
}
