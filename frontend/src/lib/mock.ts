// Демо-данные для запуска фронта вне Tauri (`npm run dev` без Rust-ядра).
// Позволяют разрабатывать и проверять UI до подключения реальных IPC-команд.

import type { Breadth, EquityDashboard, FlowPoint, RrgPoint } from "./types";

export function mockEquityDashboard(fromTs: number, toTs: number): EquityDashboard {
  return {
    from_ts: fromTs,
    to_ts: toTs,
    sectors: [
      { sector: "Нефтегаз", turnover: 4.2e9, net_flow: 3.1e8 },
      { sector: "Финансы", turnover: 3.6e9, net_flow: -1.4e8 },
      { sector: "Металлы", turnover: 2.1e9, net_flow: 0.9e8 },
      { sector: "Энергетика", turnover: 1.3e9, net_flow: 0.2e8 },
      { sector: "IT", turnover: 0.8e9, net_flow: -0.3e8 },
    ],
    top_movers: [
      { symbol: "GAZP@MISX", name: "Газпром", change: 0.041, turnover: 2.0e9 },
      { symbol: "SBER@MISX", name: "Сбербанк", change: -0.028, turnover: 1.8e9 },
      { symbol: "LKOH@MISX", name: "Лукойл", change: 0.019, turnover: 1.1e9 },
      { symbol: "GMKN@MISX", name: "Норникель", change: -0.015, turnover: 0.7e9 },
    ],
  };
}

export function mockFlowSeries(): FlowPoint[] {
  const day = 86_400;
  const start = Math.floor(Date.now() / 1000) - 30 * day;
  const out: FlowPoint[] = [];
  let acc = 0;
  for (let i = 0; i < 30; i++) {
    acc += Math.sin(i / 3) * 5e7 + (Math.random() - 0.5) * 2e7;
    out.push({ ts: start + i * day, net_flow: acc });
  }
  return out;
}

export function mockBreadth(): Breadth {
  const advancers = 142;
  const decliners = 88;
  const unchanged = 20;
  return {
    advancers,
    decliners,
    unchanged,
    pct_advancing: advancers / (advancers + decliners + unchanged),
  };
}

export function mockRrg(): RrgPoint[] {
  return [
    { sector: "Нефтегаз", rs_ratio: 103.4, rs_momentum: 101.2 },
    { sector: "Финансы", rs_ratio: 98.1, rs_momentum: 99.4 },
    { sector: "Металлы", rs_ratio: 101.0, rs_momentum: 102.6 },
    { sector: "Энергетика", rs_ratio: 97.2, rs_momentum: 98.1 },
    { sector: "IT", rs_ratio: 99.6, rs_momentum: 100.9 },
  ];
}
