# Claude Design Brief — Market Terminal: new tabs (Phases 10–12)

**Purpose.** Input for Claude Design to prototype the front-end of three new tabs
(**MOEX ALGO**, **Backtester**, **Options**) plus additions to the existing
**Settings**, for the desktop app *Market Terminal* (Russian-language market-flow
terminal for the Moscow Exchange).

This is a **front-end prototype brief**: layouts, components, states, and data
shapes. It is not a backend spec — see `SPEC_0-12.md` for engineering
detail. Task IDs in brackets (e.g. `[10.7.2]`) cross-reference that roadmap.

---

## 1. Product & design context

Market Terminal is a Tauri (Rust) + **Svelte 5 + TypeScript + Vite** desktop app.

**Existing visual language — match it exactly:**
- **Dark theme**, dense "trading terminal" feel, compact typography.
- Panels: reusable `Panel.svelte` card (title bar + body). Keep using it.
- Charts: **ECharts** (treemaps, heatmaps, scatter/RRG, donut, sankey, line/bar)
  and **Lightweight Charts** (candlesticks). Reuse these two libraries only.
- **All UI copy is in Russian.** Component names/code in English; visible labels
  in Russian. Russian labels are given below in **bold**.
- Asset-class colors/labels already exist in `lib/assetClass.ts` — reuse.
- Numbers: Russian formatting, turnover in **млрд ₽**, percentages with sign.

**New global element — Tab navigation [S.1].** Today the app is a single flat
grid of panels with a "Настройки" panel. Introduce a top **tab bar** with:
**Обзор** (the current grid, unchanged) · **MOEX ALGO** · **Бэктестер** ·
**Опционы** · **Настройки**. Active tab persists. Each tab is its own view; the
header (`Market Terminal` title + status line) stays above the tab bar.

**States to design for every data view:** loading (skeleton), empty
("нет данных"), error (inline banner — pattern already exists), and the
**mock/browser mode** (the app runs in a browser with mock data and no backend —
designs must look complete with placeholder data).

---

## 2. Tab: **MOEX ALGO** [10.7]

Powered by MOEX ALGOPACK analytics. The tab has a **sub-navigation** (segmented
control or left rail) across six modules. Markets: акции / фьючерсы / валюта
(eq/fo/fx). A global toolbar holds: **инструмент** (ticker picker), **период**
(`1ч / 1д / 1н / 1м / 3м`, default `1ч`), market selector.

### 2.1 Super Candles [10.7.2] — **«Супер-свечи»**
5-minute "super candles" with 50+ microstructure metrics.
- Main: **Lightweight Charts** candlestick + volume, with an overlaid **VWAP
  band** and a **buy/sell pressure (disbalance)** sub-indicator strip.
- Side/below: metrics table — open/high/low/close, VWAP, volume (lots),
  turnover (₽), trades count, **disb** (buy/sell ratio), buy-VWAP vs sell-VWAP,
  buy/sell volume split, price std. Anomalous-volume rows highlighted.
- Data shape (per 5-min row): `{ ts, prOpen, prHigh, prLow, prClose, prStd, vol,
  val, trades, prVwap, prChange, volB, volS, valB, valS, tradesB, tradesS, disb,
  prVwapB, prVwapS }`.

### 2.2 FUTOI [10.7.3] — **«Открытые позиции (FUTOI)»**
Futures open interest split by participant type (физлица / юрлица).
- Chart: stacked/line of **net long vs short** over time for физ and юр groups;
  toggle long/short/net.
- Table: `pos`, `posLong`, `posShort`, `posLongNum`, `posShortNum` per group;
  derived **net position** and **long share %**, **OI change** over the period.
- Highlight long/short extremes and price↔position divergence.
- Data shape: `{ ts, secid, clgroup: "FIZ"|"YUR", pos, posLong, posShort,
  posLongNum, posShortNum }`.

### 2.3 HI2 [10.7.4] — **«Концентрация рынка (HI2)»**
Herfindahl-style market-concentration index.
- Chart: concentration index time series with threshold bands (распределённый
  поток ↔ доминирование одного участника).
- Ranking table: instruments by concentration, with spike flags.
- Data shape: `{ ts, secid, hi2, ...deciles? }`.

### 2.4 Mega Alerts [10.7.5] — **«Мега-алёрты»**
Feed of unusual-activity signals derived from tradestats/FUTOI/order-book.
- A scrollable **alert feed** (newest first): signal type, ticker, metric &
  value, time, severity chip. Filter by type/ticker. Visual parity with the
  existing `AlertsPanel`.
- Signal types: volume spike, large buy/sell imbalance, spread widening, OI
  jump, HI2 concentration spike.
- Data shape: `{ ts, secid, type, metric, value, severity }`.

### 2.5 Key Activity [10.7.6] — **«Ключевая активность»**
The marquee table: key market activities over the selected **период** (default
`1ч`; also `1д / 1н / 1м / 3м`).
- Rule-driven rows (rules are default-defined and user-customizable in Settings).
- Table columns: **время · тикер · правило · метрика · значение · важность**.
  Group/sort by rule, ticker, or time; filter chips. Severity color-coding.
- Period selector prominent at top of the module.
- Data shape: `{ ts, ticker, rule, metric, value, severity }`.

### 2.6 TOTAL — LLM summary [10.7.7] — **«ИТОГО (ИИ-резюме)»**
A summary panel that runs an LLM (via OpenRouter by default) over the **Key
Activity** rows for the period and renders a narrative analysis.
- Layout: a wide card with the **generated text** (markdown), a **«Обновить»**
  button, model/period badge, timestamps.
- States: loading (generating), success, error, and **no-key fallback** (show a
  locally-assembled plain summary with a hint to add an API key in Settings).
- Data shape: `{ text, model, period, generatedAt }`.

**MOEX ALGO tab layout suggestion.** Toolbar (ticker/period/market) pinned top.
Sub-nav for the six modules. **Key Activity** + **TOTAL** are the "hero" of the
tab — consider Key Activity as a large central table with the TOTAL summary card
beside/above it, and Super Candles / FUTOI / HI2 / Mega Alerts as the other
sub-views.

---

## 3. Tab: **Backtester** [11.4] — **«Бэктестер»**

Phase 11 scope is the **historical-data loading & management** UI (the backtest
engine itself is a later phase). Two sections.

### 3.1 Load historical data [11.4.1] — **«Загрузка истории»**
A form panel:
- **Источник**: Finam Trade API · MOEX ALGO (radio/segmented; default from
  Settings).
- **Инструменты**: single ticker *or* a multi-select set of tickers.
- **Таймфреймы**: multi-select chips (M1, M5, M15, H1, D1; possibly M30/H4/W1).
- **Период**: date-range picker (from / till).
- **«Загрузить»** button → one or more **progress bars** (per ticker/TF), with
  cancel. Progress arrives via events (`history:progress/done/error`).

### 3.2 Dataset manager [11.4.2] — **«Локальные датасеты»**
A table of locally-stored datasets the user can manage:
- Columns: **источник · тикер · ТФ · диапазон дат · число баров · размер ·
  обновлено**, plus row actions **обновить / удалить**.
- A **preview** (candlestick) of a selected dataset for verification [11.4.4].
- Dataset shape: `{ id, source, ticker, tf, fromTs, toTs, bars, sizeBytes,
  updatedAt }`.

---

## 4. Tab: **Options** [12.7] — **«Опционы»**

Three sections (segmented control / sub-nav): **Калькулятор · Улыбка ·
Конструктор стратегий**. Needs charts and tables.

### 4.1 Calculator [12.7.2] — **«Калькулятор»**
- Inputs: underlying/forward price, strike, expiry, volatility, rate `r`
  (default **0** for MOEX margined options), option type (call/put), pricing
  model (**Black-76** / **Bachelier**).
- Outputs: theoretical price, **greeks** (delta, gamma, vega, theta, rho), and
  **implied volatility** (solve from market price). Results table.

### 4.2 Volatility smile [12.7.3] — **«Улыбка волатильности»**
- **ECharts** line/scatter: market IV points (per strike, sized by OI) with
  **overlays of up to four fitted models**: **MOEX**, **SABR**, **SVI**,
  **Каленкович**. Toggle each model on/off.
- Side panel: per-model **parameters** and **fit quality (RMSE)**; a model
  selector that sets the "active" smile.
- Data shapes: market point `{ strike, iv, oi }`; fit `{ model, params{...},
  rmse, curve:[{strike, iv}] }`.

### 4.3 Strategy builder [12.7.4] — **«Конструктор стратегий»**
- **Legs editor**: add/remove legs `{ type: call|put|underlying, side: long|
  short, strike, expiry, qty, entryPrice }`; preset templates (vertical,
  straddle, strangle, butterfly, condor, calendar, covered call).
- **Payoff diagram** (ECharts): P&L vs underlying price — at expiry and current
  (mark break-evens, max profit/loss).
- **Aggregated greeks** table for the whole position; optional **risk heatmap**
  (P&L over price × time/vol) [12.7.5].
- Result shape: `{ payoff:[{price, pnlExpiry, pnlNow}], greeks{...},
  breakevens:[...], maxProfit, maxLoss }`.

---

## 5. Settings additions [10.8 / 11.6 / 12.8] — **«Настройки»**

The existing Settings panel becomes a tab with **sections**. Keep the current
view controls (DOM depth, tape size, top-movers limit). Add:

**Секция «MOEX ALGO / Key Activity / LLM»**
- MOEX Passport connection — show only **«секрет задан: да/нет»** (never the
  secret value); markets + ALGOPACK watchlist.
- **Key Activity rule builder** [10.8.2] — the most complex new UI. Users build
  rules: each condition = **метрика** (volume / disb / OI / HI2 / spread / price
  change) + **оператор** (`> < >= <=`, crossing, z-score > k) + **порог** +
  **область** (ticker / set of tickers / whole market / asset class). Conditions
  compose: **«если A то B»**, **AND/OR/NOT**, arbitrary number of conditions,
  add/remove freely. Save / reset-to-defaults / import-export JSON. Design this
  as a clear visual rule editor (condition rows + logic connectors), not raw
  JSON.
- LLM: provider (**OpenRouter** default / Anthropic / OpenAI), model id, key
  status, token limit, auto-summary on/off; default analysis period.

**Секция «Данные / Историзация»** [11.6]
- Default source (Finam / MOEX ALGO), local storage directory, concurrency
  limits, default timeframes and history horizon.

**Секция «Опционы»** [12.8]
- Pricing model (Black-76 / Bachelier), rate `r` (default 0), default smile
  model per board, calibration weights/ranges, greek units.

> Secrets are entered here but stored by the backend (env/.env/OS-keyring), never
> in browser localStorage. The UI only reflects "set / not set".

---

## 6. Deliverables requested from Claude Design

1. **Tab bar** + overall shell with the five tabs.
2. **MOEX ALGO** tab with its six modules (§2), emphasizing **Key Activity** +
   **TOTAL (LLM)** as the hero.
3. **Backtester** tab: load form + dataset manager (§3).
4. **Options** tab: calculator, smile chart with multi-model overlay, strategy
   builder with payoff diagram (§4).
5. **Settings** sections, especially the **Key Activity rule builder** (§5).
6. For each: loading / empty / error / mock states, dark theme, Russian labels,
   ECharts + Lightweight Charts, consistent with the existing `Panel` look.

**Constraints:** dark theme only; Russian visible copy; reuse ECharts +
Lightweight Charts (no new chart libs); dense terminal layout; desktop-first
(Tauri window), responsive within a single window (no mobile).
