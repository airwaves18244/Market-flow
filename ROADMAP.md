# ROADMAP — Market Terminal

Пошаговый план. Отметка ✅ — сделано в текущей итерации.

## Статус верификации (ревизия 2026-07)
Фазы 0–9 реализованы и проверены на зелёном CI-контуре:
- `cargo test --workspace` — 244 теста зелёные (в т.ч. `--features ingest` — 40);
- `cargo fmt --all --check` и `cargo clippy --workspace -- -D warnings` — чисто;
- фронт: `svelte-check` — 0 ошибок, `vitest` — 16 тестов, `vite build` — ок;
- IPC-контракт согласован end-to-end: одни и те же 26 команд в `frontend/src/lib/mock.ts`,
  `frontend/src/lib/ipc.ts` и регистрации `crates/app/src/tauri_app.rs`.

Фиче-гейтед сборки (`ingest`, `live-trading`, `grpc` с vendored protoc) компилируются;
`duckdb` собирается штатно (bundled C++ — долгая сборка), `tauri` требует webkit2gtk.

Оставшиеся ниже пункты `⏳` — не незавершённая инженерия, а шаги, требующие
десктопного окружения (упаковка MSI/NSIS) или полировки (полный dockview); они
вне кросс-платформенного CI по конструкции.

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
  такт покрыт тестами на фейке (без сети).
- ✅ Подключение реального источника: боевой режим `app` (фича `live`) связывает
  `FinamMarketData` (gRPC) с планировщиком — авторизация → справочник → цикл
  опроса баров в хранилище. Секрет берётся из переменной окружения
  `FINAM_API_SECRET`, файла `.env` (ключи `FINAM_API_SECRET`/`FINAM_SECRET`, без
  учёта регистра; файл в `.gitignore`) или ОС-keyring (фича `keyring`). Загрузчик
  `.env` — чистый парсер без зависимостей (`data::dotenv`, покрыт тестами).
  Требует egress-доступа к `trade-api.finam.ru:443`; проверено до сетевой границы
  (с секретом из `.env` пайплайн доходит до allowlist-проверки egress). Живой
  smoke пайплайна — `cargo run -p data --features grpc --example live_check`.

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

## Фаза 7 — Live-функции ✅
- ✅ Транспорт live-стримов: `data::stream` (фича `grpc`) — хэндлы
  `QuoteStream`/`TradeStream`/`BarStream` поверх серверных стримов
  `MarketDataService.Subscribe*` с авторизацией и переводом протобаф→домен;
  методы `FinamMarketData::subscribe_quotes`/`subscribe_trades`/`subscribe_bars`.
- ✅ Авто-reconnect: `data::StreamReconnect` — экспоненциальная пауза с
  джиттером до потолка, сброс после успешных данных (стрим Finam рвётся ~раз в
  24 ч). Чистый контроллер и маппинг сообщений (включая `StreamError`) покрыты
  тестами без сети; сам стрим интеграционно проверяется при наличии секрета.
- ✅ DOM (стакан): доменный `OrderBook`/`BookLevel` + `FinamMarketData::order_book`
  (`MarketDataService.OrderBook`) с маппингом строк в биды/аски (сортировка,
  спред); метод `Method::OrderBook` для лимита. Маппинг покрыт тестами.
- ✅ Алёрты: `domain::metrics::alerts` — `AlertEngine` с фронтовым срабатыванием
  (цена/изменение, без повторного спама), чистый и протестированный.
- ✅ Replay-режим: `app::replay::ReplaySource` реализует `MarketData` из
  сохранённых баров (в т.ч. `from_store`) — тот же путь ингеста/аналитики без
  сети; покрыт тестами. Time&Sales — это лента `subscribe_trades`.
- ✅ Фронтовые панели Time&Sales/DOM/алёртов: компоненты `TimeSales`,
  `OrderBook` (лесенка с барами глубины и спредом), `AlertsPanel` (правила +
  срабатывания). DTO `TradeDto`/`OrderBookDto`/`AlertEventDto` + вход
  `AlertRuleInput`; команда `alerts_scan` (прогон правил по сохранённым барам)
  и команды-контракты `latest_trades`/`order_book`. Живые обновления —
  события `trade:tick`/`orderbook:tick` (эмиттеры `emit_trade`/
  `emit_order_book`), фронт подписывается через `onTrade`/`onOrderBook`
  (в браузере — мок-снимок). DTO-маппинг и `alerts_scan` покрыты тестами.

## Фаза 8 — Полировка и сборка
- ✅ Настройки представления: `frontend/src/lib/settings.ts` (localStorage) +
  панель `SettingsPanel` — глубина стакана, размер ленты сделок, лимит
  топ-движений; изменения сохраняются и перезагружают зависимые данные.
- ✅ Производительность фронта: разнесение тяжёлых библиотек (ECharts,
  Lightweight Charts) в отдельные кешируемые чанки (`manualChunks`) — код
  приложения ужался с ~1.27 МБ до ~73 КБ.
- ✅ Конфигурация упаковки: метаданные бандла в `tauri.conf.json`
  (издатель, категория, описания, NSIS-языки RU/EN), цели `msi`/`nsis`.
- ✅ Обработка ошибок: верхнеуровневый баннер ошибки + локальные состояния
  ошибок/пустоты в панелях (алёрты, стакан, лента).
- ⏳ Финальная сборка MSI/NSIS (`cargo tauri build`) и иконки — требуют
  десктопного окружения (webkit2gtk) вне кросс-платформенного CI.

## Фаза 9 — V2: Бэктестер, Торговля, Delta ✅
Превращение терминала из «обзора» в «рабочую станцию» (по образцу CQG/MultiCharts).
Новый каркас фронта — верхние вкладки: **Обзор · Delta · Торговля · Бэктест**.

- ✅ Хранилище тиковой ленты: таблица `trades` (схема v2), `Store::insert_trades`/
  `trades`, `Writer::trades` — основа footprint/дельты и заполнения симулятора.
- ✅ Бэктестер (`domain::backtest`): трейт `Strategy`, детерминированный движок
  (позиция/комиссия/слиппедж), отчёт (P&L, win-rate, profit factor, просадка,
  Sharpe) и библиотека стратегий — «известные сценарии» (`ma_cross`, `same_lot`,
  `iceberg`, `cvd_momentum`) по id + параметрам.
- ✅ Delta (`domain::delta`): footprint (объём по ценам и сторонам агрессора,
  дельта/CVD) и детектирующие роботы — равные лоты, айсберг (по стакану),
  поглощение. Только анализ/визуализация (роботы не торгуют).
- ✅ Симулятор торговли (`domain::trading`): заявки/исполнения, счёт и позиции
  (средняя цена, реализованный/нереализованный P&L), предторговый риск и
  `SimBroker` (рыночные проходят стакан, лимитки стоят и исполняются на ленте,
  стопы срабатывают по пробою). Paper trading — движок исполнения V2.
- ✅ Слой `app`: чистые обработчики `list_strategies`/`run_backtest`/
  `delta_footprint`/`robot_scan`, сессия торговли `TradeSession` в `AppState`
  (`submit_order`/`cancel_order`/`order_blotter`/`positions`/`account`), DTO,
  команды Tauri и канал событий `fill:tick`.
- ✅ Фронт: вкладки (`TabBar`, `Overview`), `Backtester` (пикер стратегий +
  форма параметров + кривая капитала + метрики + сделки), `DeltaView`
  (`DeltaChart`: свечи + гистограмма дельты + накопленная дельта + маркеры
  роботов; footprint-лесенка; переключатели роботов), `TradePanel` (кликабельный
  DOM-стакан + тикет + блоттер + позиции + счёт). Типизированный IPC + мок-режим
  (включая мок-симулятор) — все вкладки работают в браузере без бэкенда.
- ✅ Каркас боевого роутинга: контракт `data::OrderRouter` + `SimOrderRouter`
  (по умолчанию) и заглушка `FinamOrderRouter` за фичей `live-trading`
  (по умолчанию выключена; реальный gRPC `OrderService`/`AccountsService` —
  отдельная интеграция).

## Фазы 10–12 — MOEX ALGO · Историзация · Опционы ✅ (кроме сетевых слоёв)
Детальный план — в **`ROADMAP_PHASE_10-12.md`**; бриф фронта —
`design/claude-design-brief.md`.

Реализовано end-to-end на mock/локальных данных:
- ✅ Доменные ядра: `domain::algo` (Super Candles, FUTOI, HI2, Mega Alerts),
  `domain::keyactivity` (правила + LLM-промпт + fallback-свод),
  `domain::history` (каталог датасетов, `missing_ranges`),
  `domain::options` (Блэк-76/Башелье, греки, IV, улыбки MOEX/SABR/SVI/
  Каленкович + калибратор Нелдера–Мида, конструктор стратегий).
- ✅ App/IPC: обработчики + команды Tauri для всех трёх вкладок.
- ✅ Frontend: вкладки **MOEX ALGO** (`MoexAlgoTab`, `KeyActivityTable`,
  `KeyActivitySummary`), **Данные** (`HistoryTab`, `DatasetManager`),
  **Опционы** (`OptionsTab`, `OptionCalculator`, `SmileView`/`SmileChart`,
  `StrategyBuilder`, `PayoffChart`) — типизированный IPC + мок-режим.
- ⏳ Сетевые слои: транспорт `data::moex` (ALGOPACK/ISS), `http`, `llm`;
  таблицы `storage` для датасетов; блоки `(verify)` — фиксация контракта
  ALGOPACK по живым фикстурам. Требуют egress-доступа/ключей.

## V3 — целевое состояние (state of the art)
Продуктовая спецификация и план работ следующего мажорного этапа:
- **`V3/PRD.md`** — детальный PRD терминала уровня state of the art
  (аналитика + торговля): рабочие пространства, скринер, ликвидность/heatmap,
  боевой роутинг заявок, риск-менеджмент, walk-forward бэктест, LLM-копайлот,
  производительность и NFR.
- **`V3/TODO.md`** — декомпозиция на задачи с распределением по моделям Claude
  (Fable 5 / Opus 4.8 / Sonnet 5 / Haiku 4.5) и уровням усилий.
