# SUMMARY — статус проекта Market Terminal

Живой срез прогресса по фазам (детальный план — в `ROADMAP.md`).

## Готово

### Фаза 0 — Фундамент (частично)
- Cargo workspace: `finam-proto`, `domain`, `data`, `storage`, `app`.
- Дисциплина слоёв: вся математика в `domain`, без внешних зависимостей.
- Контракты `data` (`MarketData`, `TimeFrame`, ошибки), DDL DuckDB.
- ⏳ Осталось: gRPC-стабы из `.proto`, auth+refresh, rate-limiter, keyring, tracing.

### Фаза 2 — Аналитика (`domain`)
- turnover / directional turnover / unusual volume; money flow / MFI / CVD;
  breadth; sector rollups; RRG; cross-asset shares + flow matrix.
- Полное покрытие юнит-тестами.

### Фаза 1 — Хранилище и ингест
- **Схема и миграции**: `schema` (DDL) + `migrate` (версия схемы,
  идемпотентный прогон, `SCHEMA_VERSION = 1`).
- **Контракт `Store`** с двумя реализациями:
  - `MemStore` — в памяти; всегда доступна, кросс-платформенно тестируется в CI;
  - `DuckStore` — нативный DuckDB за фичей `duckdb` (bundled).
- **Ингест** (`ingest`): `Writer` (бары/снимки/инструменты, классификация),
  `snapshot_from_bars` (оборот + net flow + изменение из серии баров),
  `BatchCursor` (round-robin планировщик батч-поллинга под лимит ~200 req/min).
- **Бэкфилл** (`backfill`): `plan_backfill` (дозагрузка «хвоста» истории) и
  `chunk_range` (нарезка на страницы под лимит баров на запрос).
- **Типы**: `TurnoverSnapshot`, `SectorEntry`; `TimeFrame` перенесён в `domain`
  (с `code`/`from_code`/`seconds`) и переэкспортирован из `data`.

### Фаза 3 — Tauri-оболочка + каркас фронта
- **Ядро IPC** (`crates/app`): `AppState` поверх `Store`; DTO (camelCase);
  обработчики `instruments`, `bars`, `turnover_series`, `sector_rollup`,
  `sector_map` — чистые, протестированы на `MemStore`.
- **Привязка Tauri** за фичей `tauri`: `#[tauri::command]`-обёртки, заготовка
  событий live-push, `tauri.conf.json` + capabilities + `build.rs`. Десктоп-
  сборка требует webkit2gtk → вне кросс-платформенного CI.
- **Фронт** (`frontend`): Vite + Svelte 5 + TS, тёмная тема, каркас докуемых
  панелей, ECharts treemap + Lightweight Charts свечи, типизированный IPC-клиент
  с мок-режимом (UI работает в браузере без бэкенда). `npm run build` и
  `svelte-check` — зелёные.

## Проверка
```bash
cargo test --workspace                  # ядро + хранилище + IPC (MemStore), без C++/Tauri
cargo test -p storage --features duckdb # + нативный DuckDB (bundled)
cargo run -p app                        # smoke: domain → storage → dto
cd frontend && npm install && npm run build   # сборка фронта (мок-данные)
```

## Готово (продолжение)

### Фаза 4 — Представление 1 (Акции/секторы) ✅
- **API-обработчики** в `crates/app/src/api.rs`:
  - `breadth_data()` — статистика advancers/decliners/A/D ratio из снимков оборота;
  - `top_movers()` — инструменты с наибольшим |изменением|, отсортированы, limit;
  - `rrg_sectors()` — позиция секторов на плоскости RS-Ratio vs RS-Momentum с квадрантами.
- **DTO расширения** в `crates/app/src/dto.rs`:
  - `BreadthDto`, `TopMoverDto`, `RrgSectorDto`.
- **Tauri команды**: регистрация в `crates/app/src/tauri_app.rs`.
- **Frontend компоненты**:
  - `BreadthIndicator.svelte` — карточка с метриками ширины рынка;
  - `TopMoversTable.svelte` — таблица топ-движений;
  - `HeatmapChart.svelte` — ECharts heatmap по секторам (в %) и изменениям;
  - `RrgChart.svelte` — scatter-RRG с квадрантами и легендой.
- **Интеграция**: App.svelte загружает все новые данные и показывает панели в расширенной сетке.

### Фаза 5 — Представления 2 и 3 (Фьючерсы, Облигации) ✅
- **Store расширение** в `crates/storage/src/store.rs`:
  - `instruments_by_asset_class()` — фильтр инструментов по классу активов.
- **API-обработчики** в `crates/app/src/api.rs`:
  - `futures_rollup()` — агрегация фьючерсов по префиксам (группы контрактов);
  - `bonds_rollup()` — агрегация облигаций по эмитентам (префиксы);
  - `yield_curve()` — кривая доходности по стандартным срокам.
- **DTO расширения** в `crates/app/src/dto.rs`:
  - `FutureGroupDto`, `BondIssuerDto`, `YieldCurvePoint`.
- **Tauri команды**: регистрация всех трёх обработчиков.
- **Frontend компоненты**:
  - `FuturesTreemap.svelte` — ECharts treemap с контрактами по группам;
  - `YieldCurve.svelte` — линейный график yield по срокам;
  - `BondsTable.svelte` — таблица эмитентов с yield и duration.
- **Mock данные**: полный набор для разработки без бэкенда.

## Следующее (Фаза 6 и далее)
- Фаза 6: Представление 4 (общий оборот, gauge, stacked area, Sankey).
- Фаза 7: Live-функции (стрим вотчлиста, Time&Sales, DOM, алёрты, replay).
- Фаза 8: Асинхронный планировщик ингеста, полноценный dockview, полировка.
