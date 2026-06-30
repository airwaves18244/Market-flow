# roadmap_design.md — backend tasks for the new prototype tabs

These three terminal tabs are **UI prototypes with no backend**. The data is
generated/simulated in the component. This file lists what the Rust core
(`crates/…`) + Tauri IPC must add to make them real. Layer rule unchanged: all
analytics live in `domain`, adapters in `data`/`storage`/`app`.

---

## 1. Summary — cross-asset money-flow analysis ("where big money moves")

Goal: a decision-grade read of where capital rotates across the **Russian
market**: equities ↔ bonds ↔ futures ↔ FX spot. Regime call (Risk-ON / OFF /
Neutral) + conviction + drivers + decision aid.

New asset class to ingest: **FX spot** (валютный рынок) — USD/RUB, CNY/RUB,
EUR/RUB. Today `AssetClass = equity | future | bond`; add `fx`.

Backend tasks
- `data`: classify + ingest FX spot instruments (Finam `MarketDataService`,
  board e.g. `CETS`). Extend `classify.rs` with an `fx` class; map symbols.
- `domain/metrics/crossasset.rs`: extend net-flow & Sankey from 3→4 classes
  incl. `fx`. Add **directed net flow per class over [fromTs,toTs]** (signed ₽),
  not just shares.
- New `domain/metrics/regime.rs`:
  - inputs: per-class net flow, breadth (A/D), index CVD, sector MFI
    (overbought count), OFZ yield Δ (curve direction), RUB Δ.
  - output: `RegimeDto { regime: RiskOn|RiskOff|Neutral, conviction: 0..100,
    thesis, drivers: [{label, value, direction}], decisions: [..], risks: [..] }`.
  - classifier rules (mirror the prototype): equity outflow + bond/FX inflow ⇒
    Risk-OFF; equity inflow + bond outflow ⇒ Risk-ON; else Neutral. Conviction
    = normalized magnitude of cross-class flow. Unit-test deterministically.
- `app` IPC: `summary(fromTs,toTs) -> RegimeDto`. Wire `frontend/ipc.ts`
  (`ipc.summary`) and replace `classFlowsH` / `regimeOf` mock in the component.
- The per-period control already maps to `[fromTs,toTs]` — reuse it.

## 2. Backtesting

Goal: rules-based backtests over stored bars; equity curve vs IMOEX, stats,
monthly returns.

Backend tasks
- New crate or `domain/backtest/`:
  - strategy spec (`StrategyDef`): universe, signal rules (flow-rotation,
    RRG-momentum, unusual-volume breakout), sizing (fixed risk %), costs.
  - event-driven engine over `storage` bars; deterministic + unit-tested.
  - metrics: CAGR, Sharpe, Sortino, max drawdown, win rate, profit factor,
    exposure, trade list, monthly returns matrix.
- `storage`: ensure bar history depth + benchmark (IMOEX) series available;
  add query for date-ranged OHLCV per universe.
- `app` IPC: `backtest_run(StrategyDef, fromTs, toTs) -> BacktestResult`
  (+ optional progress events `backtest:tick`). Long runs → async task.
- `frontend`: replace `genEquity` / mock stats with `ipc.backtestRun`; presets →
  `StrategyDef` payloads; "Запустить" triggers the real run.

## 3. Live trading

Goal: real order entry + positions/orders/account. **This breaks the current
read-only v1 guarantee** — gate behind an explicit `trading` feature/flag and
clear consent; keep READ-ONLY default.

Backend tasks
- `data`: implement `AccountsService` + `OrdersService` (Finam Trade API) —
  place / cancel / replace, order status stream, positions, portfolio,
  buying power. Currently `AccountsService` is "not used" per README.
- `domain`: order validation (lot size, price step, tick, margin/risk checks),
  P&L (realized/unrealized), exposure, buying-power calc.
- `app`:
  - IPC: `place_order`, `cancel_order`, `positions`, `working_orders`,
    `account_summary`.
  - live events: `order:update`, `position:update`, `account:update`.
  - **safety**: confirm dialog, rate-limit, kill-switch, paper-trading mode,
    audit log; secrets via OS-keyring (never in repo).
- `frontend`: wire order ticket `onSubmit` → `ipc.placeOrder`; subscribe to
  position/order/account events; remove the "prototype / rejected" stub.

---

## Cross-cutting
- All aggregate IPC already takes `(fromTs,toTs)` — keep the per-panel + global
  period controls as the single time-range source of truth.
- Add `fx` everywhere asset-class is enumerated (DTOs, treemaps, donut, Sankey,
  storage schema → bump `schema vN`).
- Keep `domain` pure (no gRPC/Tauri/DuckDB) so regime + backtest stay
  CI-testable on Linux.
- Telemetry/tracing spans around new IPC handlers.
