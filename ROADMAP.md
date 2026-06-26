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

## Фаза 3 — Tauri-оболочка + каркас фронта
- App state, IPC-команды (снимки + временные ряды), события (live-push).
- Vite + Svelte, ECharts/Lightweight Charts, докуемые панели, тёмная тема.

## Фаза 4 — Представление 1 (Акции/секторы)
- treemap (размер=оборот, цвет=%изм), heatmap, breadth, топ-движения, RRG.

## Фаза 5 — Представления 2 и 3 (Фьючерсы, Облигации)
- Фьючерсы: treemap по группам, базис, терм-структура, (open interest).
- Облигации: кривая доходности, разбивка по эмитентам/секторам, обороты.

## Фаза 6 — Представление 4 (Сумма всех)
- Общий оборот (gauge), donut долей, stacked area во времени, Sankey перетоков.

## Фаза 7 — Live-функции
- Стрим вотчлиста (свечи/стакан/лента), Time&Sales, DOM, алёрты, replay-режим.

## Фаза 8 — Полировка и сборка
- Упаковка MSI/NSIS (Tauri bundler), производительность, обработка ошибок, настройки.
