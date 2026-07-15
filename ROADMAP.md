# ROADMAP — Market Terminal (фазы 0–12)

Единый план развития проекта. Здесь — «карта высот»: цель каждой фазы, её
статус и что осталось. Детальная спецификация с поштучными отметками — в
**`SPEC_0-12.md`**; задачи по оставшейся работе и план оркестрации — в
**`TASKS_list.md`**.

Статусы: ✅ готово · 🟡 частично (ядро есть, есть недоделки) · ⛔ не начато.

## Сводная таблица

| Фаза | Название | Статус | Что осталось |
|------|----------|--------|--------------|
| 0 | Фундамент (workspace, слои, gRPC, auth, лимиты, секреты) | ✅ | — |
| 1 | Хранилище и ингест (DuckDB, Store, планировщик, live-режим) | ✅ | боевой прогон (egress) |
| 2 | Аналитика `domain` (turnover, flow, breadth, sector, RRG, cross-asset) | ✅ | — |
| 3 | Tauri-оболочка + каркас фронта | ✅ | полный dockview (полировка) |
| 4 | Представление 1 — Акции/секторы | ✅ | — |
| 5 | Представления 2–3 — Фьючерсы, Облигации | ✅ | — |
| 6 | Представление 4 — Сумма всех | ✅ | — |
| 7 | Live-функции (стримы, DOM, алёрты, replay) | ✅ | — |
| 8 | Полировка и сборка | 🟡 | упаковка MSI/NSIS + иконки (десктоп) |
| 9 | V2 — Бэктестер · Торговля · Delta | ✅ | реальный OrderService (вне v1) |
| 10 | MOEX ALGO (Super Candles · FUTOI · HI2 · Mega · Key Activity · LLM) | ✅ | живая сверка контракта ISS `(unverified)` — T14 |
| 11 | Историзация для бэктестера (источники, storage v4, загрузчик, Parquet) | ✅ | — |
| 12 | Опционы (калькулятор · улыбка · конструктор · живая доска) | 🟡 | сверка коэффициентов улыбки MOEX, storage доски (12.5.1) |
| — | Сквозное: HTTP-слой, персист настроек, egress | 🟡 | egress-allowlist (P1) — блокирует T14/живые прогоны |

## Фазы 0–9 — ядро терминала ✅

Реализованы и проверены (детали и полный список отметок — `SPEC_0-12.md`):

- **0 — Фундамент**: cargo workspace (`finam-proto`, `domain`, `data`,
  `storage`, `app`), дисциплина слоёв (математика в `domain` без внешних
  зависимостей), gRPC-кодоген (vendored proto + vendored protoc, фича `grpc`),
  auth-обмен с кэшем JWT и упреждающим refresh, per-method `RateLimiter`
  (~200 req/мин), `Backoff`, секрет-резолвер env → `.env` → ОС-keyring.
- **1 — Хранилище и ингест**: контракт `Store` (`MemStore`/`DuckStore`),
  миграции, ингест баров/снимков, бэкфилл, асинхронный планировщик
  (`app::ingest`), боевой режим `app --features live` (Finam gRPC → стор).
- **2 — Аналитика**: turnover/directional/unusual volume, money flow/MFI/CVD,
  breadth, sector rollups, RRG, cross-asset shares + flow matrix.
- **3–6 — Оболочка и представления**: ядро IPC + Tauri-привязка (фича `tauri`),
  фронт Vite + Svelte 5 + TS; панели акций/секторов, фьючерсов, облигаций,
  «суммы всех» (gauge, donut, stacked area, Sankey).
- **7 — Live**: серверные стримы с авто-reconnect, DOM-стакан, движок алёртов
  (edge-trigger), offline-replay, панели Time&Sales/DOM/алёртов.
- **8 — Полировка**: настройки представления, чанкинг тяжёлых библиотек,
  метаданные бандла, обработка ошибок. *Осталось:* финальная сборка MSI/NSIS
  и иконки — требуют десктопного окружения (webkit2gtk).
- **9 — V2**: бэктестер (движок + библиотека стратегий + метрики), delta
  (footprint, CVD, роботы-детекторы), симулятор торговли (`SimBroker`, paper
  trading), вкладки фронта, каркас боевого роутинга (`FinamOrderRouter` —
  заглушка за фичей `live-trading`).

## Сквозные предпосылки для фаз 10–12 (S.*) 🟡

- ✅ Вкладочная навигация фронта (8 вкладок, единый интерфейс 0–12).
- ✅ Секционные настройки UI (`lib/settings.ts` + `SettingsTab`).
- ✅ `.env.example` с `MOEX_ALGO_API`; секрет-резолвер поддерживает ключи.
- ✅ **HTTP-слой** `data::http` (reqwest/rustls + `Backoff` + `RateLimiter`,
  `get_json`/`post_json`) — транспортная основа ISS/LLM.
- ✅ Персист настроек/правил в ядро (`app::settings`, `settings.json`,
  атомарная запись, миграция из localStorage).
- ⛔ Egress-allowlist окружения: `apim.moex.com`, `iss.moex.com`,
  `fs.moex.com`, LLM-хосты (`openrouter.ai`, `api.openai.com`,
  `api.anthropic.com`) — блокирует живую сверку (T14).

## Фаза 10 — MOEX ALGO 🟡

**Готово:** доменное ядро полностью (`domain::algo`: Super Candles, FUTOI,
HI2, Mega Alerts; `domain::keyactivity`: правила AND/OR/NOT/IfThen, дефолтный
набор, периоды, LLM-промпт + локальный свод) — всё в тестах без сети. Контракт
API подтверждён (URL `apim.moex.com`, авторизация Bearer). IPC и фронт
Key Activity + ИТОГО — боевые. Вкладка MOEX ALGO с 5 модулями свёрстана.

**Сделано в волнах 1–3:** транспорт `data::moex` (ALGOPACK, пагинация, парсер
ISS на фикстурах, `AlgoSource`); storage v3 (таблицы `algo_*`); IPC
`algo_tradestats`/`algo_futoi`/`algo_hi2`/`algo_mega_alerts` +
`AlgoIngestService`; фронт-модули на типизированном IPC (`algoMock.ts`
удалён); LLM-провайдеры (OpenRouter/Anthropic/OpenAI) с живым ИТОГО, кэшем и
деградацией; персист правил/настроек в ядро.

**Осталось:** живая сверка контракта ISS/ALGOPACK боевым ключом — фикстуры
синтетические `(unverified)`, нужен egress (T14).

## Фаза 11 — Историзация 🟡

**Готово:** доменная модель (`domain::history`: расширенная свеча с
источником/TF, каталог `DatasetMeta`/`Catalog`, нормализация диапазонов,
`missing_ranges`); решение по формату — **DuckDB как основное хранилище,
Parquet как экспорт**; IPC `history_datasets`/`history_delete`/`history_plan`;
вкладка «Данные» (форма загрузки + менеджер датасетов).

**Сделано в волнах 1–4:** `HistorySource` + `FinamHistory`/`MoexHistory`;
storage v4 (`history_bars`/`history_datasets`, upsert/дедуп, инкрементальная
дозагрузка, Parquet экспорт/импорт); сервис `app::history` (очередь, отмена)
+ события `history:*` + IPC `history_load`/`history_cancel`/`history_preview`;
превью датасета свечами; детерминированный мульти-TF фид (`app::feed`).

**Осталось:** живой прогон загрузки с боевыми источниками (egress, T14).

## Фаза 12 — Опционы 🟡

**Готово:** доменное ядро полностью (`domain::options`: Black-76 + Bachelier,
аналитические греки, устойчивый IV-решатель; 4 модели улыбки — MOEX / SABR /
SVI / Каленкович — с общим калибратором Нелдера–Мида и RMSE; конструктор
стратегий: ноги, шаблоны, payoff, греки портфеля, безубытки); IPC
`option_price`/`option_implied_vol`/`smile_fit`/`strategy_eval`/
`list_smile_models`; вкладка «Опционы» (калькулятор, улыбка, конструктор);
справочник `docs/options-smile-models.html`.

**Сделано в волнах 1–3:** доска через публичный ISS (`data::moex::options`,
`OptionsSource` + фейк), маппинг → точки улыбки (веса OI, фильтрация
неликвида, IV из цены), IPC `option_board`, кнопка «Загрузить доску» в
SmileView; `MoexSmile` приведён к срочной структуре методики (σ·√T),
`docs/options-smile-models.html` финализирован.

**Осталось:**
- дословная сверка коэффициентов биржевой улыбки по методике MOEX/НКЦ
  (первоисточники вне egress) и живая сверка контракта доски (T14);
- опц. историзация доски/снимков IV в storage (12.5.1 — отложено сознательно);
- профиль риска стратегии (тепловая карта цена/время) — полировка.

## Порядок работ (статус)

Волны 1–4 из `TASKS_list.md` выполнены (PR #16–#23). Остаток:

1. **P1 (пользователь)**: egress-allowlist (`apim.moex.com`, `iss.moex.com`,
   `fs.moex.com`, LLM-хост) + боевые ключи.
2. **T14**: живой смоук — сверка контрактов ISS/ALGOPACK/доски и LLM-вызова,
   правки парсеров по фактическим ответам.
3. **T15**: упаковка MSI/NSIS + иконки (десктоп-окружение).
4. Полировка вне критического пути: dockview, тепловая карта риска,
   12.5.1, реальный OrderService (вне v1).

## Дисциплина и качество

- Вся математика/правила/промпты — в `domain`, чисто, в тестах без сети.
- Сетевое/тяжёлое — за cargo-фичами (`grpc`, `duckdb`, `tauri`, `ingest`,
  `live`, будущие `http`/`moex`/`llm`); кросс-платформенный CI зелёный без них.
- Секреты — только через резолвер (env → `.env` → keyring), не в localStorage
  и не в логах.
- Каждый PR: `cargo fmt --check`, `cargo clippy -D warnings`,
  `cargo test --workspace`, `npm run check` + `npm run test` + `npm run build`.
- Коммиты — конвенциональные (`feat:`/`fix:`/...), как в истории репозитория.
