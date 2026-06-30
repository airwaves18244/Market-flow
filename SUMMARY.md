# SUMMARY — статус проекта Market Terminal

Живой срез прогресса по фазам (детальный план — в `ROADMAP.md`).

## Готово

### Фаза 0 — Фундамент ✅
- Cargo workspace: `finam-proto`, `domain`, `data`, `storage`, `app`.
- Дисциплина слоёв: вся математика в `domain`, без внешних зависимостей.
- Контракты `data` (`MarketData`, `TimeFrame`, ошибки), DDL DuckDB.
- `data::RateLimiter` — per-method ограничитель частоты (скользящее окно, лимит
  Finam 200 req/min по умолчанию), без внешних зависимостей, покрыт тестами.
- `data::TokenState` — учёт короткоживущего JWT и решение об упреждающем refresh
  (с запасом-skew); чистая, без сети, покрыта тестами.
- `data::Backoff` — экспоненциальный backoff с потолком/джиттером и
  `DataError::is_retryable`; чистый расчёт задержек, покрыт тестами.
- `data::Method` — канонические имена методов API (ключи лимитера + метки
  трейсинга); `RateLimiter` принимает `Method` напрямую.
- `data::SecretStore` + `MemSecretStore` — контракт хранилища API-секрета и
  in-memory реализация; покрыто тестами.
- `data::KeyringSecretStore` — боевое хранилище секрета поверх ОС-keyring за
  фичей `keyring` (нативный бэкенд под платформу: Credential Manager / Keychain /
  ключи ядра Linux). Фича выключена в кросс-платформенном CI, зависимость не
  тянется. Контрактный тест компилируется всегда; live-roundtrip — `#[ignore]`.
- `app::telemetry::init` — установка подписчика `tracing` (фильтр из `RUST_LOG`,
  по умолчанию `info`), идемпотентна; стартовый структурированный лог в `main`.
- `finam-proto` — gRPC-кодоген за фичей `grpc`: клиентские стабы из vendored
  `.proto` (`proto/`: `AuthService`/`AssetsService`/`MarketDataService` +
  `google/type/*`) через `tonic-build` + `protoc-bin-vendored` (свой `protoc`).
  По умолчанию фича выключена, `tonic`/`prost` не подтягиваются.
- `data::AuthManager` + `data::GrpcAuthTransport` — сетевой обмен `AuthService.Auth`
  за фичей `grpc`. Связывает `TokenState`/`RateLimiter`/`Backoff`/`SecretStore`:
  кэш токена, упреждающий refresh, лимит метода `Auth`, повтор транзиентных
  сбоев с backoff (без повтора auth-ошибок). Транспорт за трейтом `AuthTransport`
  → оркестрация покрыта тестами без сети.
- `data::FinamMarketData` — реализация трейта `MarketData` поверх gRPC за фичей
  `grpc`: `assets`/`bars`/`last_quote`/`latest_trades` с JWT в `authorization`,
  per-method лимитом, повтором транзиентных сбоев и маппингом протобаф→домен
  (Decimal/Timestamp/Side). Чистые функции маппинга покрыты тестами.
- Фаза 0 завершена; стримовые `Subscribe*` относятся к фазе 7 (live).

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
- **Асинхронный цикл опроса** (`app::ingest`, фича `ingest`): `IngestService`
  связывает `data::MarketData` → `AppState`/`Store`. Такт обходит вотчлист
  круговым `BatchCursor` под per-method лимитом, тянет бары и пишет снимок
  оборота; боевой цикл `run` — по таймеру tokio. Источник за трейтом → такт
  покрыт тестами на фейке (без сети).
- **Боевой режим** (`app::live`, фича `live`): авторизация по
  `FINAM_API_SECRET`/keyring → `FinamMarketData` → справочник инструментов →
  цикл ингеста. Секрет в репозиторий/логи не попадает; нужен egress-доступ к
  `trade-api.finam.ru:443`. Пайплайн проверен живым smoke (`example live_check`)
  до сетевой границы окружения.

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
cargo fmt --all --check                 # формат (как в CI)
cargo clippy --workspace -- -D warnings # линт без предупреждений (как в CI)
cargo test --workspace                  # ядро + хранилище + IPC (MemStore), без C++/Tauri
cargo test -p storage --features duckdb # + нативный DuckDB (bundled)
cargo test -p data --features keyring   # + ОС-keyring (live-roundtrip: --ignored)
cargo test -p data --features grpc      # + gRPC auth-обмен (оркестрация без сети)
cargo test -p app --features ingest     # + асинхронный планировщик ингеста
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

### Фаза 6 — Представление 4 (Сумма всех) ✅
- **API-обработчики** в `crates/app/src/api.rs` (поверх `domain::metrics::crossasset`):
  - `cross_asset_summary()` — общий оборот + доли классов (gauge + donut);
  - `turnover_timeline()` — оборот по классам во времени (stacked area);
  - `flow_sankey()` — перетоки долей между классами (первая↔последняя точка окна).
- **DTO расширения** в `crates/app/src/dto.rs`:
  - `CrossAssetSummaryDto`, `AssetClassShareDto`, `TurnoverByClassPoint`, `FlowEdgeDto`.
- **Tauri команды**: `cross_asset_summary`, `turnover_timeline`, `flow_sankey`.
- **Frontend компоненты**:
  - `TotalTurnoverGauge.svelte` — gauge общего оборота (млрд ₽);
  - `SharesDonut.svelte` — donut долей по классам;
  - `TurnoverStackedArea.svelte` — stacked area оборота по классам;
  - `FlowSankey.svelte` — Sankey перетоков долей (с пустым состоянием);
  - общий `lib/assetClass.ts` — русские подписи и цвета классов.
- **Тесты**: агрегация по классам, таймлайн, Sankey (включая определение сдвига долей).

### Фаза 7 — Live-функции ✅
- **Транспорт стримов** (`data::stream`, фича `grpc`): хэндлы
  `QuoteStream`/`TradeStream`/`BarStream` поверх серверных стримов
  `MarketDataService.Subscribe*` (авторизация + перевод протобаф→домен);
  методы `FinamMarketData::subscribe_quotes`/`subscribe_trades`/`subscribe_bars`.
- **Авто-reconnect** (`data::StreamReconnect`): экспоненциальная пауза с
  джиттером до потолка, сброс после успешных данных (стрим рвётся ~раз в 24 ч).
- **DOM (стакан)**: доменный `OrderBook`/`BookLevel` + `FinamMarketData::order_book`
  (`Method::OrderBook`); маппинг строк в биды/аски с сортировкой и спредом.
- **Алёрты**: `domain::metrics::alerts::AlertEngine` — фронтовое срабатывание по
  цене/изменению (без повторного спама), чистый и протестированный.
- **Replay**: `app::replay::ReplaySource` реализует `MarketData` из сохранённых
  баров (`from_store`) — тот же путь ингеста/аналитики offline.
- **Тесты**: политика повторов стрима и маппинг сообщений (вкл. `StreamError`);
  маппинг стакана; движок алёртов (edge-trigger); replay (окно/last_quote/from_store).
- **Фронтовые панели**: `TimeSales` (лента сделок), `OrderBook` (DOM-лесенка
  с барами глубины и спредом), `AlertsPanel` (правила цена/изменение +
  срабатывания). DTO `TradeDto`/`OrderBookDto`/`AlertEventDto` + вход
  `AlertRuleInput`; команда `alerts_scan` (прогон правил по сохранённым барам)
  и команды-контракты `latest_trades`/`order_book`. Живые обновления —
  события `trade:tick`/`orderbook:tick` (`emit_trade`/`emit_order_book`),
  фронт подписывается `onTrade`/`onOrderBook` (в браузере — мок-снимок).

### Фаза 8 — Полировка и сборка ✅ (кроме финальной упаковки)
- **Настройки** (`frontend/src/lib/settings.ts` + `SettingsPanel`): глубина
  стакана, размер ленты сделок, лимит топ-движений — в localStorage; изменение
  перезагружает зависимые данные.
- **Производительность фронта**: тяжёлые библиотеки (ECharts, Lightweight
  Charts) вынесены в отдельные чанки (`manualChunks`) — код приложения с
  ~1.27 МБ до ~73 КБ.
- **Упаковка**: метаданные бандла в `tauri.conf.json` (издатель, категория,
  описания, NSIS RU/EN), цели `msi`/`nsis`. Финальная сборка `cargo tauri build`
  и иконки требуют десктопного окружения (webkit2gtk) — вне CI.
- **Ошибки**: верхнеуровневый баннер + локальные состояния пустоты/ошибки в
  панелях.

### Фазы 10–12 — доменное ядро ✅ (сетевые/UI-слои — отдельной итерацией)
Реализован чистый, протестированный `domain`-слой новых фаз (план — в
`ROADMAP_PHASE_10-12.md`). Без сети, БД и UI; крейт `domain` — 107 юнит-тестов,
зелёные `fmt`/`clippy -D warnings`/`test`.
- **Фаза 10 — MOEX ALGO (`domain::algo`)**: Super Candles (`tradestats`) с
  агрегацией в произвольный TF, VWAP-полосой, buy-pressure и аномальным объёмом
  (z-score); FUTOI (нетто-позиция, доли long/short, ΔOI, дивергенция
  «цена↔позиция», экстремумы); HI2 (индекс Херфиндаля, уровни концентрации,
  всплески, ранжирование); движок **Mega Alerts** (edge-trigger, параметризуемые
  пороги).
- **Фаза 10 — Key Activity (`domain::keyactivity`)**: типизированная модель
  правил (метрика/оператор/порог/область), интерпретатор композиции
  `AND/OR/NOT` и «если A то B», дефолтный набор правил, период анализа
  (`1h…3m`), сборка LLM-промпта и локальный fallback-свод. Правила сериализуются
  в JSON для хранения настроек.
- **Фаза 11 — историзация (`domain::history`)**: расширенная свеча
  (OHLCV + опц. VWAP/disb/OI/HI2) с явным источником и TF; каталог
  `DatasetMeta`/`Catalog`; нормализация диапазонов и план дозагрузки
  (`missing_ranges` — дыры и хвост).
- **Фаза 12 — опционы (`domain::options`)**: ценообразование Блэк-76 и Башелье,
  аналитические греки (сверены с конечными разностями), устойчивый решатель IV
  (Ньютон + бисекция); 4 модели улыбки — MOEX-параметрическая, SABR (Hagan),
  SVI (raw), Каленкович — с общим калибратором (симплекс Нелдера–Мида, без
  внешних зависимостей) и RMSE; конструктор стратегий (ноги, шаблоны, payoff,
  агрегированные греки, точки безубытка, max profit/loss).
- Справочник по моделям улыбки — `docs/options-smile-models.html` (источник
  правды для формул/калибровки).

Вне этой итерации (нужны сеть/ключи/десктоп): транспорт `data::moex`/`http`/`llm`,
таблицы `storage`, IPC/Tauri (`app`), вкладки фронта и блоки `(verify)` —
фиксация контракта ALGOPACK по живым фикстурам.

## Загрузка секрета
- Секрет резолвится тремя путями (по приоритету): переменная окружения
  `FINAM_API_SECRET` → файл `.env` (`data::dotenv`: ключи `FINAM_API_SECRET`/
  `FINAM_SECRET`, без учёта регистра, поиск вверх по дереву каталогов) →
  ОС-keyring (фича `keyring`). Парсер `.env` — чистый, без зависимостей,
  покрыт юнит-тестами; сам файл `.env` в `.gitignore` и в репозиторий не попадает.

## Следующее
- Боевой прогон с live-данными **упирается только в egress-политику окружения**:
  с секретом из `.env` пайплайн авторизации доходит до сетевой границы и
  получает `Host not in allowlist: trade-api.finam.ru`. Нужно добавить
  `trade-api.finam.ru` в egress-allowlist окружения — после этого тот же
  бинарь/пример потянет реальные рыночные данные без изменений в коде.
- Финальная упаковка MSI/NSIS (`cargo tauri build`) + иконки — на десктопной
  машине с webkit2gtk.
