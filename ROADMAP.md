# ROADMAP — Market Terminal

Пошаговый план. Отметка ✅ — сделано в текущей итерации.

## Фаза 0 — Фундамент ✅ (частично)
- ✅ Cargo workspace, члены: `finam-proto`, `domain`, `data`, `storage`, `app`.
- ✅ Дисциплина слоёв (аналитика в `domain` без внешних зависимостей).
- ✅ Контракты `data` (трейт `MarketData`, ошибки, `TimeFrame`), классификация секторов.
- ✅ DDL схемы DuckDB в `storage::schema`.
- ⏳ gRPC-стабы из `.proto` (`tonic-build`), auth + refresh токена, per-method
  rate-limiter (`governor`), `keyring` для ключа, `tracing`.

## Фаза 1 — Хранилище и ингест ✅
- ✅ Нативный `duckdb` (bundled) за фичей `duckdb`, применение DDL, миграции
  (версия схемы, идемпотентный прогон).
- ✅ Контракт `Store` + реализации: `MemStore` (в памяти, кросс-платформенно
  тестируемая) и `DuckStore` (DuckDB).
- ✅ Writer ингеста баров/снимков оборота/инструментов; снимок оборота из серии
  баров (`snapshot_from_bars`); планировщик батч-поллинга (`BatchCursor`).
- ✅ Загрузка таблицы классификации секторов (`Writer::load_sector_map`).
- ✅ Бэкфилл исторических баров: `plan_backfill` + `chunk_range` (страницы под
  лимит баров на запрос).
- ⏳ Подключение реального источника (`data::MarketData`) и асинхронного цикла
  опроса — в фазе интеграции API/UI (`app`).

## Фаза 2 — Аналитика (`domain`) ✅
- ✅ turnover, directional turnover, unusual volume.
- ✅ money flow, MFI, CVD.
- ✅ breadth (A/D, % растущих).
- ✅ sector rollups (взвешенные по обороту).
- ✅ RRG (RS-Ratio / RS-Momentum, квадранты).
- ✅ cross-asset shares + flow matrix (Sankey).

## Фаза 3 — Tauri-оболочка + каркас фронта ✅
- ✅ Ядро IPC: `AppState` поверх `Store`, DTO (camelCase) и обработчики
  команд (`instruments`, `bars`, `turnover_series`, `sector_rollup`,
  `sector_map`) — чистые, протестированные на `MemStore`.
- ✅ Привязка Tauri за фичей `tauri`: `#[tauri::command]`-обёртки, заготовка
  событий live-push, `tauri.conf.json` + capabilities + `build.rs`. Сборка
  десктопа требует webkit2gtk, поэтому фича выключена в кросс-платформенном CI.
- ✅ Фронт: Vite + Svelte 5 + TS, тёмная тема, каркас докуемых панелей,
  типизированный IPC-клиент с мок-режимом (работает в браузере без бэкенда),
  ECharts treemap и Lightweight Charts свечи.
- ⏳ Полноценный dockview и асинхронный планировщик ингеста — в следующих фазах.

## Фаза 4 — Представление 1 (Акции/секторы) ✅
- ✅ treemap (размер=оборот, цвет=%изм) — уже реализовано в Фазе 3.
- ✅ heatmap — ECharts компонент по секторам и изменениям.
- ✅ breadth — индикатор ширины рынка (advancers/decliners/A/D ratio).
- ✅ топ-движения — таблица инструментов с наибольшим абсолютным изменением.
- ✅ RRG — scatter-график RS-Ratio vs RS-Momentum по секторам с квадрантами.
- API: новые обработчики `breadth_data()`, `top_movers()`, `rrg_sectors()` в `crates/app/src/api.rs`.
- Frontend: компоненты `BreadthIndicator.svelte`, `TopMoversTable.svelte`, `HeatmapChart.svelte`, `RrgChart.svelte`.
- Tauri: команды `breadth_data`, `top_movers`, `rrg_sectors` зарегистрированы.

## Фаза 5 — Представления 2 и 3 (Фьючерсы, Облигации) ✅
- ✅ Фьючерсы: treemap по группам (2-символьный префикс), open interest, оборот/поток.
- ✅ Облигации: кривая доходности (8 стандартных сроков); таблица эмитентов (3-символ).
- API: новые обработчики `futures_rollup()`, `bonds_rollup()`, `yield_curve()`.
- Store: новый метод `instruments_by_asset_class()` для фильтрации по классу активов.
- Frontend: компоненты `FuturesTreemap.svelte`, `YieldCurve.svelte`, `BondsTable.svelte`.
- Таури: команды `futures_rollup`, `bonds_rollup`, `yield_curve` зарегистрированы.

## Фаза 6 — Представление 4 (Сумма всех) ✅
- ✅ Общий оборот (gauge), donut долей, stacked area во времени, Sankey перетоков.
- API: `cross_asset_summary()` (gauge+donut), `turnover_timeline()` (stacked area),
  `flow_sankey()` (перетоки долей) — поверх `domain::metrics::crossasset`.
- DTO: `CrossAssetSummaryDto`, `AssetClassShareDto`, `TurnoverByClassPoint`, `FlowEdgeDto`.
- Tauri: команды `cross_asset_summary`, `turnover_timeline`, `flow_sankey`.
- Frontend: `TotalTurnoverGauge`, `SharesDonut`, `TurnoverStackedArea`, `FlowSankey`
  (+ общий помощник `assetClass.ts` с подписями/цветами классов).

## Фаза 7 — Live-функции ✅ (логика и панели; стрим — далее)
- ✅ Стакан (DOM): доменная модель книги (`domain::live::book`) — лучшие цены,
  спред, mid, дисбаланс спроса/предложения, кумулятивная глубина (лесенка).
- ✅ Лента сделок (Time&Sales): `domain::live::tape` — классификация сторон
  (явный инициатор или правило тика), CVD, VWAP, купленный/проданный объём.
- ✅ Алёрты: `domain::live::alerts` — правила (цена ↑/↓, |изменение|, всплеск
  объёма, спред) + движок проверки по снимку рынка и серьёзность.
- ✅ Replay-режим: `domain::live::replay` — курсор по шкале времени (шаг,
  перемотка, прогресс) + фронт-контролы воспроизведения.
- API: `order_book()`, `time_and_sales()`, `active_alerts()`, `replay_state()`
  в `crates/app/src/api.rs` (scaffold-данные поверх доменной логики, как
  `yield_curve` — до подключения живого источника).
- DTO: `OrderBookDto`/`OrderBookLevelDto`, `TimeAndSalesDto`/`TapeEntryDto`/
  `TapeStatsDto`, `TriggeredAlertDto`, `ReplayStateDto`.
- Tauri: команды `order_book`, `time_and_sales`, `active_alerts`, `replay_state`
  + заготовки live-push событий (`quote:tick`, `trade:tick`, `book:update`).
- Frontend: `DomLadder`, `TimeSales`, `AlertsPanel`, `ReplayControls`.
- ⏳ Осталось: подключение реального стрима Finam (`SubscribeQuote`,
  `SubscribeLatestTrades`, `SubscribeOrderBook`) и пуш событий в вебвью.

## Фаза 8 — Полировка и сборка
- Упаковка MSI/NSIS (Tauri bundler), производительность, обработка ошибок, настройки.
