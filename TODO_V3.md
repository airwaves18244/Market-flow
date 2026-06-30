# TODO V3 — детальный план реализации (из `ROADMAP_V3.md`)

Это рабочий, исполняемый TODO-лист поверх `ROADMAP_V3.md`: каждая задача
из роадмапа здесь раскрыта до уровня «можно сразу начинать PR» — с **целью**
(что считается готовым), **реализацией** (конкретные типы/сигнатуры/файлы) и
**тестами** (конкретные сценарии, включая отрицательные/edge-кейсы, не только
happy path). ID задач совпадают с `ROADMAP_V3.md` — это его раскрытие, не
параллельная нумерация.

Каждая задача спроектирована как самостоятельный PR (или 2–3 для самых
крупных, отмечено в тексте). Порядок внутри фазы — рекомендованный порядок
реализации (более ранние задачи — зависимости более поздних).

## Соглашения для всех задач этого документа

- **Без сети по умолчанию.** Все `domain`-функции — чистые: вход — срез
  данных (`&[Bar]`, `&[Trade]`, `&[BookSnapshot]`), выход — `Vec<Signal>` /
  типизированная структура. Никакого `async`, никакого I/O.
- **Детерминированные синтетические данные в тестах.** В `domain` нет внешних
  зависимостей (`crates/domain/Cargo.toml` — только `serde`), поэтому
  `rand`-крейт недоступен. Для тестов, которым нужен «органический шум»
  (проверка false-positive rate), реализовать один общий детерминированный
  генератор `domain::testutil::Xorshift32` (xorshift32 PRNG, ~10 строк,
  `#[cfg(test)]`-only модуль, переиспользуется во всех новых тестах фазы 13/18)
  с фиксированными seed'ами в тестах — прогон воспроизводим, не флакает в CI.
- **Структура теста детектора — всегда пара.** Для каждого детектора
  аномального/робот-поведения: (1) тест «образцовый паттерн ловится» на
  сконструированных вручную данных с известным ответом, (2) тест «частота
  ложных срабатываний на органическом/случайном потоке не превышает целевой
  порог» (конкретное число в тесте, не «как-нибудь мало»). Без обоих тестов
  задача не считается завершённой.
- **Тесты — `#[test]` в `mod tests` того же файла**, как везде в проекте
  (не отдельные `*.test.rs`, несмотря на то что говорит auto-сгенерированный
  skill репозитория — реальная конвенция кода подтверждена по факту: inline
  `#[cfg(test)] mod tests`).
- **Каждая задача — отдельный коммит/PR**, проходит `cargo fmt --all --check`,
  `cargo clippy --workspace -- -D warnings`, `cargo test --workspace` (плюс
  `npm run check && npm run test && npm run build` для фронтовых задач).

---

# Фаза 13 — Bot Radar

## 13.0 — Инфраструктура

#### `13.0.1` — Общие типы `domain::botradar`
- **Цель:** единый словарь типов, которым пользуются все детекторы фазы;
  компилируется, сериализуется, без логики обнаружения внутри.
- **Реализация:** новый модуль `crates/domain/src/botradar/mod.rs`:
  ```rust
  pub struct Evidence { pub ts: i64, pub price: f64, pub note: String }
  pub enum SignalKind { Iceberg, MomentumIgnition, PingPong, Heartbeat,
      SizeEntropy, PriceLevelConcentration, Vpin, CrossInstrumentLag,
      BookShapeAnomaly }
  pub struct Signal {
      pub kind: SignalKind, pub ts: i64, pub symbol: String,
      pub score: f64,            // нормировано в [0,1] конструктором/сеттером
      pub evidence: Vec<Evidence>,
  }
  pub struct BookSnapshot { pub ts: i64, pub bids: Vec<BookLevel>, pub asks: Vec<BookLevel> }
  ```
  `Signal::new(...)` зажимает `score` в `[0.0, 1.0]` (`clamp`).
- **Тесты:**
  - `signal_score_is_clamped_to_unit_interval` — `Signal::new(..., score: 1.7, ...)` → `.score == 1.0`; `score: -0.3` → `.score == 0.0`.
  - `signal_round_trips_through_serde_json` — собрать `Signal` с непустым `evidence`, сериализовать/десериализовать, сравнить через `assert_eq!`.
  - `evidence_list_can_be_empty` — `Signal` без evidence не паникует при сериализации.

#### `13.0.2` — `storage`: таблица `book_snapshots`
- **Цель:** периодические снимки DOM сохраняются и читаются по символу/окну
  времени так же, как уже работает `trades`/`bars` (схема v2 → v3).
- **Реализация:** `storage::schema` — DDL `book_snapshots(symbol, ts, bids_json,
  asks_json)`; `SCHEMA_VERSION = 3`; `Store::insert_book_snapshot`,
  `Store::book_snapshots(symbol, from_ts, to_ts) -> Vec<BookSnapshot>` в трейте
  `Store`, реализации в `MemStore` и `DuckStore` (фича `duckdb`).
- **Тесты (`storage`, `MemStore`, кросс-платформенно):**
  - `book_snapshot_insert_and_range_query_is_ordered` — вставить 5 снимков с
    несмежными ts, запросить подвыборку, проверить порядок по возрастанию ts.
  - `book_snapshot_isolated_by_symbol` — вставить снимки для `SBER@MISX` и
    `GAZP@MISX`, запрос по одному символу не возвращает чужие.
  - `range_query_excludes_out_of_window` — снимок строго до/после окна не
    попадает в результат (как уже сделано для `bars`/`trades` — повторить
    паттерн теста `mem::tests::range_query_excludes_out_of_window`).
  - `migrate_schema_v3_adds_book_snapshots_table` — `migrate::tests`: прогон
    миграции с версии 2 поднимает `schema_version` до 3, повторный прогон
    идемпотентен (как `migrate::tests::pending_is_true_for_fresh_or_older_db`).

#### `13.0.3` — Скоринг и объединение сигналов в `RobotProfile`
- **Цель:** детерминированно свернуть произвольный список независимых
  `Signal` за окно в один `RobotProfile` с доминирующим архетипом (архетипы —
  задача `13.3.2`, здесь — только механизм агрегации очков, архетип на этом
  этапе временно `Unclassified`/`None`).
- **Реализация:** `domain::botradar::scoring`:
  ```rust
  pub struct ScoringConfig { pub weights: HashMap<SignalKind, f64> } // Default: все веса = 1.0
  pub struct RobotProfile { pub symbol: String, pub window: (i64, i64),
      pub overall_score: f64, pub signals: Vec<Signal> }
  pub fn build_profile(symbol: &str, window: (i64, i64), signals: &[Signal], cfg: &ScoringConfig) -> RobotProfile
  ```
  `overall_score` — взвешенное среднее по `signals`, см. тест ниже.
- **Тесты:**
  - `empty_signals_yield_zero_score_profile` — пустой срез → `overall_score == 0.0`, `signals.is_empty()`.
  - `profile_score_is_weighted_average` — 2 сигнала, score 0.4 и 0.8, веса 1.0 и 3.0 → `overall_score == (0.4*1.0 + 0.8*3.0) / 4.0` (точное сравнение с допуском `1e-9`).
  - `unknown_signal_kind_falls_back_to_weight_one` — `ScoringConfig` без записи для конкретного `SignalKind` не паникует, использует вес по умолчанию 1.0.

---

## 13.1 — Уровень A: детекторы на доступных данных

#### `13.1.1` — Профиль айсберг-доливок (расширение `delta::robots::detect_icebergs`)
- **Цель:** для каждого обнаруженного айсберг-уровня посчитать стабильность
  интервала доливок и оценку скрытого объёма — отличить алгоритм от ручной
  докупки на одном уровне.
- **Реализация:** `domain::delta::robots`:
  ```rust
  pub struct IcebergProfile {
      pub level_price: f64,
      pub refill_count: usize,
      pub refill_interval_cv: Option<f64>, // None если refill_count < 2
      pub estimated_hidden_qty: f64,       // EWMA по размерам доливок, alpha = 0.3
  }
  pub fn profile_icebergs(trades: &[Trade], book: &OrderBook, mult: f64) -> Vec<IcebergProfile>
  ```
  CV (coefficient of variation) = `stddev(intervals) / mean(intervals)`.
- **Тесты:**
  - `regular_refills_have_low_cv_and_stable_estimate` — 8 доливок размером 50
    с интервалом ровно 2.0с (без шума) → `refill_interval_cv < 0.05`,
    `estimated_hidden_qty` в пределах `[45, 55]`.
  - `irregular_refills_have_high_cv` — интервалы `[0.5, 9.0, 1.2, 7.5, 0.8]`
    (вручную заданные, не псевдослучайные — детерминированность теста) →
    `refill_interval_cv > 0.5`.
  - `single_refill_returns_none_cv_without_panic` — ровно 1 доливка →
    `refill_interval_cv.is_none()`, функция не паникует.
  - `no_trades_returns_empty_vec` — пустой `trades` → `Vec::is_empty()`.

#### `13.1.2` — Momentum ignition / охота за стопами
- **Цель:** найти бар(ы) с импульсным выходом цены за пределы недавнего
  диапазона на подозрительно низком объёме и последующим быстрым откатом.
- **Реализация:** `domain::botradar::momentum`:
  ```rust
  pub fn detect_momentum_ignition(
      bars: &[Bar], lookback: usize, reversion_window: usize, vol_z_threshold: f64,
  ) -> Vec<Signal>
  ```
  Алгоритм: для каждого бара `i >= lookback` — `swing_high/low` за
  `bars[i-lookback..i]`; если `close[i]` выходит за swing ±X% и
  `volume_z = (vol[i] - mean(vol[i-lookback..i])) / stddev(...)  < vol_z_threshold`
  (отрицательный z, т.е. объём аномально мал) — кандидат; подтверждение —
  цена за `reversion_window` баров возвращается минимум на 50% хода обратно
  внутрь диапазона. `score` растёт с полнотой отката.
- **Тесты:**
  - `spike_on_low_volume_with_full_reversion_is_detected` — 30 стабильных
    баров (узкий диапазон), бар 31 — `close` на +3% выше swing high при
    объёме 20% от среднего, бары 32–34 откатывают цену внутрь диапазона
    полностью → 1 сигнал, `score > 0.6`.
  - `spike_on_proportional_volume_is_not_flagged` — тот же ценовой ход, но
    объём = 150% от среднего → `Vec::is_empty()`.
  - `spike_without_reversion_is_not_flagged` — цена удерживается на новом
    уровне все `reversion_window` баров после спайка → `Vec::is_empty()`.
  - `flat_random_walk_false_positive_rate_below_2pct` — 500 баров случайного
    блуждания через `Xorshift32` (seed = 42, шаг ±0.1% от цены, объём ±20% от
    базового) → доля баров с сигналом `< 0.02`.

#### `13.1.3` — «Пинг-понг» принты (wash-подобный паттерн)
- **Цель:** найти статистически маловероятную серию сделок одинаковой цены/
  размера, чередующих сторону агрессора без чистого сдвига цены/объёма,
  частота которых значимо превышает пуассоновский бейзлайн активности тикера.
- **Реализация:** `domain::botradar::pingpong`:
  ```rust
  pub fn detect_ping_pong_prints(tape: &[Trade], window_secs: i64, min_repeats: usize) -> Vec<Signal>
  ```
  Группировка сделок в скользящем окне `window_secs` по ключу `(price, size)`;
  внутри группы — проверка чередования `buyer_initiated`; baseline-частота —
  среднее число сделок такого же `(price,size)` за случайно выбранное окно
  той же длины в остальной части ленты; `score` = `repeats / baseline` через
  `(1 - exp(-x))`-нормировку в `[0,1)`.
- **Тесты:**
  - `alternating_same_price_size_trades_are_flagged` — 20 сделок: цена 100.0,
    размер 10, строго чередование buy/sell, шаг 0.5с → 1 сигнал с
    `repeats == 10` (5 пар), score выше 0.7.
  - `same_price_size_same_side_is_not_flagged` — те же 20 сделок, но все
    одной стороны (не round-trip, обычная агрессивная скупка) → не считается
    ping-pong (`Vec::is_empty()` для этого паттерна, даже если объём большой).
  - `organic_poisson_tape_false_positive_rate_below_1pct` — детерминированно
    сгенерированная (через `Xorshift32`, seed = 7) лента из 2000 сделок со
    случайными ценами/размерами/сторонами, без искусственных повторов →
    доля окон с сигналом `< 0.01` по 20 независимым seed'ам (параметризованный
    тест/цикл по seed в одном `#[test]`).

#### `13.1.4` — «Heartbeat»: таймер-driven роботы
- **Цель:** найти доминирующую периодичность в межсделочных интервалах
  (proxy одного источника потока — фильтр по близкой цене/размеру), которой
  не бывает у событийного/органического потока.
- **Реализация:** `domain::botradar::heartbeat`:
  ```rust
  pub fn detect_heartbeat(timestamps: &[i64], min_snr: f64) -> Option<Signal>
  ```
  Автокорреляция ряда интервалов `Δt_i` по лагам `1..=N/3` (без FFT —
  прямая сумма произведений, `domain` не тянет внешние DSP-крейты; ряд
  коротких лент, `O(N²)` приемлемо при разумных N, ограничить `N <= 2000` на
  вызов и задокументировать ограничение в doc-комментарии); SNR = пик
  автокорреляции / медиана остальных лагов.
- **Тесты:**
  - `exact_periodic_intervals_detected_with_high_snr` — 50 интервалов ровно
    1.0с (без шума) → `Some(signal)`, SNR существенно выше `min_snr = 4.0`.
  - `periodic_intervals_with_small_jitter_still_detected` — интервалы
    `1.0 ± 0.05с` (вручную заданная детерминированная последовательность
    джиттера, не PRNG) → `Some(signal)`.
  - `exponential_like_intervals_not_detected` — вручную заданная
    last-digit-возрастающая последовательность, имитирующая экспоненциальное
    распределение интервалов (без доминирующего пика) → `None`.
  - `fewer_than_min_samples_returns_none_without_panic` — 3 таймстампа →
    `None`, без паники (нужен явный нижний порог, например `< 16` точек).

#### `13.1.5` — Энтропия и «круглость» размеров сделок
- **Цель:** низкая энтропия + аномальная доля «круглых» размеров
  относительно собственной истории инструмента — сигнатура наивного бота с
  фиксированным/круглым лотом.
- **Реализация:** `domain::botradar::sizeprofile`:
  ```rust
  pub fn order_size_entropy(sizes: &[u64]) -> f64       // биты, Shannon
  pub fn round_lot_share(sizes: &[u64], round_to: u64) -> f64  // доля size % round_to == 0
  pub fn size_anomaly_score(window: &[u64], baseline: &[u64]) -> f64 // z-score (entropy, round_share) против baseline, свёрнутый в [0,1]
  ```
- **Тесты:**
  - `uniform_round_sizes_have_low_entropy` — `[100;50]` (50 сделок размером
    ровно 100) → `order_size_entropy == 0.0`, `round_lot_share(..., 100) == 1.0`.
  - `diverse_organic_sizes_have_higher_entropy` — вручную заданный
    разнообразный набор размеров (`[3, 17, 42, 8, 91, 5, 23, ...]`, 20+
    различных значений) → энтропия строго больше, чем в предыдущем тесте
    (сравнение двух прогонов в одном тесте, не абсолютный порог).
  - `size_anomaly_score_flags_round_lot_burst_against_diverse_baseline` —
    `baseline` — разнообразные размеры (как выше), `window` — однородные
    круглые лоты → `size_anomaly_score > 0.6`.
  - `size_anomaly_score_is_low_when_window_matches_baseline_distribution` —
    `window` и `baseline` — одинаковое (или статистически схожее)
    распределение → `score < 0.3`.

#### `13.1.6` — Концентрация объёма по ценовым уровням (переиспользование `algo::hi2`)
- **Цель:** применить уже готовую математику HHI (`domain::algo::hi2`) к
  распределению объёма ленты по ценовым уровням — найти «стены».
- **Реализация:** `domain::botradar::concentration`:
  ```rust
  pub fn price_level_concentration(tape: &[Trade], tick_size: f64) -> Hi2Result
  ```
  Группировка `trade.price` в бакеты `tick_size`, суммирование `size`,
  вызов существующей `algo::hi2::herfindahl(...)` (или эквивалентной
  публичной функции — **проверить и переиспользовать сигнатуру, не
  копировать формулу**, это явное требование задачи: «тонкая обёртка»).
- **Тесты:**
  - `single_dominant_level_yields_high_hhi` — 90% объёма на одном ценовом
    уровне, 10% размазано по 5 другим → `Hi2Result` близок к
    концентрированному краю шкалы (сравнить с эталоном из существующих
    тестов `algo::hi2`, не дублировать константы вручную — импортировать).
  - `evenly_spread_volume_yields_low_hhi` — объём поровну по 10 уровням →
    низкая концентрация.
  - `delegates_to_existing_hi2_implementation` — модульный тест,
    подтверждающий идентичный результат при ручном агрегировании в `HashMap`
    и прямом вызове `algo::hi2::herfindahl` на тех же агрегатах (защита от
    дублирования формулы при рефакторинге).

#### `13.1.7` — VPIN (Volume-Synchronized PIN)
- **Цель:** реализовать индикатор «токсичности» потока — нормированный
  дисбаланс покупок/продаж по объёмным бакетам как ранний индикатор
  повышенной информированной/алгоритмической активности.
- **Реализация:** `domain::botradar::vpin`:
  ```rust
  pub fn volume_buckets(tape: &[Trade], bucket_size: f64) -> Vec<VolumeBucket> // { buy_vol, sell_vol }
  pub fn vpin(buckets: &[VolumeBucket], window: usize) -> Vec<f64> // скользящее окно бакетов
  ```
  `vpin_t = mean(|buy_vol - sell_vol|) / mean(buy_vol + sell_vol)` по
  последним `window` бакетам.
- **Тесты:**
  - `balanced_flow_has_low_vpin` — бакеты с равным buy/sell объёмом →
    `vpin ≈ 0.0` (допуск `1e-9`).
  - `one_sided_flow_has_vpin_near_one` — бакеты, где весь объём — buy →
    `vpin ≈ 1.0`.
  - `vpin_rises_during_synthetic_toxic_period` — серия бакетов: первая
    половина сбалансирована (vpin низкий), вторая — однонаправленная
    (vpin высокий) → ряд `vpin(...)` монотонно растёт между сегментами,
    последнее значение существенно выше первого.
  - `bucket_size_zero_or_negative_does_not_panic` — `bucket_size <= 0.0` →
    функция возвращает `Result`/пустой вектор, не паникует и не делит на 0
    (зафиксировать через `assert!` или явный `Result`-тип в сигнатуре).

#### `13.1.8` — Межинструментный lag/shadowing
- **Цель:** найти устойчивый ненулевой лаг с высокой кросс-корреляцией
  между потоками двух связанных инструментов — признак алгоритма-копира/
  арбитражёра, а не совпадения.
- **Реализация:** `domain::botradar::leadlag`:
  ```rust
  pub struct LagCorrelation { pub best_lag_ms: i64, pub correlation: f64 }
  pub fn cross_instrument_lag(
      tape_a: &[Trade], tape_b: &[Trade], bucket_ms: i64, max_lag_ms: i64,
  ) -> LagCorrelation
  ```
  Агрегировать обе ленты в равные тайм-бакеты (подписанный объём агрессора),
  посчитать кросс-корреляцию Пирсона по сетке лагов
  `-max_lag_ms..=max_lag_ms` шагом `bucket_ms`, вернуть лучший.
- **Тесты:**
  - `synthetic_lagged_copy_is_detected` — `tape_b` строится как точная копия
    `tape_a` со сдвигом таймстампов на ровно `+300мс` → `best_lag_ms == 300`
    (с допуском в один бакет), `correlation > 0.9`.
  - `independent_random_tapes_have_low_correlation` — две независимо
    сгенерированные (разные seed'ы `Xorshift32`) синтетические ленты →
    `correlation < 0.3` на найденном лучшем лаге.
  - `identical_simultaneous_tapes_have_zero_lag` — `tape_b == tape_a` без
    сдвига → `best_lag_ms == 0`, `correlation` около максимума.
  - `empty_tape_returns_zero_correlation_without_panic` — один из срезов
    пуст → `correlation == 0.0`, без деления на 0/паники.

#### `13.1.9` — Самореференсная аномалия формы стакана
- **Цель:** сравнить профиль глубины текущего снимка DOM с историческим
  бейзлайном того же инструмента в то же время сессии — без необходимости
  размеченных данных по рынку в целом.
- **Реализация:** `domain::botradar::bookshape`:
  ```rust
  pub fn depth_profile(snapshot: &BookSnapshot, levels: usize) -> Vec<f64> // нормированный объём по относительным от mid уровням, levels с каждой стороны
  pub fn book_shape_anomaly(current: &[f64], baseline: &[f64]) -> f64 // Jensen-Shannon distance, [0,1]
  ```
- **Тесты:**
  - `identical_profile_to_baseline_has_zero_anomaly` — `current == baseline`
    → `book_shape_anomaly == 0.0`.
  - `symmetric_wall_far_from_touch_is_flagged_as_anomaly` — `baseline` —
    плавно убывающий профиль; `current` — тот же профиль плюс резкий пик
    объёма на 5-м уровне с обеих сторон → `book_shape_anomaly` существенно
    выше порога (например, `> 0.4` при шкале JS-расстояния `[0,1]`).
  - `profiles_with_different_lengths_are_handled_explicitly` — `current` и
    `baseline` разной длины (`levels` отличается) → задокументированное
    поведение: либо `Result::Err`, либо обрезка до `min(len)` — тест
    фиксирует выбранное поведение явно (не оставлять как implementation
    detail без теста).

---

## 13.2 — Уровень B: задел под L3-данные (без реализации алгоритмов)

#### `13.2.1`–`13.2.3` — Трейты-заглушки без логики
- **Цель:** зафиксировать форму контракта, чтобы не блокировать App/Storage
  дизайн, **не реализуя** сами детекторы (они требуют L3-источника,
  которого пока нет — см. «Открытые вопросы» в `ROADMAP_V3.md`).
- **Реализация:** `domain::botradar::level_b` — только типы и
  `#[allow(dead_code)]`-трейт `OrderMessageDetector` с doc-комментарием
  `(verify, требует L3-источник)`, без тела/реализации.
- **Тесты:** компиляционный тест (`cargo check` зелёный — отдельного
  юнит-теста не требуется, т.к. логики нет; явно зафиксировать в PR-описании
  «нет рантайм-кода — только контракт»).

---

## 13.3 — Таксономия, скоринг, «отпечатки»

#### `13.3.1` — Кластеризация поведенческих отпечатков (DBSCAN на чистом Rust)
- **Цель:** сгруппировать «эпизоды» активности в фиче-векторах в кластеры
  без внешних ML-зависимостей, выявляя повторяющиеся «семьи» поведения.
- **Реализация:** `domain::botradar::cluster`:
  ```rust
  pub struct EpisodeFeatures { pub avg_size: f64, pub interval_cv: f64,
      pub dist_from_mid_ticks: f64, pub size_entropy: f64, pub heartbeat_snr: f64 }
  pub fn dbscan(points: &[EpisodeFeatures], eps: f64, min_points: usize) -> Vec<i32> // -1 = шум, иначе id кластера
  ```
  Евклидово расстояние по нормированным (z-score по выборке) признакам;
  классический DBSCAN (региональные запросы — `O(n²)`, документировать
  ограничение по размеру входа, как в `13.1.4`).
- **Тесты:**
  - `two_well_separated_clusters_are_found` — 10 точек вокруг `(0,0,0,0,0)`
    + 10 точек вокруг `(10,10,10,10,10)`, `eps` подобран между группами →
    ровно 2 уникальных непустых id кластера, без пересечения меток между
    группами.
  - `noise_points_are_labeled_minus_one` — 1 точка далеко от всех остальных
    кластеров → её метка `== -1`.
  - `min_points_threshold_is_respected` — кластер из `min_points - 1` точек,
    плотно расположенных → все помечены `-1` (недостаточно для core point).
  - `empty_input_returns_empty_vec` — `points.is_empty()` → `Vec::is_empty()`.

#### `13.3.2` — Таксономия архетипов
- **Цель:** типизированное сопоставление набора активных `Signal` →
  `RobotArchetype` с confidence, в стиле существующих правил
  `domain::keyactivity` (явные типизированные правила, не «чёрный ящик»).
- **Реализация:** `domain::botradar::archetype`:
  ```rust
  pub enum RobotArchetype { MarketMaker, IcebergExecutor, MomentumIgnition,
      StopHunter, CrossAssetArbitrageur, Unclassified }
  pub fn classify(signals: &[Signal]) -> (RobotArchetype, f64 /* confidence */)
  ```
  Правила — явная таблица соответствий доминирующего/комбинации `SignalKind`
  → архетип (например: `Iceberg` доминирует и `score > 0.6` →
  `IcebergExecutor`; `MomentumIgnition` + высокий `score` →
  `StopHunter`/`MomentumIgnition` в зависимости от наличия отката).
- **Тесты:**
  - `dominant_iceberg_signal_classified_as_iceberg_executor` — один сильный
    `Iceberg`-сигнал, остальные слабые/отсутствуют → `RobotArchetype::IcebergExecutor`.
  - `dominant_momentum_signal_classified_as_momentum_ignition` — аналогично
    для `MomentumIgnition`.
  - `mixed_weak_signals_classified_as_unclassified` — несколько сигналов с
    `score < 0.3` каждый → `RobotArchetype::Unclassified`, низкая confidence.
  - `empty_signals_classified_as_unclassified_with_zero_confidence` —
    `signals.is_empty()` → `(Unclassified, 0.0)`.

#### `13.3.3` — Сборка полного `RobotProfile` с доказательной базой
- **Цель:** объединить `13.0.3` (скоринг) + `13.3.2` (архетип) в финальную
  публичную функцию модуля, готовую к вызову из `app`.
- **Реализация:** `domain::botradar::profile_window(symbol, window, signals, cfg) -> RobotProfile`
  (расширяет `RobotProfile` полями `archetype: RobotArchetype`,
  `confidence: f64`).
- **Тесты:**
  - `profile_includes_archetype_and_confidence_consistent_with_signals` —
    собрать набор сигналов, эквивалентный фикстуре из `13.3.2`, убедиться,
    что итоговый `RobotProfile.archetype`/`confidence` совпадают.
  - `profile_evidence_is_traceable_to_source_signals` — каждый `Evidence` в
    итоговом профиле имеет соответствие хотя бы одному входному `Signal`
    (round-trip проверка, не теряются ссылки при агрегации).

---

## 13.4 — Storage / App / Frontend

#### `13.4.1` — `storage`: таблица `robot_signals`
- **Цель:** персистентность результатов сканирования для истории/повторного
  просмотра без пересчёта.
- **Реализация:** `Store::insert_robot_signals`, `Store::robot_signals(symbol, from_ts, to_ts)`;
  DDL аналогичен `book_snapshots` (JSON-колонка для `evidence`).
- **Тесты (storage, MemStore):**
  - `robot_signal_insert_and_range_query_is_ordered`
  - `robot_signal_isolated_by_symbol`
  - `robot_signal_kind_round_trips_through_storage` (enum ↔ строка в БД не
    теряет значение — explicit match на все варианты `SignalKind` в тесте,
    чтобы добавление нового варианта без обновления маппинга падало
    компиляцией/тестом).

#### `13.4.2` — `app::botradar` (фича `botradar`): фоновый скан-таск
- **Цель:** периодический прогон уровня-A детекторов по вотчлисту, как
  `IngestService`, без сети по умолчанию (работает на уже сохранённых
  `trades`/`bars`/`book_snapshots`).
- **Реализация:** `BotRadarService::tick(&self, store: &dyn Store, watchlist: &[String]) -> Vec<RobotProfile>`,
  тактируется как `IngestService::tick` (без сети — фейковый `Store` в тестах).
- **Тесты (`app`, фича `botradar`, без сети):**
  - `tick_scans_each_watchlist_symbol_and_persists_profiles` — `MemStore` с
    предзаполненными `trades` для 2 символов → после `tick()` оба символа
    имеют записи в `robot_signals`.
  - `tick_skips_symbols_without_data_without_error` — символ без данных в
    хранилище → не паникует, профиль с нулевым score либо пропуск (явно
    зафиксировать поведение тестом).
  - `tick_is_idempotent_safe_to_rerun` — два последовательных `tick()` на
    неизменных данных не создают дублирующихся «активных» сигналов сверх
    ожидаемого (зависит от стратегии дедупликации — зафиксировать решение
    тестом).

#### `13.4.3` — IPC-команды
- **Цель:** `robot_profile(symbol, fromTs, toTs)` и `robot_signals_scan(symbol,
  fromTs, toTs, config)` — расширение существующего паттерна `robot_scan`.
- **Реализация:** `crates/app/src/api.rs` + DTO в `dto.rs`
  (`RobotProfileDto`, `RobotSignalDto` — расширить уже существующий
  `RobotSignalDto`, не плодить дубликат) + регистрация в `tauri_app.rs`.
- **Тесты (`app`, на `MemStore`):**
  - `robot_profile_handler_returns_dto_matching_domain_profile`
  - `robot_signals_scan_handler_respects_time_window`
  - `robot_profile_handler_on_empty_store_returns_unclassified_dto` (не
    `Err`, если данных просто нет за окно — зафиксировать контракт).

#### `13.4.4` — Frontend: вкладка/панель «Bot Radar» *(делить на 2 PR)*
**PR 1 — данные и каркас:**
- **Цель:** новая вкладка в `TabBar`, IPC-клиент, мок-данные, базовый макет
  без интерактивности (loading/empty/error состояния — обязательны, как
  указано в `design/claude-design-brief.md`).
- **Реализация:** `frontend/src/lib/components/BotRadarView.svelte` +
  IPC-методы в `ipc.ts` (`robotProfile`, `robotSignalsScan`) + мок-генератор
  в `mock.ts` (детерминированный, без `Math.random()` для тестируемости —
  см. ниже).
- **Тесты (vitest, по образцу уже добавленных `ipc.test.ts`):**
  - `ipc.test.ts`: `robot_profile mock command returns a profile with a symbol and score in [0,1]`.
  - `ipc.test.ts`: `robot_signals_scan mock command respects a limit/window argument`.
  - `ipc.test.ts`: `rejects an unknown botradar command` (расширение
    существующего паттерна с `unknown command`).

**PR 2 — визуализация и drill-down:**
- **Цель:** тепловая карта тикеров по score (ECharts heatmap, как Фаза 4),
  лента сигналов в реальном времени, карточка профиля с архетипом, клик по
  сигналу подсвечивает связанные сделки на `DeltaChart`.
- **Тесты:** `npm run check` зелёный (типы пропсов компонентов); если в
  компонент выносится чистая логика форматирования/сортировки — покрыть
  отдельным `*.test.ts` по аналогии с `assetClass.test.ts`/`settings.test.ts`
  (например, функция сортировки тикеров по score для heatmap — чистая,
  тестируемая без рендера компонента).

#### `13.4.5` — Интеграция с `domain::keyactivity` (LLM-контекст)
- **Цель:** сигналы Bot Radar — новая «область» в правилах Key Activity,
  попадают в сборку LLM-промпта.
- **Реализация:** расширить enum «область» в `keyactivity` новым вариантом
  (например `Domain::BotRadar`), добавить дефолтное правило в набор по
  умолчанию.
- **Тесты:**
  - `default_ruleset_includes_a_bot_radar_rule`
  - `prompt_assembly_includes_bot_radar_evidence_when_rule_triggers` —
    собрать промпт с активным правилом Bot Radar, убедиться, что текст
    промпта содержит ожидаемую секцию/маркер (строковая проверка, как уже
    тестируется сборка промпта в существующих тестах `keyactivity`).

---

# Фаза 14 — Мультиактивный бэктестинг, оптимизация, издержки

#### `14.1` — Портфельный бэктестер
- **Цель:** один движок прогоняет несколько символов синхронно по времени,
  с неттингом позиций и единой кривой капитала портфеля.
- **Реализация:** `domain::backtest::portfolio`:
  ```rust
  pub struct PortfolioConfig { pub initial_capital: f64, pub commission: f64, pub slippage: f64 }
  pub struct PortfolioReport { pub equity_curve: Vec<(i64, f64)>, pub per_symbol: HashMap<String, BacktestReport> }
  pub fn run_portfolio_backtest(
      bars_by_symbol: &HashMap<String, Vec<Bar>>,
      strategies_by_symbol: &HashMap<String, Box<dyn Strategy>>,
      cfg: &PortfolioConfig,
  ) -> PortfolioReport
  ```
- **Тесты:**
  - `two_uncorrelated_symbols_combine_into_single_equity_curve` — 2 символа
    с разными, не совпадающими по времени сделками → итоговая
    `equity_curve` отражает сумму обоих P&L в каждой точке времени
    (явная сверка нескольких контрольных точек).
  - `per_symbol_reports_match_standalone_single_symbol_backtest` — прогон
    одного и того же символа через портфельный движок и через существующий
    `run_backtest` (Фаза 9) дают идентичный `BacktestReport` для этого
    символа (регрессионная проверка, что портфельный движок не меняет
    однострочную логику).
  - `commission_and_slippage_applied_per_fill_across_symbols` — ненулевая
    комиссия/слиппедж уменьшают итоговый капитал на ожидаемую сумму при
    известном числе сделок по обоим символам.

#### `14.2` — Walk-forward анализ
- **Цель:** скользящие окна train/test + метрика деградации out-of-sample.
- **Реализация:** `domain::backtest::walkforward::run_walk_forward(bars, strategy_factory, train_len, test_len, step) -> Vec<WalkForwardFold>`
  (`WalkForwardFold { train_sharpe: f64, test_sharpe: f64 }`).
- **Тесты:**
  - `folds_cover_entire_series_without_gaps_or_overlap_beyond_step` —
    проверка, что объединение train/test окон по всем фолдам покрывает
    входной ряд согласно `step` (арифметика индексов, не реальная стратегия).
  - `degraded_out_of_sample_strategy_shows_lower_test_sharpe` — синтетическая
    «стратегия», прибыльная только на первой половине ряда (искусственно
    сконструированный сигнал) → среднее `test_sharpe` по фолдам второй
    половины заметно ниже `train_sharpe`.
  - `insufficient_data_for_one_fold_returns_empty_without_panic` —
    `bars.len() < train_len + test_len` → `Vec::is_empty()`.

#### `14.3` — Оптимизация параметров (grid + random search)
- **Цель:** перебор параметров стратегии с защитой от переобучения через
  `14.2`.
- **Реализация:** `domain::backtest::optimize::grid_search(bars, strategy_id, param_grid: &[ParamRange], objective: Objective) -> Vec<OptimizationResult>`;
  `random_search(..., n_trials: usize, seed: u64)` — детерминированный PRNG
  (`Xorshift32`, вынесенный из тестового модуля в `domain::util` для
  переиспользования вне тестов — единственное место, где этот генератор
  нужен в продакшен-коде).
- **Тесты:**
  - `grid_search_evaluates_full_cartesian_product` — сетка `2x3` параметров
    → ровно 6 результатов.
  - `random_search_is_deterministic_for_fixed_seed` — два вызова с одним
    `seed` дают идентичные результаты (побитово/поэлементно).
  - `best_result_matches_max_objective_value` — среди результатов
    `grid_search` ручная проверка `argmax` по `objective` совпадает с тем,
    что функция считает «лучшим» (если есть отдельный `best()` хелпер).

#### `14.4` — Monte Carlo / bootstrap по сделкам
- **Цель:** ресэмплинг последовательности сделок → доверительные интервалы
  метрик, оценка risk-of-ruin.
- **Реализация:** `domain::backtest::montecarlo::bootstrap_trades(trades: &[TradeResult], n_resamples: usize, seed: u64) -> MonteCarloReport`
  (`MonteCarloReport { pnl_p5: f64, pnl_p50: f64, pnl_p95: f64, risk_of_ruin: f64 }`).
- **Тесты:**
  - `bootstrap_is_deterministic_for_fixed_seed`
  - `all_winning_trades_yield_zero_risk_of_ruin` — синтетический набор
    сделок, все с положительным P&L → `risk_of_ruin == 0.0`.
  - `mostly_losing_trades_yield_high_risk_of_ruin` — набор с
    систематическими убытками, превышающими капитал в худших ресэмплах →
    `risk_of_ruin` существенно `> 0`.
  - `percentiles_are_monotonic` — `pnl_p5 <= pnl_p50 <= pnl_p95` всегда
    (инвариант, проверяется на нескольких seed'ах в одном тесте).

#### `14.5` — Модель проскальзывания и market impact
- **Цель:** линейная и «√объём» модели импакта, калибруемые по
  историческому спреду/объёму инструмента.
- **Реализация:** `domain::backtest::impact::{linear_impact, sqrt_impact}(qty, avg_volume, spread) -> f64`.
- **Тесты:**
  - `linear_impact_scales_proportionally_with_quantity` — удвоение `qty` →
    удвоение импакта (с допуском).
  - `sqrt_impact_grows_slower_than_linear_for_large_orders` — для большого
    `qty` `sqrt_impact(...) < linear_impact(...)` при одинаковых остальных
    параметрах.
  - `zero_quantity_yields_zero_impact` — `qty == 0` → `0.0`, без деления на 0
    при `avg_volume == 0` тоже (явный edge-кейс тест).

#### `14.6` — TCA (Transaction Cost Analysis)
- **Цель:** сравнение факта исполнения с бенчмарками (arrival price, VWAP
  периода).
- **Реализация:** `domain::backtest::tca::analyze(fills: &[Fill], benchmark: Benchmark) -> TcaReport`
  (`Benchmark::Arrival(f64) | Benchmark::Vwap(f64)`, `TcaReport { slippage_bps: f64 }`).
- **Тесты:**
  - `fill_at_benchmark_price_has_zero_slippage`
  - `fill_worse_than_benchmark_has_positive_slippage_bps_for_buys` (и
    симметрично — `negative` для продаж с худшей ценой, в зависимости от
    знаковой конвенции, зафиксированной doc-комментарием и тестом).
  - `multiple_fills_aggregate_volume_weighted_slippage`.

#### `14.7` — Мониторинг распада альфы
- **Цель:** алёрт при устойчивой деградации скользящего Sharpe/win-rate в
  paper-режиме.
- **Реализация:** `domain::backtest::decay::AlphaDecayMonitor::push(&mut self, trade_pnl: f64) -> Option<Signal>`
  (edge-triggered, как `AlertEngine`).
- **Тесты:**
  - `sustained_negative_rolling_sharpe_triggers_signal_once` (edge-trigger —
    повторные обновления при удерживаемом состоянии не плодят сигнал
    повторно, паттерн как у существующего `AlertEngine`).
  - `recovery_above_threshold_resets_trigger_state` — после восстановления
    выше порога новое падение снова триггерит сигнал.

#### `14.8` — Frontend: портфельный режим, walk-forward, Monte Carlo
- **Цель:** UI-расширение `Backtester.svelte` без поломки текущего
  однострочного режима.
- **Тесты:** `npm run check`/`npm run test`/`npm run build` зелёные; чистые
  функции форматирования (например, перевод `MonteCarloReport` в данные для
  fan-chart ECharts) — покрыть `*.test.ts` отдельно от компонента.

#### `14.9` — Сводные тесты фазы
- **Цель:** интеграционный smoke-тест полного пути «портфель → walk-forward
  → оптимизация → Monte Carlo» на едином синтетическом датасете, чтобы
  ловить рассинхронизацию контрактов между подмодулями.
- **Тесты:** `portfolio_walkforward_optimize_montecarlo_pipeline_smoke` — один
  `#[test]` в `crates/domain/tests/` (интеграционная директория, не
  `mod tests` внутри файла — пересекает модули), без сети, на фикстуре из
  ~200 синтетических баров.

---

# Фаза 15 — Опционы 2.0

#### `15.1` — Options Flow: необычная активность и sweep-детекция
- **Цель:** объём/OI ratio по страйку + детекция «sweep» (серия мелких
  сделок по нескольким страйкам/сериям в короткое окно).
- **Реализация:** `domain::options::flow`:
  ```rust
  pub fn volume_oi_ratio(strikes: &[StrikeActivity]) -> Vec<(f64 /*strike*/, f64 /*ratio*/)>
  pub fn detect_sweep(trades: &[OptionTrade], window_secs: i64, min_strikes: usize) -> Vec<Signal>
  ```
- **Тесты:**
  - `high_volume_oi_ratio_flags_unusual_strike` — один страйк с объёмом,
    кратно превышающим OI → попадает в верх отсортированного списка.
  - `sweep_across_multiple_strikes_in_short_window_is_detected` — 5 мелких
    сделок по 5 разным страйкам одной экспирации за 2 секунды,
    однонаправленных по стороне → 1 `Signal`.
  - `scattered_trades_over_long_window_not_flagged_as_sweep` — те же сделки,
    но растянутые на 10 минут → `Vec::is_empty()`.

#### `15.2` — Gamma Exposure (GEX)
- **Цель:** оценка чистой гаммы маркет-мейкеров по доске при стандартных
  допущениях о стороне дилера, уровень «gamma flip».
- **Реализация:** `domain::options::gex::{net_gamma_by_strike, gamma_flip_level}`
  — переиспользует греки из `domain::options::pricing` (не пересчитывает
  формулы заново).
- **Тесты:**
  - `gex_known_board_matches_hand_calculated_value` — маленькая (3–4 страйка)
    синтетическая доска с известными вручную посчитанными гаммами/OI →
    `net_gamma_by_strike` совпадает с ручным расчётом с допуском `1e-6`.
  - `gamma_flip_level_is_between_negative_and_positive_regions` — доска,
    сконструированная так, что net gamma меняет знак ровно на известном
    страйке → `gamma_flip_level` совпадает с этим страйком.

#### `15.3` — Sentiment-индикаторы
- **Цель:** put/call ratio (объём и OI), term structure skew, 25-delta risk
  reversal на базе уже откалиброванных улыбок.
- **Реализация:** `domain::options::sentiment::{put_call_ratio, risk_reversal_25d}`.
- **Тесты:**
  - `put_call_ratio_above_one_when_puts_dominate`
  - `risk_reversal_sign_matches_skew_direction` — синтетическая улыбка с
    явным перекосом в путы → `risk_reversal_25d` отрицательный (или
    зафиксированный знак по конвенции, явно задокументированной).

#### `15.4` — Бэктестер опционных стратегий + greek P&L attribution
- **Цель:** прогон шаблонов стратегий (`domain::options::strategy`) по
  исторической доске с разложением P&L на delta/gamma/theta/vega.
- **Реализация:** `domain::options::backtest::{run_strategy_backtest, attribute_pnl}`
  поверх движка Фазы 14.1.
- **Тесты:**
  - `covered_call_backtest_matches_hand_calculated_payoff_at_expiry` —
    однопериодный сценарий (одна точка входа, экспирация) с известным
    исходом → итоговый P&L совпадает с ручным расчётом payoff.
  - `pnl_attribution_components_sum_to_total_pnl` — `delta_pnl + gamma_pnl +
    theta_pnl + vega_pnl ≈ total_pnl` с заданным допуском (классическая
    проверка консистентности greek attribution).

#### `15.5` — Vol surface во времени + событийная разметка
- **Цель:** таймлайн поверхности волатильности с разметкой событий
  (экспирация, дивотсечка) и обнаружением «vol crush».
- **Реализация:** `domain::options::surface_timeline::{detect_vol_crush}`.
- **Тесты:**
  - `iv_drop_after_expiry_event_is_flagged_as_vol_crush` — синтетический ряд
    IV с резким падением сразу после отмеченного события экспирации →
    сигнал.
  - `gradual_iv_decline_without_event_is_not_flagged` — постепенное падение
    IV без привязки к событию → `Vec::is_empty()`.

#### `15.6` — Frontend
- **Тесты:** `npm run check/test/build`; чистые форматтеры (сортировка
  ленты Flow, подготовка данных GEX-чарта) — отдельные `*.test.ts`.

#### `15.7` — `(verify)` Доступность исторической опционной доски
- **Цель:** не реализация, а исследовательская задача — зафиксировать
  ответ через тест-фикстуру по аналогии с `10.0.4` (живой ответ → сохранить
  как фикстуру, дальше парсер тестируется офлайн).
- **Тесты:** парсер фикстуры (когда появится) — `parses_known_fixture_into_option_board`
  (структурный тест на сохранённом JSON, не сетевой).

---

# Фаза 16 — Межрыночные потоки

#### `16.1` — Risk-on/Risk-off композит
- **Цель:** индекс относительной силы акций vs защитных активов на базе уже
  готовой RRG-математики (Фаза 4), применённой к классам активов.
- **Реализация:** `domain::metrics::crossasset::risk_on_off_index(class_rollups: &[AssetClassRollup]) -> f64`
  (переиспользует `rrg::rs_ratio`/`rs_momentum`, не копирует формулу).
- **Тесты:**
  - `equities_outperforming_bonds_yields_risk_on_positive_index`
  - `bonds_outperforming_equities_yields_risk_off_negative_index`
  - `delegates_rs_calculation_to_existing_rrg_module` (как в `13.1.6` —
    защита от дублирования формулы).

#### `16.2` — Корреляции и скользящая бета
- **Цель:** матрица корреляций между классами/секторами/инструментами на
  скользящем окне + детекция «разрыва корреляции».
- **Реализация:** `domain::metrics::correlation::{rolling_correlation_matrix, correlation_break}`.
- **Тесты:**
  - `perfectly_correlated_series_yield_correlation_one` — `series_b = 2 *
    series_a` → корреляция `≈ 1.0`.
  - `inverted_series_yield_correlation_minus_one`.
  - `correlation_break_detected_when_regime_changes_mid_window` —
    синтетический ряд: первая половина сильно коррелирована, вторая —
    декоррелирована → сигнал разрыва около точки перехода.

#### `16.3` — Базис фьючерс-спот и календарные спреды
- **Цель:** контанго/бэквордация по срокам, сигнал на аномальное расширение
  базиса.
- **Реализация:** `domain::metrics::basis::{futures_basis, basis_zscore}`.
- **Тесты:**
  - `futures_above_spot_is_contango_positive_basis`
  - `futures_below_spot_is_backwardation_negative_basis`
  - `basis_zscore_flags_extreme_widening_relative_to_history`.

#### `16.4` — Lead-lag между классами активов
- **Цель:** переиспользовать `13.1.8` на агрегированных потоках классов.
- **Реализация:** тонкая обёртка `domain::metrics::leadlag::asset_class_lag(...)`
  поверх `botradar::leadlag::cross_instrument_lag`.
- **Тесты:**
  - `delegates_to_cross_instrument_lag_implementation` (как `13.1.6`/`16.1`).
  - `futures_leading_equities_detected_with_expected_sign_of_lag`.

#### `16.5` — Сезонность
- **Цель:** профиль оборота/потоков по времени суток/дню недели/месяцу.
- **Реализация:** `domain::metrics::seasonality::{by_weekday, by_hour_of_day}`.
- **Тесты:**
  - `seasonality_buckets_aggregate_correct_weekday`
  - `single_data_point_does_not_panic_on_stddev_computation` (n=1 → `stddev`
    не делит на 0/не паникует, edge-кейс как и везде в проекте).

#### `16.6` — Frontend
- **Тесты:** стандартный набор `check/test/build`; чистая логика подготовки
  heatmap-матрицы корреляций — `*.test.ts`.

#### `16.7` — Сводный тест фазы
- **Тесты:** `crossasset_rotation_pipeline_smoke` — синтетический мультикласс
  датасет → risk-on/off индекс, корреляции и базис считаются без паники и
  дают согласованные знаки на заведомо одностороннем синтетическом сценарии.

---

# Фаза 17 — Облигации и FX

#### `17.1` — Сигналы формы кривой доходности
- **Цель:** бабочка/стипенер/флэттенер относительно собственной исторической
  нормы (z-score формы).
- **Реализация:** `domain::metrics::yieldcurve::{curve_shape_zscore, classify_shape_change}`.
- **Тесты:**
  - `parallel_shift_does_not_trigger_shape_signal` — равномерный сдвиг всех
    точек кривой на одну величину → форма не изменилась → нет сигнала.
  - `steepening_is_classified_correctly` — короткий конец вниз, длинный
    вверх → `Steepener`.
  - `flattening_is_classified_correctly` — обратное движение → `Flattener`.

#### `17.2` — Дюрация/выпуклость портфеля
- **Цель:** сценарный P&L от сдвига/наклона кривой для портфеля
  облигационных позиций.
- **Реализация:** `domain::trading::bonds::{portfolio_duration, scenario_pnl}`.
- **Тесты:**
  - `scenario_pnl_matches_duration_approximation_for_small_shift` — для
    малого параллельного сдвига `ΔPnL ≈ -duration * Δyield * price`
    (сравнение с допуском, классическая first-order проверка).
  - `convexity_term_improves_estimate_for_large_shift` — для большого сдвига
    оценка с поправкой на выпуклость ближе к «точному» синтетическому
    эталону, чем чисто дюрационная (сравнение двух оценок).

#### `17.3` — Кредитные спреды
- **Цель:** ранжирование эмитентов по динамике спреда vs ОФЗ сопоставимой
  дюрации.
- **Реализация:** `domain::metrics::credit::{spread_to_benchmark, rank_by_spread_widening}`.
- **Тесты:**
  - `issuer_with_widening_spread_ranks_above_stable_issuer`
  - `missing_benchmark_point_for_duration_is_handled_explicitly` (интерполяция
    или явный `Option::None` — зафиксировать тестом выбранное поведение).

#### `17.4` — FX carry и базис
- **Цель:** калькулятор carry trade, таблица спот/форвард, кросс-валютный
  базис.
- **Реализация:** `domain::metrics::fx::{carry_return, cross_currency_basis}`.
- **Тесты:**
  - `positive_rate_differential_yields_positive_carry`
  - `covered_interest_parity_holds_in_frictionless_synthetic_case` —
    синтетические ставки/форвардные пункты, подобранные так, чтобы паритет
    выполнялся точно → `cross_currency_basis ≈ 0.0`.
  - `basis_deviation_from_parity_is_detected` — намеренно «сломанный»
    синтетический набор → `cross_currency_basis` существенно отличен от 0.

#### `17.5` — `(verify)` Источник данных FX/бонды
- Аналогично `15.7` — исследовательская задача, тест появляется вместе с
  фикстурой парсера.

#### `17.6` — Frontend
- **Тесты:** `check/test/build`; чистые форматтеры кривой/carry-таблицы.

#### `17.7` — Сводный тест фазы
- **Тесты:** `bonds_fx_pipeline_smoke` — синтетический набор кривой/ставок →
  весь конвейер (форма кривой → дюрация-сценарий → кредитные спреды →
  carry) без паники, с согласованными знаками на одностороннем сценарии.

---

# Фаза 18 — Статистика и режимы рынка

#### `18.1` — Детекция режима рынка
- **Цель:** простая HMM/changepoint (чистый Rust) — тренд/флэт/выс.вол/низ.вол.
- **Реализация:** `domain::regime::{detect_regime, RegimeKind}` —
  пороговый классификатор по реализованной волатильности + направленности
  (без полноценного байесовского вывода в первой итерации; задокументировать
  упрощение явно в doc-комментарии модуля).
- **Тесты:**
  - `low_volatility_trending_series_classified_as_trend`
  - `high_volatility_choppy_series_classified_as_high_vol_range`
  - `regime_changes_are_detected_at_approximately_correct_index` —
    синтетический ряд со сменой режима на известном индексе → обнаруженная
    точка перехода в пределах допуска `± window/2`.

#### `18.2` — Прогноз волатильности (EWMA, GARCH(1,1))
- **Цель:** реализация без внешних ML-зависимостей.
- **Реализация:** `domain::volatility::{ewma_vol, garch11_fit_and_forecast}`.
- **Тесты:**
  - `ewma_vol_reacts_faster_than_simple_average_to_shock` — после резкого
    скачка доходности EWMA-оценка растёт быстрее простого скользящего
    среднего за то же окно.
  - `garch_forecast_converges_to_long_run_variance_on_constant_input` —
    постоянная (нулевая дисперсия) входная серия → прогноз сходится к
    ожидаемому значению без NaN/расхождения.
  - `garch_fit_does_not_diverge_on_synthetic_volatility_clustering_series` —
    вручную сконструированный ряд с кластерами волатильности → параметры
    остаются в допустимых границах (`alpha + beta < 1`, стационарность).

#### `18.3` — Парный трейдинг / коинтеграция
- **Цель:** тест коинтеграции (Engle-Granger) + z-score спреда как новая
  стратегия бэктестера.
- **Реализация:** `domain::stats::cointegration::engle_granger_test`;
  `domain::backtest::strategies::pairs` (новая стратегия в библиотеке,
  совместимая с трейтом `Strategy`).
- **Тесты:**
  - `cointegrated_synthetic_pair_passes_test` — `b = a + stationary_noise`
    (вручную сконструированный стационарный шум) → тест признаёт
    коинтеграцию (p-value/статистика ниже порога).
  - `independent_random_walks_fail_cointegration_test` — два независимых
    случайных блуждания (детерминированный `Xorshift32`, разные seed) →
    тест не признаёт коинтеграцию.
  - `pairs_strategy_enters_on_extreme_zscore_and_exits_on_reversion` —
    интеграционный тест стратегии поверх движка Фазы 9/14: спред уходит за
    порог → вход; спред возвращается к среднему → выход, проверка через
    `BacktestReport.trades`.

#### `18.4` — Факторный анализ секторных потоков (PCA)
- **Цель:** топ-компоненты по `sector_rollup` степенным методом (без
  внешних линал-библиотек).
- **Реализация:** `domain::stats::pca::top_component_power_iteration(matrix, n_components, iters) -> PcaResult`.
- **Тесты:**
  - `power_iteration_recovers_known_dominant_eigenvector` — синтетическая
    матрица с заведомо известным доминирующим собственным вектором
    (сконструированная вручную, например диагональная с одним большим
    значением) → результат совпадает с эталоном с допуском.
  - `orthogonal_synthetic_factors_are_separated_into_distinct_components` —
    данные, явно построенные как сумма двух ортогональных факторов →
    `n_components = 2` находит оба с ожидаемым соотношением объяснённой
    дисперсии.

#### `18.5` — Frontend
- **Тесты:** бейдж режима — чистая функция выбора иконки/цвета по
  `RegimeKind` (`*.test.ts`); панель парного трейдинга — `check/test/build`.

#### `18.6` — Сводный тест фазы
- **Тесты:** `regime_volatility_pairs_pipeline_smoke` на едином синтетическом
  датасете с известной структурой (один режим-переход, одна коинтегрированная
  пара) — весь конвейер без паники, ожидаемые сигналы присутствуют.

---

# Фаза 19 — Риск-менеджмент и портфель

#### `19.1` — VaR/CVaR
- **Цель:** исторический и параметрический расчёт на портфель позиций
  симулятора.
- **Реализация:** `domain::risk::{historical_var, parametric_var, cvar}`.
- **Тесты:**
  - `historical_var_matches_manual_percentile_on_small_known_sample` —
    маленький (10–20 значений) вручную заданный ряд P&L → `VaR(95%)`
    совпадает с ручным перцентилем.
  - `cvar_is_always_greater_than_or_equal_to_var_in_magnitude` — инвариант
    (CVaR — хвостовое среднее за VaR) проверяется на нескольких синтетических
    распределениях P&L.
  - `parametric_var_matches_historical_var_on_normal_synthetic_returns` —
    сгенерированный (детерминированно, через обратное Гауссово
    преобразование от `Xorshift32`) ряд, близкий к нормальному → два метода
    дают близкий результат (допуск, не точное совпадение).

#### `19.2` — Стресс-тесты по сценариям
- **Цель:** пользовательские и исторические сценарии (шок цены/волатильности/
  корреляции).
- **Реализация:** `domain::risk::stress::{apply_scenario, ScenarioShock}`.
- **Тесты:**
  - `price_shock_scenario_reduces_long_only_portfolio_value_as_expected` —
    известный шок −10% на все позиции long → итоговая оценка портфеля
    падает на сопоставимую величину (с учётом весов).
  - `correlation_shock_increases_portfolio_var_for_diversified_book` —
    повышение корреляций между некоррелированными изначально позициями →
    VaR портфеля растёт (диверсификационный эффект ослабевает).

#### `19.3` — Позиционный сайзинг (Kelly, risk parity)
- **Цель:** дробный Kelly + risk parity между стратегиями/инструментами.
- **Реализация:** `domain::risk::sizing::{fractional_kelly, risk_parity_weights}`.
- **Тесты:**
  - `kelly_fraction_matches_textbook_formula_for_known_win_probability` —
    известные `win_prob`/`win_loss_ratio` → результат совпадает с
    классической формулой Келли `f* = p - (1-p)/b` (точная проверка).
  - `risk_parity_weights_equalize_risk_contribution` — для портфеля с
    известными волатильностями инструментов веса обратно пропорциональны
    волатильности (классический частный случай risk parity без корреляций);
    проверка через расчёт risk contribution каждого актива после применения
    весов — равны с допуском.

#### `19.4` — Drawdown circuit breaker
- **Цель:** автостоп стратегии/сессии при превышении лимита просадки —
  расширение `domain::trading::risk` (уровень заявки → уровень сессии).
- **Реализация:** `domain::trading::risk::SessionRiskGuard::{push_equity, is_tripped}`.
- **Тесты:**
  - `breaker_trips_when_drawdown_exceeds_configured_limit`
  - `breaker_does_not_trip_below_limit`
  - `breaker_stays_tripped_until_explicit_reset` (не «отпускает» сама по
    себе при частичном восстановлении, если так задизайнено — зафиксировать
    тестом выбранное поведение, как и в `14.7`).

#### `19.5` — Frontend: риск-дашборд + настройки лимитов
- **Реализация:** новая секция `risk` в `lib/settings.ts` (по схеме `S.2.1`).
- **Тесты:** расширение существующего `settings.test.ts` —
  `risk section defaults are populated`, `legacy settings without risk section migrate cleanly`
  (обратная совместимость со старым плоским объектом — явное требование
  `S.2.1` из `ROADMAP_PHASE_10-12.md`).

#### `19.6` — Сводный тест фазы
- **Тесты:** `risk_pipeline_smoke` — портфель с известной просадкой проходит
  VaR/CVaR/Kelly/circuit-breaker без паники, breaker трипается на
  заведомо превышающем лимит сценарии.

---

# Фаза 20 — Инфраструктура алготрейдера

#### `20.1` — Внешние уведомления (Telegram/webhook)
- **Цель:** канал доставки алёртов; секрет — через существующий резолвер.
- **Реализация:** `data::notify` (фича `notify`): трейт `Notifier`,
  `TelegramNotifier`/`WebhookNotifier`; чистая сборка сообщения — отдельно
  от сетевого вызова (как `data::dotenv`/`data::Method` — чистое отделено
  от сетевого).
- **Тесты:**
  - `message_formatting_for_robot_signal_is_human_readable` — чистая функция
    `format_signal_message(&Signal) -> String`, без сети — проверка на
    конкретный ожидаемый текст/наличие ключевых полей.
  - `notifier_trait_object_is_mockable_in_tests` — фейковая реализация
    `Notifier` в тестах фиксирует факт вызова с ожидаемым телом сообщения
    (как `AuthTransport`/фейковый `MarketData` в существующих тестах
    `IngestService`).

#### `20.2` — Отчёты и дневник сделок
- **Цель:** экспорт CSV + журнал сделок с тегами/заметками.
- **Реализация:** `domain::report::trades_to_csv(&[TradeResult]) -> String` (чистая
  функция, без файлового I/O — запись в файл делает `app`); `storage`:
  таблица `trade_journal` (символ, ts, теги, заметка).
- **Тесты:**
  - `csv_export_has_expected_header_and_row_count`
  - `csv_export_escapes_commas_and_quotes_in_notes` (корректное
    CSV-экранирование — конкретный, легко забываемый edge-кейс).
  - `journal_entry_insert_and_query_by_symbol` (storage, `MemStore`).

#### `20.3` — SDK для пользовательских стратегий/детекторов
- **Цель:** стабилизировать трейты `Strategy` и `RobotDetector` для внешних
  реализаций (первая итерация — обычные Rust-крейты за cargo-фичей, без WASM).
- **Реализация:** явная версионируемая трейт-сигнатура в отдельном
  под-крейте `domain` (или новом тонком крейте `botradar-sdk`, если трейт
  должен быть стабильнее остального `domain` — решить и
  задокументировать в PR-описании).
- **Тесты:**
  - `example_third_party_strategy_implementation_compiles_and_runs` —
    тестовая реализация трейта `Strategy` вне основного модуля стратегий
    (например, в `tests/`) проходит через существующий движок бэктеста без
    модификации движка — доказательство, что трейт действительно
    расширяем извне.

#### `20.4` — Мультиисточники данных
- **Цель:** абстракция над несколькими источниками (Finam + MOEX ISS +
  офлайн CSV-импорт) без переписывания `domain`/`app`.
- **Реализация:** уже существующий трейт `data::MarketData` — добавить
  `CsvFileMarketData` (читает локальный CSV в формате баров) как третью
  реализацию, доказывающую расширяемость без сети.
- **Тесты:**
  - `csv_market_data_parses_well_formed_file_into_bars`
  - `csv_market_data_rejects_malformed_row_with_clear_error` (не паникует,
    возвращает `DataError` с понятным сообщением).
  - `csv_market_data_satisfies_market_data_trait_contract` — тот же
    контрактный тест, что уже используется для `FinamMarketData`/`ReplaySource`
    (переиспользовать существующий shared-test-suite паттерн, если он есть,
    либо явно повторить минимальный набор инвариантов трейта).

#### `20.5` — Тесты/доки фазы
- **Тесты:** обновить `SUMMARY.md`/`README.md` по факту реализации (как
  делалось для прошлых фаз); финальный прогон `cargo test --workspace` +
  `npm run check && npm run test && npm run build` зелёный.

---

## Как пользоваться этим документом

1. Брать задачи **по одной**, в порядке внутри фазы (зависимости учтены).
2. Для каждой задачи: сначала тесты с известным ответом (TDD необязателен
   жёстко, но «образцовый паттерн ловится» и «шум не ловится» — писать
   до/вместе с реализацией, не после).
3. Закрывать PR только когда: `cargo fmt --all --check`,
   `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`
   зелёные (плюс фронтовая триада для UI-задач).
4. Если в процессе реализации обнаруживается, что сигнатура из этого
   документа неудобна — менять смело, документ описывает **цель и тестовые
   инварианты**, не диктует реализацию буква в букву.
