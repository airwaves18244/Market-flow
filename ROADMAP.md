# ROADMAP — Market Terminal

Пошаговый план развития. Отметка ✅ — сделано в текущей итерации.

> Гранулярный, отслеживаемый по чекбоксам перечень задач с критериями
> готовности (DoD) и привязкой к слоям — в [`task_list.md`](./task_list.md).
> Нумерация фаз и подпунктов в обоих документах совпадает (напр. § 0.1, § 1.3).

Слои workspace: **`finam-proto`** · **`data`** · **`storage`** · **`domain`** ·
**`app`** · **`frontend`** (см. `README.md`).

---

## Фаза 0 — Фундамент ✅ (частично)
- ✅ Cargo workspace, члены: `finam-proto`, `domain`, `data`, `storage`, `app`.
- ✅ Дисциплина слоёв (аналитика в `domain` без внешних зависимостей).
- ✅ Контракты `data` (трейт `MarketData`, ошибки, `TimeFrame`), классификация секторов.
- ✅ DDL схемы DuckDB в `storage::schema`.
- ✅ § 0.1 gRPC-стабы: vendored proto `FinamWeb/finam-trade-api` (@3cec0896),
  кодоген клиентов через `protox` + `tonic-prost-build` (без системного protoc).
- ⏳ Сетевой каркас gRPC — открытые подзадачи:
  - § 0.2 транспорт (`rustls`/HTTP-2), auth + refresh токена, `keyring` для ключа.
  - § 0.3 per-method rate-limiter (`governor`, ≤200/мин), reconnect стримов, `tracing`.
  - § 0.4 реализация трейта `MarketData` (`assets`/`bars`/`last_quote`/`latest_trades`).

## Фаза 1 — Хранилище и ингест (`storage`) ✅ (частично)
- ✅ § 1.1 Нативный `duckdb` (bundled): `Db::open`/`open_in_memory`, схема при старте.
- ✅ § 1.2 Миграции (`schema_migrations`, атомарный идемпотентный раннер).
- ✅ § 1.3 Writer ингеста: upsert инструментов, батч-инсерт баров/снимков (ON CONFLICT).
- ⏳ § 1.4 Планировщик: примитив `app::ingest::backfill_symbols` есть; фоновый цикл
  и rate-limit — с реальным `data` (Фаза 0).
- ✅ § 1.5 Классификация: `sector_map` + проставление сектора (тикер > ISIN).
- ⏳ § 1.6 Бэкфилл: функция `app::ingest::backfill_bars` есть; пагинация — с `data`.
- ✅ § 1.7 Аналитические запросы (оборот по секторам, топ-движения, ряды нетто-потока).
- Проверено: 10/10 тестов хранилища (юнит + интеграционные против DuckDB).

## Фаза 2 — Аналитика (`domain`) ✅
- ✅ turnover, directional turnover, unusual volume.
- ✅ money flow, MFI, CVD.
- ✅ breadth (A/D, % растущих).
- ✅ sector rollups (взвешенные по обороту).
- ✅ RRG (RS-Ratio / RS-Momentum, квадранты).
- ✅ cross-asset shares + flow matrix (Sankey).
- ⏳ Поддерживающее: property-тесты инвариантов, бенчмарки (`criterion`).

## Фаза 3 — Tauri-оболочка + каркас фронта (`app` + `frontend`) ⏳ (частично)
> Рантайм `tauri` требует webkit2gtk (нет в headless-CI), поэтому в воркспейс не
> добавлен; собрана независимая от `tauri` часть, сам бинарь — на десктоп-цели.
- ⏳ § 3.1 App state: lib-таргет + `app::ingest`; запуск Tauri — на десктопе.
- ✅ § 3.2 View-model `app::api` (`equity_dashboard`, `flow_series`) — без `tauri`,
  обёртки `#[tauri::command]` добавляются на десктоп-цели.
- ⏳ § 3.3 События (live-push) — с реальными стримами `data`.
- ✅ § 3.4 Каркас фронта: Vite + Svelte 5 + TS, ECharts, тёмная тема, типизированный
  `invoke` с моками. Проверено: `svelte-check` (0 ошибок) + `vite build`.

## Фаза 4 — Представление 1 (Акции / секторы) ✅ (каркас)
- ✅ Докуемые панели (dockview-core, тёмная тема) + реактивный стор.
- ✅ treemap секторов, heatmap, breadth (gauge), топ-движения, RRG (квадранты).
- ⏳ Данные breadth/RRG пока из моков — нужны команды `app::api` (§ 3.2);
  виртуализация таблиц (TanStack), свечи (Lightweight Charts).
- Проверено: `svelte-check` (0 ошибок) + `vite build`.

## Фаза 5 — Представления 2 и 3 (Фьючерсы, Облигации)
- § 5.1 Фьючерсы: treemap по группам, базис, терм-структура, (open interest).
- § 5.2 Облигации: кривая доходности, разбивка по эмитентам/секторам, обороты.

## Фаза 6 — Представление 4 (Сумма всех)
- общий оборот (gauge), donut долей, stacked area во времени, Sankey перетоков.

## Фаза 7 — Live-функции
- стрим вотчлиста (свечи/стакан/лента), Time & Sales, DOM, алёрты, replay-режим.

## Фаза 8 — Полировка и сборка
- упаковка MSI/NSIS (Tauri bundler), производительность, обработка ошибок, настройки.

---

## Сквозные задачи (вне фаз)
- CI: `cargo test --workspace`, `cargo clippy --workspace -D warnings`, сборка/линт фронта.
- Тесты адаптеров `data`/`storage` на golden-фикстурах ответов API.
- `tracing` с уровнями, без утечки секретов.
- Синхронность `ROADMAP.md` ↔ [`task_list.md`](./task_list.md).

Детализация каждого подпункта (под-задачи + DoD) — в [`task_list.md`](./task_list.md).
</content>
