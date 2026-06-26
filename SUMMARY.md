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

## Следующее (Фаза 4)
Представление «Акции/секторы»: treemap (размер=оборот, цвет=%изм), heatmap,
breadth, топ-движения, RRG. Плюс асинхронный планировщик ингеста поверх
`data::MarketData` и `storage::Store`, полноценный dockview.
