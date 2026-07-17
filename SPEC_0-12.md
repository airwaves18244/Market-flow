# SPEC 0–12 — спецификация Market Terminal с отметками выполнения

Спецификация всех фаз проекта с фактическим статусом, **сверенным с кодом**
(ревизия 2026-07-15). Карта высот — `ROADMAP.md`; задачи по невыполненному —
`TASKS_list.md`.

Легенда: `[x]` — сделано и проверено · `[~]` — частично · `[ ]` — не сделано.
Идентификаторы задач фаз 10–12 (`10.1.2` и т.п.) стабильны — на них ссылаются
`TASKS_list.md` и дизайн-бриф (`design/claude-design-brief.md`).

## Снимок верификации (2026-07-15, после волн 1–4 TASKS_list)

Проверено фактически в этой ревизии (ветка PR #23):

- `cargo test --workspace` — **334 теста, зелёные** (без нативных фич);
- фиче-комбо: `data` (`grpc,http,moex,llm,keyring`) — 141;
  `app` (`moex,llm,ingest`) — 116; `storage --features duckdb` — 62
  (вкл. миграции v2→v3→v4 и Parquet-roundtrip);
- фронт: `svelte-check` 0 ошибок · `vitest` **41 тест** · `vite build` ок;
- IPC-контракт согласован end-to-end: **одни и те же 50 команд** (после
  полировки T16 добавлена `algo_hi2_ranking`) в
  `frontend/src/lib/ipc.ts` и регистрации `crates/app/src/tauri_app.rs`,
  все покрыты моком `mock.ts`;
- модули `data`: + **`http`, `moex` (ALGOPACK + ISS-доска опционов), `llm`
  (OpenRouter/OpenAI/Anthropic), `history` (HistorySource: Finam/MOEX)**;
- `storage::schema` **v4**: + таблицы `algo_*` (5 датасетов ALGOPACK),
  `history_bars`, `history_datasets`; Parquet экспорт/импорт (фича `duckdb`);
- фронт: все модули MOEX ALGO (Супер-свечи/FUTOI/HI2/Мега) и Key Activity —
  на типизированном IPC (`algoMock.ts` удалён); вкладка «Данные» — реальный
  загрузчик с событиями `history:*` и превью датасета; улыбка опционов
  умеет живую доску (`option_board`);
- настройки/правила Key Activity персистятся в ядро (`settings.json` в
  ОС-config-dir, атомарная запись) с миграцией из localStorage;
- cargo-фичи: `grpc`, `keyring`, `duckdb`, `tauri`, `ingest`, `live`,
  `live-trading` (заглушка), **`http`, `moex`, `llm`** (data),
  **`moex`, `llm`** (app).

**Живой смоук T14 (2026-07-17, egress открыт по P1):**

- ✅ **Finam gRPC** — сквозной прогон `live_check` живым секретом: auth-обмен,
  `Assets` (4565 инструментов MISX), `Bars` (D1), `LastQuote`. Попутно
  исправлен эндпоинт: `trade-api.finam.ru` теперь отвечает
  `301 → tradeapi.finam.ru`, gRPC редиректы не следует — константа
  `finam_proto::ENDPOINT` обновлена на `tradeapi.finam.ru:443`.
- ✅ **Опционная доска ISS** — контракт сверен живым ответом и фикстуры
  приведены к нему: колонки заглавными, в `marketdata` нет `IV`/`THEORPRICE`
  (IV решается из mid bid/ask), серверная фильтрация `assets=<код>`,
  ответ одной страницей без курсора, форвард — по SECID фьючерса из
  `UNDERLYINGASSET` (по коду актива forts пуст) с фолбэком на
  `UNDERLYINGSETTLEPRICE`. Детали — `crates/data/tests/fixtures/moex/README.md`.
- ✅ **ALGOPACK** (дозаезд 2026-07-17, боевой ключ): все 6 датасетов сверены
  живыми ответами через `example algopack_check` — `tradestats` (202 строки
  SBER), `orderstats` (204), `obstats` (198), `hi2` (160 точек), `futoi`
  (1000 точек Si), свечи (18 H1). Найдены и исправлены расхождения:
  `futoi` живёт на `iss/analyticalproducts/futoi/securities/{ticker}`
  (колонка `ticker`, `FIZ`/`YUR`; путь `datashop/.../fo/futoi` — 404);
  `hi2` — длинный формат `metric`/`value` (`hhi_*`, шкала 0..10 000 →
  нормировка к 0..1, метрика-заголовок `hhi_volume`); свечей в `datashop`
  нет — живой ресурс `iss/engines/.../candles.json?interval=`; фикстуры
  приведены к живым формам (`crates/data/tests/fixtures/moex/README.md`).
- ✅ **LLM** (дозаезд 2026-07-17): живой вызов OpenRouter зелёный; дефолтная
  модель `anthropic/claude-3.5-sonnet` больше не существует у провайдера
  (404) — дефолт обновлён на `anthropic/claude-sonnet-5`.
- ⚠️ Коэффициенты биржевой улыбки MOEX дословно не сверены (первоисточники
  методики вне egress); единицы `spread_*` живого obstats похожи на б.п. —
  перепроверить при выводе живого obstats в аналитику Mega.

---

## Фаза 0 — Фундамент ✅

- [x] Cargo workspace: `finam-proto`, `domain`, `data`, `storage`, `app`.
- [x] Дисциплина слоёв: аналитика в `domain` без зависимостей от API/UI/БД.
- [x] Контракты `data`: трейт `MarketData`, `TimeFrame`, ошибки
      (`DataError::is_retryable`), классификация секторов.
- [x] DDL схемы DuckDB (`storage::schema`).
- [x] `data::RateLimiter` — per-method скользящее окно (~200 req/мин).
- [x] `data::TokenState` — учёт JWT + упреждающий refresh (skew).
- [x] `data::Backoff` — экспоненциальный backoff с потолком и джиттером.
- [x] `data::Method` — канонические имена методов API (лимитер + трейсинг).
- [x] `data::SecretStore` + `MemSecretStore`; `KeyringSecretStore` за фичей
      `keyring` (Credential Manager / Keychain / keyutils).
- [x] Секрет-резолвер: env `FINAM_API_SECRET` → `.env` (`data::dotenv`,
      чистый парсер) → ОС-keyring.
- [x] `app::telemetry::init` — `tracing` с фильтром из `RUST_LOG`.
- [x] gRPC-кодоген `finam-proto` (vendored `.proto` + `protoc-bin-vendored`,
      фича `grpc`, по умолчанию выключена).
- [x] `data::AuthManager` + `GrpcAuthTransport` — обмен `AuthService.Auth`:
      кэш JWT, refresh, лимит, повторы; оркестрация в тестах без сети.
- [x] `data::FinamMarketData` — `assets`/`bars`/`last_quote`/`latest_trades`/
      `order_book` с JWT, лимитами и маппингом протобаф→домен (в тестах).

## Фаза 1 — Хранилище и ингест ✅

- [x] Контракт `Store`; реализации `MemStore` (кросс-платформенная) и
      `DuckStore` (нативный DuckDB за фичей `duckdb`, bundled).
- [x] Миграции с версией схемы (идемпотентный прогон), `SCHEMA_VERSION = 2`.
- [x] Ингест: `Writer` (бары/снимки/инструменты/классификация/trades),
      `snapshot_from_bars`, планировщик `BatchCursor` (round-robin).
- [x] Бэкфилл: `plan_backfill` + `chunk_range`.
- [x] Асинхронный цикл опроса `app::ingest::IngestService` (фича `ingest`);
      такт покрыт тестами на фейке.
- [x] Боевой режим `app --features live`: авторизация → справочник → цикл
      ингеста; секрет через резолвер; live smoke `example live_check`.
- [x] Боевой прогон с живыми данными: `example live_check` живым секретом —
      auth, Assets (4565 MISX), Bars, LastQuote (T14, 2026-07-17).

## Фаза 2 — Аналитика (`domain`) ✅

- [x] turnover, directional turnover, unusual volume (скан).
- [x] money flow, MFI, CVD.
- [x] breadth (A/D, % растущих).
- [x] sector rollups (взвешенные по обороту).
- [x] RRG (RS-Ratio / RS-Momentum, квадранты).
- [x] cross-asset shares + flow matrix (Sankey).
- [x] Полное юнит-покрытие без сети.

## Фаза 3 — Tauri-оболочка + каркас фронта ✅

- [x] Ядро IPC: `AppState` поверх `Store`, DTO (camelCase), обработчики
      `instruments`/`bars`/`turnover_series`/`sector_rollup`/`sector_map`.
- [x] Привязка Tauri за фичей `tauri` (`#[tauri::command]`, capabilities,
      `tauri.conf.json`, `build.rs`).
- [x] Фронт: Vite + Svelte 5 + TS, тёмная тема, панели, типизированный
      IPC-клиент с мок-режимом (работает в браузере без бэкенда).
- [ ] Полноценный dockview (drag-n-drop панелей) — полировка, не блокирует.

## Фаза 4 — Представление 1 (Акции/секторы) ✅

- [x] Treemap (размер = оборот, цвет = %изм).
- [x] Heatmap по секторам; breadth-индикатор; таблица топ-движений; RRG.
- [x] API `breadth_data`/`top_movers`/`rrg_sectors` + DTO + Tauri-команды.

## Фаза 5 — Представления 2–3 (Фьючерсы, Облигации) ✅

- [x] Фьючерсы: treemap по группам (2-символьный префикс).
- [x] Облигации: кривая доходности (8 сроков), таблица эмитентов.
- [x] API `futures_rollup`/`bonds_rollup`/`yield_curve`;
      `Store::instruments_by_asset_class`.

## Фаза 6 — Представление 4 (Сумма всех) ✅

- [x] Gauge общего оборота, donut долей, stacked area, Sankey перетоков.
- [x] API `cross_asset_summary`/`turnover_timeline`/`flow_sankey` + DTO.

## Фаза 7 — Live-функции ✅

- [x] Транспорт стримов `data::stream` (фича `grpc`): `subscribe_quotes`/
      `subscribe_trades`/`subscribe_bars`.
- [x] Авто-reconnect `StreamReconnect` (стрим рвётся ~раз в 24 ч).
- [x] DOM: `OrderBook`/`BookLevel` + `FinamMarketData::order_book`.
- [x] Алёрты: `domain::metrics::alerts::AlertEngine` (edge-trigger).
- [x] Replay: `app::replay::ReplaySource` (`MarketData` из сохранённых баров).
- [x] Панели `TimeSales`/`OrderBook`/`AlertsPanel`; события `trade:tick`/
      `orderbook:tick`; команда `alerts_scan`.

## Фаза 8 — Полировка и сборка 🟡

- [x] Настройки представления (`lib/settings.ts` + localStorage).
- [x] Чанкинг тяжёлых библиотек (`manualChunks`): ~1.27 МБ → ~73 КБ.
- [x] Метаданные бандла в `tauri.conf.json` (msi/nsis, NSIS RU/EN).
- [x] Обработка ошибок: верхний баннер + локальные состояния панелей.
- [ ] `8.5` Финальная сборка MSI/NSIS (`cargo tauri build`) + иконки —
      требует десктопного окружения (webkit2gtk/Windows).

## Фаза 9 — V2: Бэктестер · Торговля · Delta ✅

- [x] Таблица `trades` (схема v2): `Store::insert_trades`/`trades`.
- [x] `domain::backtest`: трейт `Strategy`, движок (позиция/комиссия/
      слиппедж), отчёт (P&L, win-rate, PF, просадка, Sharpe), библиотека
      стратегий (`ma_cross`, `same_lot`, `iceberg`, `cvd_momentum`).
- [x] `domain::delta`: footprint, дельта/CVD, роботы (равные лоты, айсберг,
      поглощение) — только анализ.
- [x] `domain::trading`: заявки/исполнения, счёт/позиции, риск, `SimBroker`.
- [x] `app`: `list_strategies`/`run_backtest`/`delta_footprint`/`robot_scan`,
      `TradeSession` (`submit_order`/`cancel_order`/`order_blotter`/
      `positions`/`account`), событие `fill:tick`.
- [x] Фронт: `Backtester`, `DeltaView`/`DeltaChart`, `TradePanel`.
- [x] Каркас роутинга: `data::OrderRouter` + `SimOrderRouter`;
      `FinamOrderRouter` — заглушка за фичей `live-trading`.
- [ ] `9.8` Реальный gRPC `OrderService`/`AccountsService` — отдельная
      интеграция, вне v1 (терминал read-only).

---

## Сквозные предпосылки S.1–S.4 (для фаз 10–12)

- [x] `S.1` Вкладочная навигация: `TabBar` + 8 вкладок, ленивая сборка чанков.
- [x] `S.2.1` Секционные настройки UI (`lib/settings.ts`, миграция).
- [x] `S.2.2` Секреты вне localStorage: резолвер env → `.env` → keyring;
      правила Key Activity и настройки персистятся в ядро (`app::settings`,
      `settings.json` + атомарная запись; миграция из localStorage).
- [ ] `S.3.1` Egress-allowlist: `apim.moex.com`, `iss.moex.com`,
      `data.moex.com`, LLM-хосты; документировать в README.
- [x] `S.3.2` `.env.example`: ключ `MOEX_ALGO_API` с комментариями.
- [x] `S.4.1` Фича `http` в `data`: `HttpTransport`/`ReqwestTransport`/`HttpClient`
      (`get_json`/`post_json`), rustls+gzip, тайм-ауты, повторы через `Backoff`,
      маппинг статусов в `DataError` (429/5xx ретраябельны).
- [x] `S.4.2` `data::Method` расширен: `MoexTradestats`, `MoexFutoi`, `MoexHi2`,
      `MoexObstats`, `MoexOrderstats`, `MoexCandles`, `MoexOptions`, `Llm`.
- [x] `S.4.3` Юнит-тесты HTTP-слоя на фейк-транспорте (URL, заголовки, ретраи,
      4xx/5xx, отсутствие утечки Authorization в Debug).

## Фаза 10 — MOEX ALGO 🟡

### 10.0 — Контракт API ALGOPACK
- [x] `10.0.1` Базовый URL/пути:
      `https://apim.moex.com/iss/datashop/algopack/{market}/{dataset}.json`;
      датасеты `tradestats`/`orderstats`/`obstats`/`hi2`/`futoi` (fo);
      per-ticker `.../{dataset}/{SECID}.json`.
- [ ] `10.0.2` `(verify)` Параметры запроса (`date`, `from`, `till`, `latest`,
      `tickers`, `start`, `iss.meta=off`, `iss.json=extended`), формат
      пагинации (cursor block), шаг свечей 5 мин — по живому ответу.
- [x] `10.0.3` Авторизация: `Authorization: Bearer <MOEX_ALGO_API>`;
      ключ через секрет-резолвер.
- [ ] `10.0.4` Живые JSON-фикстуры в `crates/data/tests/fixtures/moex/*.json`
      (нужны боевой ключ + egress; дальше парсер тестируется офлайн).

### 10.1 — Транспорт MOEX (`data`, фича `moex`)
- [x] `10.1.1` Модуль `data::moex`: клиент `MoexAlgo` поверх `HttpClient`
      с Bearer-заголовком (токен не попадает в Debug/логи).
- [x] `10.1.2` Методы `tradestats`/`orderstats`/`obstats`/`hi2`/`futoi`/`candles`;
      пагинация курсором, склейка страниц, лимит, ретраи.
- [x] `10.1.3` Парсер ISS JSON (`columns`+`data` → строки) — чистые функции
      на фикстурах `(unverified)`; мягкий маппинг полей (`Option`), MSK→UTC.
- [x] `10.1.4` Трейт `AlgoSource` + `FakeAlgoSource`; также `OptionsSource`
      + фейк (доска опционов, фаза 12.4).

### 10.2 — Доменные модели и аналитика (`domain`) ✅ (кроме 10.2.4)
- [x] `10.2.1` Типы Super Candles (`domain::algo::tradestats`).
- [x] `10.2.2` Типы FUTOI (нетто, доли long/short, ΔOI).
- [x] `10.2.3` Тип HI2 (Херфиндаль, интерпретация концентрации).
- [x] `10.2.4` Типы `ObstatsPoint`/`OrderstatsPoint` (спред BBO, imbalance,
      put/cancel) — `Option`-мягкие, с тестами.
- [x] `10.2.5` Аналитика Super Candles: агрегация TF, VWAP-полоса,
      buy-pressure, аномальный объём (z-score).
- [x] `10.2.6` Аналитика FUTOI: динамика нетто, дивергенция, экстремумы.
- [x] `10.2.7` Аналитика HI2: пороги, всплески, ранжирование.
- [x] `10.2.8` Движок Mega Alerts (edge-trigger, параметризуемые пороги).

### 10.3 — Движок Key Activity (`domain::keyactivity`) ✅
- [x] `10.3.1` Модель правил: метрика + оператор + порог + область.
- [x] `10.3.2` Композиция `AND/OR/NOT` + `if A then B`; JSON-сериализация.
- [x] `10.3.3` Набор правил по умолчанию (документирован).
- [x] `10.3.4` Периоды `1h|1d|1w|1m|3m`, резолвер диапазона.
- [x] `10.3.5` `KeyActivityRow` — вход таблицы и LLM-итога.
- [x] `10.3.6` Полное юнит-покрытие.

### 10.4 — LLM-итог
- [x] `10.4.1` Трейт `LlmProvider` + реализации OpenRouter (дефолт) /
      Anthropic / OpenAI за фичей `llm` (поверх `http`, `post_json`).
- [x] `10.4.2` Сборка промпта (`domain::keyactivity::prompt`) — чистая,
      с лимитом токенов/усечением.
- [x] `10.4.3` Безопасность: ключ через резолвер (`OPENROUTER_API_KEY` и др.),
      без логирования; тайм-аут 45с, ретраи, деградация в локальный свод
      (`source: llm|local` в DTO).
- [x] `10.4.4` Кэш результата (хеш входа + период + провайдер + модель) на
      время сессии в `AppState`.

### 10.5 — Storage ALGOPACK
- [x] `10.5.1` Таблицы `algo_tradestats`/`algo_futoi`/`algo_hi2`/
      `algo_obstats`/`algo_orderstats` (ключ secid+ts+market; futoi + clgroup),
      схема v3, миграция v2→v3 с тестом.
- [x] `10.5.2` Writer'ы ингеста + дедуп по ключам (MemStore + DuckStore).
- [x] `10.5.3` Персист правил Key Activity: файл настроек `app::settings`
      (валидация через доменные типы), UI мигрирует из localStorage.
- [x] `10.5.4` Запросы чтения по тикеру/периоду/датасету (сортировка по ts).

### 10.6 — App/IPC
- [x] `10.6.1` DTO: `KeyActivity*Dto`, `TradestatsDto`/`FutoiDto`/`Hi2Dto`/
      `MegaAlertDto`, `OptionQuoteDto`/`OptionBoardDto`.
- [x] `10.6.2` Обработчики: `key_activity`/`key_activity_summary` (LLM при
      наличии ключа, иначе локальный свод)/`key_activity_rules`,
      `algo_tradestats`/`algo_futoi`/`algo_hi2`/`algo_mega_alerts` (чтение из
      storage через аналитику `domain::algo`).
- [x] `10.6.3` Tauri-команды для реализованных обработчиков.
- [x] `10.6.4` Ингест ALGOPACK: `app::algo_ingest::AlgoIngestService`
      (батч/лимиты, тесты на FakeAlgoSource) + вход `live::run_algo`.
- [x] `10.6.5` Мок-данные Key Activity в `frontend/src/lib/mock.ts`.

### 10.7 — Frontend: вкладка «MOEX ALGO»
- [x] `10.7.1` `MoexAlgoTab` — тулбар (инструмент/период/рынок) + 5 модулей.
- [x] `10.7.2` Супер-свечи (`SuperCandlesChart` + `DisbBars`) — типизированный
      IPC `algo_tradestats` (в браузере — мок-IPC; `algoMock.ts` удалён).
- [x] `10.7.3` FUTOI (`FutoiChart`) — IPC `algo_futoi`.
- [x] `10.7.4` HI2 (`Hi2Chart`) — IPC `algo_hi2`.
- [x] `10.7.5` Мега-алёрты — IPC `algo_mega_alerts`.
- [x] `10.7.6` `KeyActivityTable` — боевой IPC, периоды, фильтры.
- [x] `10.7.7` `KeyActivitySummary` («ИТОГО») — боевой IPC, локальный свод.
- [x] `10.7.8` Типы `lib/types.ts` + методы `lib/ipc.ts` для Key Activity.

### 10.8 — Настройки (раздел MOEX ALGO / Key Activity / LLM)
- [x] `10.8.1` Паспорт/ALGOPACK: UI + персист в ядро через `app::settings`.
- [x] `10.8.2` Конструктор правил Key Activity: UI + персист в ядро
      (`key_activity_rules_set` с доменной валидацией).
- [x] `10.8.3` LLM-настройки: UI + живой вызов провайдера (10.4).
- [x] `10.8.4` Период анализа по умолчанию.

### 10.9 — Тесты/CI/доки
- [x] `10.9.1` `domain`: аналитика, Mega Alerts, Key Activity, промпт.
- [x] `10.9.2` `data`: парсер ISS на фикстурах `(unverified)`, оркестрация
      на фейк-транспорте (пагинация, лимиты, заголовки).
- [x] `10.9.3` `app`: Key Activity, algo_*, option_board, history — в тестах.
- [x] `10.9.4` Документация синхронизирована (этот файл).

## Фаза 11 — Историзация 🟡

### 11.0 — Модель данных и каталог ✅
- [x] `11.0.1` `domain::history`: расширенная свеча (OHLCV + опц.
      VWAP/disb/OI/HI2) с источником и TF.
- [x] `11.0.2` Каталог `DatasetMeta`/`Catalog`, нормализация диапазонов,
      `missing_ranges` (дыры + хвост).
- [x] `11.0.3` Семантика TF: `domain::TimeFrame` переиспользован.

### 11.1 — Абстракция источника истории (`data`)
- [x] `11.1.1` Трейт `HistorySource`:
      `load(ticker, tf, from, till) -> Vec<HistoryBar>` (`data::history`).
- [x] `11.1.2` Адаптеры `FinamHistory` (gRPC bars + чанкинг, склейка/дедуп)
      и `MoexHistory` (tradestats → OHLCV + vwap/disb); `FakeHistorySource`.
- [x] `11.1.3` Выбор источника — параметр запроса; TS нормализованы к UTC.
- [x] `11.1.4` Лимиты/ретраи наследуются от транспортов (gRPC/http).

### 11.2 — Локальное хранилище истории (`storage`)
- [x] `11.2.1` Формат зафиксирован: DuckDB — основное хранилище, Parquet —
      экспорт (обоснование — в конце файла).
- [x] `11.2.2` Таблицы `history_bars`/`history_datasets` + идемпотентный
      upsert, дедуп по ключу (source, secid, tf, ts); `SCHEMA_VERSION = 4`,
      миграция v3→v4 с тестом.
- [x] `11.2.3` Инкрементальная дозагрузка: `Store::history_missing_ranges`
      поверх `domain::history::missing_ranges` (источник+TF).
- [x] `11.2.4` Персист `DatasetMeta` (`history_datasets`): список/размер/
      удаление/рефреш; каталог переживает переоткрытие БД.
- [x] `11.2.5` Конфигурируемая директория данных (`storage::config`,
      env-переопределение, по умолчанию ОС data-dir).
- [x] `11.2.6` Экспорт в Parquet (`export_history_parquet`; расширение
      parquet статически в bundled DuckDB — офлайн).
- [x] `11.2.7` Импорт Parquet (`import_history_parquet`, roundtrip-тест).

### 11.3 — Загрузчик (`app`)
- [x] `11.3.1` Сервис `app::history` (фича `ingest`): очередь (тикер×TF),
      качает только дыры, пишет бары + каталог, колбэк прогресса.
- [x] `11.3.2` События `history:progress`/`history:done`/`history:error`.
- [x] `11.3.3` Очередь + кооперативная отмена (`history_cancel`, реестр
      задач); ошибка одной задачи не роняет остальные.
- [x] `11.3.4` IPC: `history_datasets`/`history_delete`/`history_plan`/
      `history_load`/`history_cancel`/`history_preview` (DTO, Tauri, мок, тесты).

### 11.4 — Frontend: вкладка «Данные»
- [x] `11.4.1` `HistoryTab`: источник, инструмент, мультиселект TF, диапазон,
      «Загрузить», прогресс (в мок-режиме — симуляция).
- [x] `11.4.2` `DatasetManager`: таблица датасетов + удаление.
- [x] `11.4.3` Подписки на `history:*` в `HistoryTab` (реальный прогресс под
      Tauri; в браузере — детерминированная симуляция мока).
- [x] `11.4.4` Превью датасета свечами (`history_preview` + `CandleChart`).

### 11.5 — Контракт фида для бэктестера
- [ ] `11.5.1` Расширить `ReplaySource`: мульти-TF, расширенные поля,
      чтение из стора истории.
- [ ] `11.5.2` Детерминированный курсор по (ticker, tf, range).

### 11.6 — Настройки
- [~] `11.6.1` Раздел «Данные» в `SettingsTab` есть (источник по умолчанию и
      пр.) — персист localStorage; директория/лимиты — с загрузчиком.

### 11.7 — Тесты/доки
- [x] `11.7.1` `domain`: каталог/нормализация/план дозагрузки.
- [x] `11.7.2` `storage`: upsert/дедуп/планирование/каталог/Parquet
      (MemStore + DuckDB).
- [x] `11.7.3` `app`: оркестрация загрузчика на `FakeHistorySource`
      (дыры, монотонный прогресс, отмена, ошибки).
- [x] `11.7.4` Документация синхронизирована (этот файл).

## Фаза 12 — Опционы 🟡

### 12.0 — Исследование и спецификация улыбки
- [x] `12.0.1` Black-76 + Bachelier сверены с методикой MOEX/НКЦ (r=0 для
      маржируемых) — совпали, изменений не потребовалось.
- [~] `12.0.2` MOEX-улыбка приведена к документированной срочной структуре
      (денежность σ·√T, подъём крыльев); точные коэффициенты биржевой формулы
      дословно не сверены — первоисточники вне egress `(unverified)`.
- [x] `12.0.3` SABR (Hagan): α, β, ρ, ν; вырожденные случаи.
- [x] `12.0.4` SVI (raw): total variance, условия no-arbitrage.
- [x] `12.0.5` Каленкович: уровень/наклон/кривизна/время.
- [x] `12.0.6` `docs/options-smile-models.html` финализирован (форма σ(d),
      блок verified/unverified, ссылки на методики).

### 12.1 — Ядро ценообразования (`domain::options`) ✅
- [x] `12.1.1` Black-76 (call/put, форвардная конвенция, `r`).
- [x] `12.1.2` Bachelier (нормальная модель).
- [x] `12.1.3` Греки: delta/gamma/vega/theta/rho (аналитически, сверены с
      конечными разностями).
- [x] `12.1.4` IV-решатель: Ньютон + бисекция, устойчив на крыльях.
- [x] `12.1.5` Полное юнит-покрытие (эталоны, put-call паритет, пределы).

### 12.2 — Модели улыбки ✅
- [x] `12.2.1` Трейт `SmileModel`: `iv(...)` + `calibrate(...)`.
- [x] `12.2.2`–`12.2.5` `MoexSmile`, `Sabr`, `Svi`, `KalenkovichSmile`.
- [x] `12.2.6` Общий калибратор (Нелдер–Мид, без внешних зависимостей),
      RMSE, веса по OI.

### 12.3 — Конструктор стратегий ✅
- [x] `12.3.1` Модель ноги (call/put/underlying, сторона, страйк, кол-во).
- [x] `12.3.2` Шаблоны: vertical/straddle/strangle/butterfly/condor/
      calendar/covered call.
- [x] `12.3.3` Payoff (экспирация + текущий), греки портфеля, безубытки,
      max profit/loss.
- [x] `12.3.4` Профиль риска по диапазону; юнит-покрытие.

### 12.4 — Данные опционной доски (`data`, MOEX ISS)
- [x] `12.4.1` Загрузка доски (серии, страйки, bid/ask/last, OI) через
      публичный ISS (`data::moex::options`, `MoexIss`); контракт сверен живым
      ответом (T14): серверный фильтр `assets=`, колонки заглавными, `IV`/
      `THEORPRICE` в живом `marketdata` отсутствуют.
- [x] `12.4.2` Базовый актив/форвард (forts по `UNDERLYINGASSET`, фолбэк
      `UNDERLYINGSETTLEPRICE`); маппинг доски → точки улыбки (IV через
      IV-решатель из mid bid/ask, вес = OI, фильтрация неликвида).
- [x] `12.4.3` Трейт `OptionsSource` + `FakeOptionsSource`.

### 12.5 — Storage
- [ ] `12.5.1` Таблицы доски/снимков IV (опц. историзация улыбки).

### 12.6 — App/IPC
- [x] `12.6.1` DTO: `GreeksDto`, `OptionPrice*`, `ImpliedVol*`,
      `SmilePointInput`, `SmileFit*`, `StrategyLeg*`, `StrategyEval*`,
      `SmileModelDto`. `OptionQuoteDto` — с фазой 12.4.
- [x] `12.6.2` Обработчики `option_price`/`option_implied_vol`/`smile_fit`/
      `strategy_eval`/`list_smile_models`/`option_board` — готовы, в тестах.

### 12.7 — Frontend: вкладка «Опционы»
- [x] `12.7.1` `OptionsTab`: Калькулятор · Улыбка · Конструктор.
- [x] `12.7.2` `OptionCalculator`: параметры → цена/греки/IV.
- [x] `12.7.3` `SmileChart`/`SmileView`: все модели сразу, карточки
      параметров + RMSE, «активная» модель, OI-взвешенный scatter
      (рыночные точки — мок до 12.4).
- [x] `12.7.4` `StrategyBuilder` + `PayoffChart`: пресеты, payoff, греки,
      безубытки.
- [~] `12.7.5` Таблица греков/типы/IPC готовы; профиль риска (тепловая карта
      цена/время) — позже.

### 12.8 — Настройки
- [~] `12.8.1` Раздел «Опционы» в `SettingsTab` есть (модель, ставка,
      улыбка) — персист localStorage.

### 12.9 — Тесты/доки
- [x] `12.9.1` `domain`: ценообразование/греки/IV/калибровка/стратегии.
- [x] `12.9.2` `app`: обработчики опционов в тестах; `data`: парсер доски на
      фикстурах, сверенных живым ответом ISS (T14).
- [~] `12.9.3` Docs финализированы; остаток — дословная сверка коэффициентов
      по методике при появлении egress (T14).

---

## Приложение А — решение по формату хранения истории (11.2.1)

**Первичное хранилище — DuckDB, Parquet — формат экспорта/обмена.**

Почему DuckDB: уже в стеке (`DuckStore`, миграции, upsert, бэкфилл написаны и
протестированы); бэктест требует SQL-выборок «бар за баром по тикеру/TF» и
джойнов с ALGOPACK-метриками; инкрементальная дозагрузка/дедуп — это UPSERT;
одна транзакционная БД проще в управлении. Parquet при этом доступен нативно
(`COPY TO`/`read_parquet`).

Почему Parquet как экспорт: переносимость (pandas/polars/Arrow/R), архив
«холодных» данных, снимок для воспроизводимости бэктеста.

Пересмотреть, если объёмы дорастут до десятков ГБ на инструмент или
понадобится распределённое хранение (тогда Hive-partitioned Parquet + DuckDB
как движок запросов).

## Приложение Б — принятые решения по фазам 10–12

- Язык: roadmap/spec — RU, дизайн-бриф — EN.
- Модели улыбки: MOEX-параметрическая + SABR + SVI + Каленкович (4 шт.,
  выбор — в настройках доски).
- LLM-провайдер: абстракция, по умолчанию **OpenRouter**; поддерживаются
  Anthropic и OpenAI.
- Открытые `(verify)`-вопросы: точные имена полей/пагинация ALGOPACK ISS —
  по живому ответу боевого ключа (10.0.2/10.0.4); официальная форма улыбки
  MOEX — по авторизованной методике MOEX/НКЦ (12.0).
