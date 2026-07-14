# Market Terminal — аналитический терминал рыночных данных (Finam Trade API)

Десктопное приложение под Windows для наблюдения за рыночным оборотом и
денежными потоками: куда идут деньги, какие сектора лидируют, как перетекает
капитал между акциями, фьючерсами и облигациями.

> **read-only аналитика (v1).** Терминал только читает рыночные данные и считает
> метрики — он **не выставляет заявки**.

## Стек

- **Ядро на Rust** (cargo workspace): gRPC к Finam Trade API (`tonic`),
  аналитика, локальное хранилище **DuckDB**.
- **UI на Tauri** (Rust + веб-фронт): графика — ECharts (treemap, heatmap,
  Sankey), TradingView Lightweight Charts (свечи), TanStack Table (таблицы).

## Архитектура (workspace)

```
crates/
  finam-proto/  gRPC-стабы Finam Trade API (генерируются из .proto)
  domain/       доменная модель + аналитика — без зависимостей от API/UI/БД
  data/         адаптер Finam: auth+refresh, fetch, стримы, rate-limit, классификация
  storage/      DuckDB: схема, ингест, аналитические запросы
  app/          Tauri: оркестрация, IPC-команды, события, планировщик ингеста
frontend/       Vite + TS + Svelte + ECharts + Lightweight Charts
```

**Правило слоёв:** вся аналитическая математика живёт в `domain` и не знает про
gRPC/Tauri/DuckDB, поэтому собирается и тестируется кросс-платформенно (включая
CI на Linux). `data`/`storage`/`app` — тонкие адаптеры к API и железу.

## Что уже реализовано (фазы 0–12; статус — в `SPEC_0-12.md`)

- `domain` — аналитическое ядро, полностью покрыто тестами:
  - **turnover** — оборот, направленный оборот, скан «необычного объёма»;
  - **flow** — net money flow, Money Flow Index (MFI), Cumulative Volume Delta;
  - **breadth** — ширина рынка (A/D, % растущих);
  - **sector** — роллапы метрик по секторам (взвешенные по обороту);
  - **rrg** — секторная ротация (RS-Ratio / RS-Momentum, квадранты);
  - **crossasset** — доли оборота по классам активов и матрица перетоков (Sankey);
  - **alerts** — движок алёртов по цене/изменению (edge-triggered, без спама);
  - **backtest** (V2) — бэктестер стратегий: трейт `Strategy`, движок и метрики
    (P&L, win-rate, profit factor, просадка, Sharpe), библиотека сценариев
    (`ma_cross`, `same_lot`, `iceberg`, `cvd_momentum`);
  - **delta** (V2) — footprint/дельта по ленте и детектирующие роботы
    (равные лоты, айсберг, поглощение);
  - **trading** (V2) — симулятор исполнения (paper trading): заявки, счёт/позиции,
    риск, матчинг `SimBroker`.
- `storage` — слой хранилища (Фаза 1), покрыт тестами:
  - контракт `Store` + реализации `MemStore` (в памяти) и `DuckStore` (нативный
    DuckDB за фичей `duckdb`);
  - миграции с версией схемы; ингест баров/снимков/инструментов; загрузка
    классификации секторов; планировщик батч-поллинга и бэкфилл истории.
- `data` — контракты адаптера Finam (трейт `MarketData`, `TimeFrame`, ошибки) и
  классификация секторов; плюс чистые, протестированные примитивы клиентского
  слоя (Фаза 0): `RateLimiter` (per-method лимит ~200/мин), `TokenState`
  (учёт JWT + упреждающий refresh), `Backoff` (экспоненциальные повторы с
  джиттером и `is_retryable`), `Method` (канонические имена методов API),
  `SecretStore`/`MemSecretStore` (контракт + in-memory хранилище API-секрета),
  `KeyringSecretStore` (боевое хранилище в ОС-keyring за фичей `keyring`).
  Сетевой gRPC-клиент — за фичей `grpc`: `AuthManager` + `GrpcAuthTransport`
  (обмен `AuthService.Auth`: кэш JWT, упреждающий refresh, лимит метода, повтор
  транзиентных сбоев), `FinamMarketData` — реализация трейта `MarketData`
  (`assets`/`bars`/`last_quote`/`latest_trades`/`order_book` — DOM) с JWT-
  авторизацией, лимитами и маппингом протобаф→домен, и live-стримы (`stream`):
  `subscribe_*` поверх `Subscribe*` + `StreamReconnect` (авто-reconnect при
  обрыве ~раз в 24 ч). Оркестрация, маппинг и политика повторов покрыты тестами
  без сети. Offline-реплей — `app::replay::ReplaySource` (тот же трейт из баров).
- `app` — каркас Tauri (Фаза 3): ядро IPC (`AppState` + DTO + обработчики
  `instruments`/`bars`/`turnover_series`/`sector_rollup`/`sector_map`),
  протестированное на `MemStore`; привязка Tauri за фичей `tauri`; инициализация
  `tracing` (`telemetry::init`, фильтр из `RUST_LOG`, по умолчанию `info`).
  Асинхронный планировщик ингеста `ingest::IngestService` за фичей `ingest`
  (опрос `data::MarketData` в хранилище под лимитом; такт покрыт тестами).
- `frontend` — фронт-приложение (Фазы 3–8): Vite + Svelte 5 + TS, тёмная тема,
  докуемые панели, ECharts (treemap, heatmap, scatter, line, gauge, pie, Sankey)
  + Lightweight Charts свечи, типизированный IPC-клиент с мок-режимом (работает
  в браузере без бэкенда). Панели: акции/секторы (оборот, breadth, топ-движения,
  RRG), фьючерсы (группы), облигации (кривая доходности, эмитенты), «сумма всех»
  (gauge общего оборота, donut долей, stacked area, Sankey перетоков), live-
  панели Time&Sales (лента сделок), DOM (стакан-лесенка со спредом) и алёрты
  (правила цена/изменение + срабатывания), а также настройки представления
  (localStorage). Тяжёлые графические библиотеки вынесены в отдельные чанки.

Представления 1–4 и live-панели (Time&Sales/DOM/алёрты) готовы. Сетевой
gRPC-клиент Finam за фичей `grpc` реализован полностью: auth, рыночные данные
(`MarketData`) и live-стримы с авто-reconnect. Боевой режим (`app` фича `live`)
связывает живой источник с планировщиком ингеста: авторизация → справочник
инструментов → цикл опроса баров в хранилище. Секрет берётся из
`FINAM_API_SECRET` или ОС-keyring (в репозиторий не попадает). Остаётся сетевой контур
фаз 10–12 (`data::moex`/`http`/`llm`), storage истории/ALGOPACK, финальная
упаковка MSI/NSIS (`cargo tauri build` + иконки) и боевой прогон с live-данными
(нужны egress-доступ и секрет) — см. `ROADMAP.md`/`SPEC_0-12.md`/`TASKS_list.md`.

> **Доступ к API.** Боевой режим требует сетевого доступа к
> `trade-api.finam.ru:443`. В Claude Code on the web добавьте этот хост в
> network egress allowlist окружения, иначе вызовы вернут
> «Host not in allowlist».

## Сборка и тесты

```bash
# Кросс-платформенно (работает в т.ч. в CI на Linux): без нативного DuckDB/Tauri.
cargo test --workspace
cargo clippy --workspace

# С нативным движком DuckDB (bundled, компиляция C++ из исходников):
cargo test -p storage --features duckdb

# С ОС-keyring (нативный бэкенд под платформу; live-тест — только с --ignored):
cargo test -p data --features keyring

# С gRPC-слоем (codegen из .proto через vendored protoc + auth-обмен):
cargo test -p data --features grpc

# С планировщиком ингеста (async-цикл опроса данных в хранилище):
cargo test -p app --features ingest

# Каркас боевого роутинга заявок (заглушка FinamOrderRouter; по умолчанию off):
cargo build -p data --features live-trading

# Консольный smoke (путь domain → storage → dto на MemStore):
cargo run -p app

# Фронт (мок-данные вне Tauri):
cd frontend && npm install && npm run check && npm run test && npm run build

# Десктоп целиком (нужен webkit2gtk на Linux):
cargo run -p app --features tauri

# Сборка инсталляторов MSI/NSIS (Windows; нужен tauri-cli и иконки):
#   bundle.active=true в crates/app/tauri.conf.json, затем:
cargo tauri build

# Живой smoke gRPC-пайплайна (auth → assets → bars/quote) против Finam:
FINAM_API_SECRET=… cargo run -p data --features grpc --example live_check

# Боевой режим: live-подключение и ингест баров вотчлиста в хранилище.
# Нужен сетевой доступ к trade-api.finam.ru:443 и валидный секрет.
FINAM_API_SECRET=… cargo run -p app --features live
# (опц.) сохранить секрет в ОС-keyring один раз, дальше запускать без env:
FINAM_API_SECRET=… cargo run -p app --features live,keyring -- --store-secret
cargo run -p app --features live,keyring
```

## Finam Trade API

- gRPC (+ REST-gateway): `https://trade-api.finam.ru:443` (HTTP/2, TLS).
- Сервисы: `AuthService`, `AssetsService`, `MarketDataService` (в v1),
  `AccountsService` (не используется).
- Лимит ~200 запросов/мин на метод; техокно 05:00–06:15 MSK; стрим обрывается
  ~раз в 24 ч (нужен авто-reconnect).
- Документация: https://tradeapi.finam.ru/docs/ ,
  proto: https://github.com/FinamWeb/trade-api-docs
- API-ключ хранится в ОС-keyring, **не** в репозитории.

См. также: `ROADMAP.md` — общий план фаз 0–12; `SPEC_0-12.md` — спецификация
с отметками выполнения (сверена с кодом); `TASKS_list.md` — задачи по
оставшейся работе и план оркестрации.
