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

## Фаза 9 — Дизайн-итерация: оболочка + Сводка / Бэктест / Торговля

Источник правды — экспорт из Claude Design (`frontend/design/`,
см. `frontend/design/README.md`). Эта фаза приводит фронт к дизайну (рабочие
пространства в левом рейле, глобальный период, статус READ-ONLY) и добавляет три
новые вкладки. Бэкенд новых вкладок сведён из `frontend/design/roadmap_design.md`.

### 9.0 — Оболочка по дизайну ✅
- ✅ `App.svelte` переписан в shell: верхняя панель (бренд, глобальный период
  `1Д/1Н/1М/3М/YTD/1Г`, LIVE, READ-ONLY), левый рейл из 10 рабочих пространств,
  статус-бар. Период считает окно `[fromTs,toTs]` и переинициирует аналитику.
- ✅ Существующие панели разнесены по пространствам (Обзор/Секторы/Потоки/Лента/
  Фьючерсы/Облигации/Настройки) без потери функций.

### 9.1 — Сводка (Summary) — «куда идут большие деньги» ✅ (ядро) / ⏳ (FX-ингест)
- ✅ Новый класс актива `AssetClass::Fx` (валютный спот) — `code/from_code/ALL`,
  учитывается в donut/gauge/timeline/Sankey (доля 0, пока нет FX-данных).
- ✅ Чистый классификатор `domain::metrics::regime`: Risk-ON/OFF/Neutral +
  conviction (0..100) по нетто-потокам классов; детерминирован, юнит-тесты.
- ✅ IPC `summary(fromTs,toTs) -> RegimeSignalDto` (`api::summary` →
  `class_net_flow` + `assess_regime`); регистрация в Tauri; зеркало в
  `frontend` (`types`/`ipc`/`mock`) + компонент `SummaryPanel.svelte`
  (режим, conviction, нетто-потоки по классам, решения, риски).
- ⏳ Ингест FX-спота в `data` (`classify.rs`/`market.rs`, борд `CETS`,
  USD/RUB·CNY/RUB·EUR/RUB): сейчас `CURRENCY → None`. После ингеста сигнал
  начнёт учитывать реальные FX-потоки без правок аналитики.
- ⏳ Обогащение драйверов сигнала (breadth/CVD/MFI/кривая ОФЗ/RUB) в `regime`.

### 9.2 — Бэктест (Backtest) — ⏳ прототип без бэкенда
- ✅ Фронт `BacktestPanel.svelte`: пресеты, параметры, кривая капитала vs IMOEX,
  статистика (CAGR/Sharpe/MaxDD/винрейт/сделки/бенчмарк), доходность по месяцам.
  Результат симулируется детерминированным сид-генератором в UI.
- ⏳ Бэкенд (`domain/backtest/` или новый крейт): `StrategyDef`, event-driven
  движок по сохранённым барам (DuckDB), метрики (CAGR/Sharpe/Sortino/MaxDD/
  винрейт/profit factor/экспозиция/сделки/мес. матрица); IPC
  `backtest_run(StrategyDef,fromTs,toTs) -> BacktestResult` (+ `backtest:tick`);
  бенчмарк IMOEX и запрос OHLCV по вселенной с датным диапазоном в `storage`.

### 9.3 — Торговля (Live trading) — ⏳ прототип, нарушает READ-ONLY
- ✅ Фронт `TradePanel.svelte`: тикет заявки (купля/продажа, тип, лоты, цена,
  оценка), позиции, активные заявки, счёт. Сабмит отклоняется (READ-ONLY).
- ⏳ Бэкенд за явным флагом `trading` (READ-ONLY по умолчанию):
  `AccountsService`/`OrdersService` Finam (place/cancel/replace, статусы,
  позиции, портфель, покуп. способность); доменная валидация заявок и P&L;
  IPC `place_order`/`cancel_order`/`positions`/`working_orders`/
  `account_summary` + события `order:update`/`position:update`/`account:update`;
  безопасность: подтверждение, rate-limit, kill-switch, paper-trading,
  аудит-лог, секреты в ОС-keyring.

### 9.x — Сквозное
- ⏳ Реальные per-panel период-контролы (как в дизайне) поверх глобального.
- ⏳ Бамп схемы `storage` при добавлении FX (новый класс в DTO/Sankey/donut).
- Дисциплина слоёв неизменна: `domain` чистый (regime/backtest CI-тестируемы),
  адаптеры — в `data`/`storage`/`app`.
