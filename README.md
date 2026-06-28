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

## Что уже реализовано (Фазы 0–6)

- `domain` — аналитическое ядро, полностью покрыто тестами:
  - **turnover** — оборот, направленный оборот, скан «необычного объёма»;
  - **flow** — net money flow, Money Flow Index (MFI), Cumulative Volume Delta;
  - **breadth** — ширина рынка (A/D, % растущих);
  - **sector** — роллапы метрик по секторам (взвешенные по обороту);
  - **rrg** — секторная ротация (RS-Ratio / RS-Momentum, квадранты);
  - **crossasset** — доли оборота по классам активов и матрица перетоков (Sankey).
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
  транзиентных сбоев) и `FinamMarketData` — реализация трейта `MarketData`
  (`assets`/`bars`/`last_quote`/`latest_trades`) с JWT-авторизацией, лимитами и
  маппингом протобаф→домен. Оркестрация и маппинг покрыты тестами без сети.
- `app` — каркас Tauri (Фаза 3): ядро IPC (`AppState` + DTO + обработчики
  `instruments`/`bars`/`turnover_series`/`sector_rollup`/`sector_map`),
  протестированное на `MemStore`; привязка Tauri за фичей `tauri`; инициализация
  `tracing` (`telemetry::init`, фильтр из `RUST_LOG`, по умолчанию `info`).
  Асинхронный планировщик ингеста `ingest::IngestService` за фичей `ingest`
  (опрос `data::MarketData` в хранилище под лимитом; такт покрыт тестами).
- `frontend` — фронт-приложение (Фазы 3–6): Vite + Svelte 5 + TS, тёмная тема,
  докуемые панели, ECharts (treemap, heatmap, scatter, line, gauge, pie, Sankey)
  + Lightweight Charts свечи, типизированный IPC-клиент с мок-режимом (работает
  в браузере без бэкенда). Панели: акции/секторы (оборот, breadth, топ-движения,
  RRG), фьючерсы (группы), облигации (кривая доходности, эмитенты), «сумма всех»
  (gauge общего оборота, donut долей, stacked area, Sankey перетоков).

Представления 1–4 готовы. Сетевой gRPC-клиент Finam (auth + рыночные данные)
реализован за фичей `grpc`; остаётся подключить его к асинхронному циклу опроса
и live-стримам (Фазы 7–8, см. `ROADMAP`).

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

# Консольный smoke (путь domain → storage → dto на MemStore):
cargo run -p app

# Фронт (мок-данные вне Tauri):
cd frontend && npm install && npm run build

# Десктоп целиком (нужен webkit2gtk на Linux):
cargo run -p app --features tauri
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

См. также `ROADMAP.md` — пошаговый план развития.
