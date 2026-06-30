# Design source — Market Flow Terminal

Эталонный прототип терминала, экспортированный из **Claude Design**
(claude.ai/design). Это HTML/CSS/JS-макет (не продакшен-код) — источник правды
по визуалу, раскладке и поведению вкладок. Реализация живёт в `frontend/src`
(Svelte 5) и `crates/*` (Rust-ядро).

## Содержимое

- `Market Flow Terminal.dc.html` — прототип всех десяти рабочих пространств.
- `support.js` — рантайм-помощник прототипа (из бандла Claude Design).
- `roadmap_design.md` — бэкенд-задачи под новые вкладки (Сводка / Бэктест /
  Торговля), как их сформулировал дизайн-ассистент. Сведено и расширено в
  корневом `ROADMAP.md`.
- `screenshots/` — `overview` / `sectors` / `flows` / `live`.

## Как макет лёг на реализацию

| Вкладка дизайна            | Реализация                                                            |
| -------------------------- | -------------------------------------------------------------------- |
| Обзор / Overview           | `App.svelte` ws=overview · treemap, gauge, donut, breadth, movers     |
| Сводка / Summary           | `SummaryPanel.svelte` ← `ipc.summary` ← `domain::metrics::regime`     |
| Секторы / Sectors          | `SectorTreemap`, `RrgChart`, `HeatmapChart`                           |
| Потоки / Flows             | `FlowSankey`, `SharesDonut`, `TurnoverStackedArea`                    |
| Лента / Tape & DOM         | `CandleChart`, `OrderBook`, `TimeSales`, `AlertsPanel`               |
| Фьючерсы / Futures         | `FuturesTreemap` ← `ipc.futuresRollup`                                |
| Облигации / Bonds          | `YieldCurve`, `BondsTable`                                            |
| Бэктест / Backtest         | `BacktestPanel.svelte` — **прототип** (UI-симуляция, без бэкенда)     |
| Торговля / Trade           | `TradePanel.svelte` — **прототип** (READ-ONLY, заявки не уходят)      |
| Настройки / Settings       | `SettingsPanel` (localStorage)                                        |

Вкладки «Бэктест» и «Торговля» намеренно остаются UI-прототипами: их бэкенд
ломает гарантию READ-ONLY v1 и описан как будущая работа в `ROADMAP.md`.
