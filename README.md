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

## Что уже реализовано (Фазы 0–2)

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
  классификация секторов.
- `app` — smoke-точка входа: прогоняет путь данных `domain` → `storage`
  (миграция → ингест → снимок → запрос).

Сетевые реализации (tonic/gRPC) и Tauri-UI подключаются в следующих фазах
(см. `ROADMAP`).

## Сборка и тесты

```bash
# Кросс-платформенно (работает в т.ч. в CI на Linux): без нативного DuckDB.
cargo test --workspace
cargo clippy --workspace

# С нативным движком DuckDB (bundled, компиляция C++ из исходников):
cargo test -p storage --features duckdb

# Запуск smoke-точки входа:
cargo run -p app
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
