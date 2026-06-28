# ROADMAP — Market Terminal

Пошаговый план. Отметка ✅ — сделано в текущей итерации.

## Фаза 0 — Фундамент ✅
- ✅ Cargo workspace, члены: `finam-proto`, `domain`, `data`, `storage`, `app`.
- ✅ Дисциплина слоёв (аналитика в `domain` без внешних зависимостей).
- ✅ Контракты `data` (трейт `MarketData`, ошибки, `TimeFrame`), классификация секторов.
- ✅ DDL схемы DuckDB в `storage::schema`.
- ✅ Per-method rate-limiter: чистый, без внешних зависимостей, скользящее окно
  по каждому методу (`data::RateLimiter`, лимит Finam 200 req/min по умолчанию),
  кросс-платформенно протестирован.
- ✅ Учёт авторизации: `data::TokenState` отслеживает короткоживущий JWT и его
  срок, решает об упреждающем refresh (с запасом-skew) — чистая, тестируемая
  логика; сетевой обмен `AuthService.Auth` подключается в фазе интеграции.
- ✅ Повторы: `data::Backoff` — экспоненциальный backoff с потолком, полным
  джиттером и классификацией ретраябельности ошибок (`DataError::is_retryable`).
- ✅ `tracing`: инициализация подписчика в `app::telemetry::init` (фильтр уровней
  из `RUST_LOG`, по умолчанию `info`), стартовый структурированный лог.
- ✅ Канонические методы API: `data::Method` (Auth/Assets/Bars/LastQuote/
  LatestTrades) — единый источник имён для ключей лимитера и меток трейсинга;
  `RateLimiter` принимает `Method` напрямую.
- ✅ Хранилище секрета: контракт `data::SecretStore` + in-memory `MemSecretStore`
  (тестируемо кросс-платформенно).
- ✅ ОС-keyring реализация `SecretStore`: `data::KeyringSecretStore` за фичей
  `keyring` — нативный бэкенд под платформу (Windows → Credential Manager,
  macOS → Keychain, Linux → ключи ядра/keyutils). Фича выключена в кросс-
  платформенном CI (как `duckdb`/`tauri`), зависимость не подтягивается.
  Контрактный тест компилируется всегда; live-roundtrip помечен `#[ignore]`
  (нужна реальная keyring-сессия).
- ✅ gRPC-кодоген: `finam-proto` генерирует клиентские стабы из vendored
  `.proto` (`proto/`, санитизированные копии из `FinamWeb/finam-trade-api`:
  `AuthService`, `AssetsService`, `MarketDataService`, `side`, плюс
  `google/type/*`) через `tonic-build` + `protoc-bin-vendored` (свой `protoc`,
  без системного) — за фичей `grpc`. По умолчанию фича выключена: тяжёлые
  `tonic`/`prost` и build-tooling не подтягиваются (лёгкий CI, как
  `duckdb`/`tauri`).
- ✅ Сетевой обмен auth (`AuthService.Auth`): `data::AuthManager` +
  `data::GrpcAuthTransport` за фичей `grpc`. Менеджер связывает чистые примитивы
  (`TokenState`/`RateLimiter`/`Backoff`/`SecretStore`): переиспользует
  действующий JWT, упреждающе обновляет, держит лимит метода `Auth`, повторяет
  транзиентные сбои с backoff и не повторяет ошибки авторизации. Транспорт
  отделён трейтом `AuthTransport`, поэтому оркестрация покрыта тестами без сети.
- ✅ Реализация `MarketData` поверх gRPC: `data::FinamMarketData` за фичей
  `grpc` реализует `assets`/`bars`/`last_quote`/`latest_trades`. Каждый вызов
  берёт JWT у `AuthManager` (метаданные `authorization`), держит per-method
  лимит, повторяет транзиентные сбои с backoff и переводит протобаф-типы
  (Decimal/Timestamp/Side) в чистые доменные значения. Маппинг вынесен в чистые
  функции и покрыт тестами; сетевые вызовы интеграционно проверяются при наличии
  реального секрета (в CI выключено).

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
- ✅ Асинхронный цикл опроса: `app::ingest::IngestService` (фича `ingest`)
  связывает источник `data::MarketData` с хранилищем через `AppState`. Такт
  (`tick`) обходит вотчлист круговым `BatchCursor`, держит per-method лимит
  (`RateLimiter`), тянет бары и пишет их со снимком оборота; боевой цикл (`run`)
  крутит такты по таймеру tokio. Источник абстрактный (трейт `MarketData`) —
  такт покрыт тестами на фейке (без сети). Боевой источник — `FinamMarketData`
  (фича `grpc`); подключение к живому API требует секрета (вне CI).

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
- ✅ Асинхронный планировщик ингеста — `app::ingest` (фича `ingest`), см. фазу 1.
- ⏳ Полноценный dockview (фронт) — в фазе полировки.

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

## Фаза 7 — Live-функции
- Стрим вотчлиста (свечи/стакан/лента), Time&Sales, DOM, алёрты, replay-режим.

## Фаза 8 — Полировка и сборка
- Упаковка MSI/NSIS (Tauri bundler), производительность, обработка ошибок, настройки.
