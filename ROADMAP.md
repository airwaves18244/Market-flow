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
| 10 | MOEX ALGO (Super Candles · FUTOI · HI2 · Mega · Key Activity · LLM) | 🟡 | транспорт `data::moex`, storage, LLM, боевой IPC 4 модулей |
| 11 | Историзация для бэктестера | 🟡 | `HistorySource`, storage истории, загрузчик, Parquet |
| 12 | Опционы (калькулятор · улыбка · конструктор) | 🟡 | живая доска ISS, `option_board`, верификация методики MOEX |
| — | Сквозное: HTTP-слой, персист настроек, egress | 🟡 | `data::http`, персист правил/настроек, allowlist |

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
- ⛔ **HTTP-слой** `data::http` (reqwest/rustls + `Backoff` + `RateLimiter`) —
  транспортная основа ISS/LLM; gRPC-стек для них не годится.
- ⛔ Egress-allowlist окружения: `apim.moex.com`, `iss.moex.com`,
  LLM-хосты (`openrouter.ai`, `api.openai.com`, `api.anthropic.com`).
- ⛔ Персист приватных настроек/правил в ядро (сейчас — только localStorage).

## Фаза 10 — MOEX ALGO 🟡

**Готово:** доменное ядро полностью (`domain::algo`: Super Candles, FUTOI,
HI2, Mega Alerts; `domain::keyactivity`: правила AND/OR/NOT/IfThen, дефолтный
набор, периоды, LLM-промпт + локальный свод) — всё в тестах без сети. Контракт
API подтверждён (URL `apim.moex.com`, авторизация Bearer). IPC и фронт
Key Activity + ИТОГО — боевые. Вкладка MOEX ALGO с 5 модулями свёрстана.

**Осталось:**
- транспорт `data::moex` (клиент ALGOPACK: tradestats/orderstats/obstats/
  hi2/futoi, пагинация, парсер ISS JSON на фикстурах, трейт `AlgoSource`);
- фикстуры живых ответов (нужны боевой ключ + egress);
- storage-таблицы ALGOPACK + writer'ы + запросы (schema v3);
- IPC `algo_tradestats`/`algo_futoi`/`algo_hi2`/`algo_mega_alerts` + ингест
  ALGOPACK в планировщик;
- перевод модулей Супер-свечи/FUTOI/HI2/Мега с демо-генераторов
  (`lib/algoMock.ts`) на боевой IPC;
- LLM-провайдер (`LlmProvider`: OpenRouter/Anthropic/OpenAI, фича `llm`) +
  подключение к `key_activity_summary`, кэш, деградация без ключа;
- персист правил Key Activity и настроек паспорта в ядро.

## Фаза 11 — Историзация 🟡

**Готово:** доменная модель (`domain::history`: расширенная свеча с
источником/TF, каталог `DatasetMeta`/`Catalog`, нормализация диапазонов,
`missing_ranges`); решение по формату — **DuckDB как основное хранилище,
Parquet как экспорт**; IPC `history_datasets`/`history_delete`/`history_plan`;
вкладка «Данные» (форма загрузки + менеджер датасетов).

**Осталось:**
- трейт `HistorySource` + адаптеры `FinamHistory` (gRPC bars) и `MoexHistory`
  (ISS candles/tradestats);
- storage истории: таблицы, идемпотентный upsert/дедуп, инкрементальная
  дозагрузка, персист `DatasetMeta`, экспорт/импорт Parquet;
- сервис историзации `app::history` (очередь, лимиты, отмена) + события
  `history:progress`/`done`/`error` + IPC `history_load`/`history_cancel`;
- превью датасета (свечи) во вкладке «Данные»;
- фид для бэктестера: мульти-TF `ReplaySource` + детерминированный курсор.

## Фаза 12 — Опционы 🟡

**Готово:** доменное ядро полностью (`domain::options`: Black-76 + Bachelier,
аналитические греки, устойчивый IV-решатель; 4 модели улыбки — MOEX / SABR /
SVI / Каленкович — с общим калибратором Нелдера–Мида и RMSE; конструктор
стратегий: ноги, шаблоны, payoff, греки портфеля, безубытки); IPC
`option_price`/`option_implied_vol`/`smile_fit`/`strategy_eval`/
`list_smile_models`; вкладка «Опционы» (калькулятор, улыбка, конструктор);
справочник `docs/options-smile-models.html`.

**Осталось:**
- загрузка живой опционной доски MOEX через ISS (`data::moex`), маппинг доски
  → точки улыбки, трейт `OptionsSource` + фейк;
- IPC `option_board` + фронт улыбки на живых данных (сейчас — мок-точки);
- опц. историзация доски/снимков IV в storage;
- верификация формы улыбки MOEX по официальной «Методике…» НКЦ, финализация
  `docs/options-smile-models.html`;
- профиль риска стратегии (тепловая карта цена/время) — полировка.

## Порядок работ

1. **HTTP-слой + egress + фикстуры** — разблокируют весь сетевой контур 10–12.
2. **Фаза 10 транспорт** (`data::moex`) — переиспользуется фазами 11
   (MoexHistory) и 12 (опционная доска).
3. **Фаза 11** (storage + загрузчик) и **LLM** — параллельно после транспорта.
4. **Фаза 12 данные** (доска, `option_board`) — после `data::moex`.
5. **Финализация**: персист настроек, превью, упаковка MSI/NSIS (десктоп).

Пошаговая разбивка с исполнителями — `TASKS_list.md`.

## Дисциплина и качество

- Вся математика/правила/промпты — в `domain`, чисто, в тестах без сети.
- Сетевое/тяжёлое — за cargo-фичами (`grpc`, `duckdb`, `tauri`, `ingest`,
  `live`, будущие `http`/`moex`/`llm`); кросс-платформенный CI зелёный без них.
- Секреты — только через резолвер (env → `.env` → keyring), не в localStorage
  и не в логах.
- Каждый PR: `cargo fmt --check`, `cargo clippy -D warnings`,
  `cargo test --workspace`, `npm run check` + `npm run test` + `npm run build`.
- Коммиты — конвенциональные (`feat:`/`fix:`/...), как в истории репозитория.
