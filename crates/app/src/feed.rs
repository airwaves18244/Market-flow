//! Контракт фида баров для бэктестера (SPEC 11.5.1, 11.5.2).
//!
//! Будущий движок бэктеста (следующие итерации) должен детерминированно
//! итерировать бары по времени, в общем случае — сразу по нескольким
//! тайм-фреймам. Этот модуль даёт для этого чистую, тестируемую без сети/БД
//! основу:
//!
//! - [`HistoryBar`](domain::history::HistoryBar) — единица фида (расширенная
//!   свеча с явным источником/TF, см. `domain::history`);
//! - [`FeedSource`] — маленький трейт «дай бары тикера/TF за диапазон»;
//!   сейчас единственная реализация — [`StoreFeedSource`] поверх текущего
//!   `storage::Store`, а будущее хранилище истории (T9) подключится своей
//!   реализацией трейта без изменения курсора/движка;
//! - [`bar_to_history_bar`] — чистая конверсия `domain::Bar → HistoryBar`;
//! - [`FeedCursor`] — детерминированный курсор-итератор: k-way merge баров
//!   одного или нескольких `(тикер, TF, диапазон)` строго по возрастанию
//!   `ts`, при равных `ts` — по тикеру, затем по TF (короче период — раньше).
//!   Два прогона по одним и тем же данным дают идентичную последовательность.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use domain::history::{DataSource, HistoryBar, TimeRange};
use domain::{Bar, TimeFrame};
use storage::{StorageError, Store};

/// Источник баров для фида бэктестера.
///
/// Абстракция специально маленькая: единственный метод, ничего не знающий
/// про конкретное хранилище. Сейчас за ней стоит [`StoreFeedSource`]
/// (текущий `storage::Store`), в будущем — отдельное хранилище истории
/// (задача T9); движок бэктеста и [`FeedCursor`] от конкретной реализации
/// не зависят.
pub trait FeedSource {
    /// Бары `ticker`/`tf` в диапазоне `range` (полуоткрыт `[from, till)`), по
    /// возрастанию `ts`. Неизвестный тикер или пустой диапазон — пустой
    /// вектор, не ошибка.
    fn bars(
        &self,
        ticker: &str,
        tf: TimeFrame,
        range: TimeRange,
    ) -> Result<Vec<HistoryBar>, StorageError>;
}

/// Реализация [`FeedSource`] поверх текущего `storage::Store`.
///
/// `Store` пока не различает источники данных внутри одной серии
/// тикер/TF (это придёт вместе с хранилищем истории, T9), поэтому источник
/// для получившихся [`HistoryBar`] фиксируется явно при создании.
pub struct StoreFeedSource<'a> {
    store: &'a dyn Store,
    source: DataSource,
}

impl<'a> StoreFeedSource<'a> {
    /// Обернуть стор: `source` проставляется во все выдаваемые `HistoryBar`.
    pub fn new(store: &'a dyn Store, source: DataSource) -> Self {
        Self { store, source }
    }
}

impl FeedSource for StoreFeedSource<'_> {
    fn bars(
        &self,
        ticker: &str,
        tf: TimeFrame,
        range: TimeRange,
    ) -> Result<Vec<HistoryBar>, StorageError> {
        if range.is_empty() {
            return Ok(Vec::new());
        }
        // TimeRange полуоткрыт [from, till), Store::bars — включительно
        // [from_ts, to_ts]: конец сдвигаем на секунду.
        let raw = self.store.bars(ticker, tf, range.from, range.till - 1)?;
        Ok(raw
            .iter()
            .map(|b| bar_to_history_bar(b, self.source, ticker, tf))
            .collect())
    }
}

/// Конверсия обычного бара стора в расширенную историческую свечу.
///
/// Чистая функция без сети/БД: ALGOPACK-поля (VWAP/дисбаланс/OI/HI2)
/// остаются `None` — их даёт только источник MOEX ALGOPACK, а не текущий
/// `Store::bars`.
pub fn bar_to_history_bar(
    bar: &Bar,
    source: DataSource,
    ticker: &str,
    tf: TimeFrame,
) -> HistoryBar {
    HistoryBar::ohlcv(
        source, ticker, tf, bar.ts, bar.open, bar.high, bar.low, bar.close, bar.volume,
    )
}

/// Запрос одной ленты фида: тикер, тайм-фрейм и диапазон.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedRequest {
    pub ticker: String,
    pub tf: TimeFrame,
    pub range: TimeRange,
}

impl FeedRequest {
    pub fn new(ticker: impl Into<String>, tf: TimeFrame, range: TimeRange) -> Self {
        Self {
            ticker: ticker.into(),
            tf,
            range,
        }
    }
}

/// Одна лента фида: уже полученные бары одного `(ticker, tf)` по
/// возрастанию `ts`. Источник исполняет запрос сразу (объём данных
/// бэктеста мал относительно рынка целиком); будущий стор истории сможет
/// отдавать бары лениво, не меняя протокол курсора.
struct Lane {
    bars: std::vec::IntoIter<HistoryBar>,
}

/// Элемент кучи k-way merge: бар + индекс ленты, из которой он взят.
struct HeapItem {
    bar: HistoryBar,
    lane: usize,
}

impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for HeapItem {}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap — max-heap, а курсору нужен минимум по (ts, secid,
        // tf.seconds()) — сравнение инвертировано (self/other переставлены).
        other
            .bar
            .ts
            .cmp(&self.bar.ts)
            .then_with(|| other.bar.secid.cmp(&self.bar.secid))
            .then_with(|| other.bar.tf.seconds().cmp(&self.bar.tf.seconds()))
    }
}

/// Детерминированный курсор-итератор по одной или нескольким лентам
/// `(ticker, tf, range)`.
///
/// Отдаёт бары строго по возрастанию `ts`; при равных `ts` — стабильный
/// вторичный порядок: сначала по тикеру (лексикографически), затем по TF
/// (короче период — раньше). Два прогона по одним и тем же данным дают
/// идентичную последовательность — курсор не зависит ни от чего, кроме
/// содержимого лент.
pub struct FeedCursor {
    lanes: Vec<Lane>,
    heap: BinaryHeap<HeapItem>,
}

impl FeedCursor {
    /// Построить курсор из источника и списка запросов. Пустой список или
    /// запросы без данных дают пустой курсор — не ошибку и не панику.
    pub fn new(source: &dyn FeedSource, requests: &[FeedRequest]) -> Result<Self, StorageError> {
        let mut lanes = Vec::with_capacity(requests.len());
        let mut heap = BinaryHeap::new();
        for req in requests {
            let bars = source.bars(&req.ticker, req.tf, req.range)?;
            let mut it = bars.into_iter();
            let lane_idx = lanes.len();
            if let Some(first) = it.next() {
                heap.push(HeapItem {
                    bar: first,
                    lane: lane_idx,
                });
            }
            lanes.push(Lane { bars: it });
        }
        Ok(FeedCursor { lanes, heap })
    }

    /// Курсор по одной ленте `(ticker, tf, range)` — частный случай
    /// [`FeedCursor::new`] с одним запросом.
    pub fn single(
        source: &dyn FeedSource,
        ticker: impl Into<String>,
        tf: TimeFrame,
        range: TimeRange,
    ) -> Result<Self, StorageError> {
        Self::new(source, &[FeedRequest::new(ticker, tf, range)])
    }
}

impl Iterator for FeedCursor {
    type Item = HistoryBar;

    fn next(&mut self) -> Option<HistoryBar> {
        let HeapItem { bar, lane } = self.heap.pop()?;
        if let Some(next_bar) = self.lanes[lane].bars.next() {
            self.heap.push(HeapItem {
                bar: next_bar,
                lane,
            });
        }
        Some(bar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use storage::{ingest::Writer, MemStore};

    fn bar(ts: i64, c: f64) -> Bar {
        Bar {
            ts,
            open: c,
            high: c,
            low: c,
            close: c,
            volume: 10.0,
        }
    }

    /// Фейковый источник в памяти: (ticker, tf) → бары, отсортированы по `ts`.
    /// Никакого стора/сети — для проверки чистой логики курсора.
    struct FakeSource {
        data: HashMap<(String, TimeFrame), Vec<HistoryBar>>,
    }

    impl FakeSource {
        fn new() -> Self {
            Self {
                data: HashMap::new(),
            }
        }

        fn with(mut self, ticker: &str, tf: TimeFrame, bars: Vec<HistoryBar>) -> Self {
            self.data.insert((ticker.to_string(), tf), bars);
            self
        }
    }

    impl FeedSource for FakeSource {
        fn bars(
            &self,
            ticker: &str,
            tf: TimeFrame,
            range: TimeRange,
        ) -> Result<Vec<HistoryBar>, StorageError> {
            Ok(self
                .data
                .get(&(ticker.to_string(), tf))
                .map(|series| {
                    series
                        .iter()
                        .filter(|b| range.contains(b.ts))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default())
        }
    }

    fn hbar(ticker: &str, tf: TimeFrame, ts: i64, close: f64) -> HistoryBar {
        HistoryBar::ohlcv(
            DataSource::Finam,
            ticker,
            tf,
            ts,
            close,
            close,
            close,
            close,
            1.0,
        )
    }

    // ── Конверсия Bar → HistoryBar ─────────────────────────────────────────

    #[test]
    fn bar_to_history_bar_carries_ohlcv_without_algopack_fields() {
        let b = bar(300, 101.5);
        let h = bar_to_history_bar(&b, DataSource::MoexAlgo, "SBER@MISX", TimeFrame::M5);
        assert_eq!(h.source, DataSource::MoexAlgo);
        assert_eq!(h.secid, "SBER@MISX");
        assert_eq!(h.tf, TimeFrame::M5);
        assert_eq!(h.ts, 300);
        assert_eq!(
            (h.open, h.high, h.low, h.close, h.volume),
            (101.5, 101.5, 101.5, 101.5, 10.0)
        );
        assert!(h.vwap.is_none() && h.disb.is_none() && h.oi.is_none() && h.hi2.is_none());
    }

    // ── StoreFeedSource: пустые диапазоны/отсутствующий тикер ──────────────

    #[test]
    fn store_feed_source_empty_range_and_unknown_ticker_are_empty_not_panic() {
        let mut store = MemStore::new();
        store.migrate().unwrap();
        {
            let mut w = Writer::new(&mut store);
            w.bars("SBER@MISX", TimeFrame::D1, &[bar(1, 10.0), bar(2, 11.0)])
                .unwrap();
        }
        let src = StoreFeedSource::new(&store, DataSource::Finam);

        // Пустой диапазон (till <= from).
        let got = src
            .bars("SBER@MISX", TimeFrame::D1, TimeRange::new(5, 5))
            .unwrap();
        assert!(got.is_empty());

        // Неизвестный тикер.
        let got = src
            .bars("GAZP@MISX", TimeFrame::D1, TimeRange::new(0, 100))
            .unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn store_feed_source_converts_bars_in_range() {
        let mut store = MemStore::new();
        store.migrate().unwrap();
        {
            let mut w = Writer::new(&mut store);
            w.bars(
                "SBER@MISX",
                TimeFrame::D1,
                &[bar(1, 10.0), bar(2, 11.0), bar(3, 12.0)],
            )
            .unwrap();
        }
        let src = StoreFeedSource::new(&store, DataSource::Finam);
        let got = src
            .bars("SBER@MISX", TimeFrame::D1, TimeRange::new(1, 3))
            .unwrap();
        assert_eq!(got.iter().map(|b| b.ts).collect::<Vec<_>>(), [1, 2]);
        assert!(got.iter().all(|b| b.source == DataSource::Finam));
    }

    // ── Детерминизм одной ленты ─────────────────────────────────────────

    #[test]
    fn single_cursor_yields_bars_in_ascending_ts_order_deterministically() {
        let src = FakeSource::new().with(
            "SBER@MISX",
            TimeFrame::M1,
            vec![
                hbar("SBER@MISX", TimeFrame::M1, 10, 1.0),
                hbar("SBER@MISX", TimeFrame::M1, 20, 2.0),
                hbar("SBER@MISX", TimeFrame::M1, 30, 3.0),
            ],
        );

        let run = || {
            FeedCursor::single(&src, "SBER@MISX", TimeFrame::M1, TimeRange::new(0, 100))
                .unwrap()
                .map(|b| b.ts)
                .collect::<Vec<_>>()
        };
        let first = run();
        let second = run();
        assert_eq!(first, vec![10, 20, 30]);
        assert_eq!(
            first, second,
            "два прогона по одним данным должны совпадать"
        );
    }

    #[test]
    fn empty_range_and_missing_ticker_give_empty_feed_without_panic() {
        let src = FakeSource::new().with(
            "SBER@MISX",
            TimeFrame::M1,
            vec![hbar("SBER@MISX", TimeFrame::M1, 10, 1.0)],
        );

        let empty_range =
            FeedCursor::single(&src, "SBER@MISX", TimeFrame::M1, TimeRange::new(5, 5)).unwrap();
        assert_eq!(empty_range.count(), 0);

        let missing_ticker =
            FeedCursor::single(&src, "UNKNOWN@MISX", TimeFrame::M1, TimeRange::new(0, 100))
                .unwrap();
        assert_eq!(missing_ticker.count(), 0);

        // Пустой список запросов вовсе.
        let no_requests = FeedCursor::new(&src, &[]).unwrap();
        assert_eq!(no_requests.count(), 0);
    }

    // ── Мульти-TF слияние ─────────────────────────────────────────────────

    #[test]
    fn multi_tf_merge_orders_by_ts_across_streams() {
        let src = FakeSource::new()
            .with(
                "SBER@MISX",
                TimeFrame::M1,
                vec![
                    hbar("SBER@MISX", TimeFrame::M1, 10, 1.0),
                    hbar("SBER@MISX", TimeFrame::M1, 40, 4.0),
                ],
            )
            .with(
                "SBER@MISX",
                TimeFrame::M5,
                vec![
                    hbar("SBER@MISX", TimeFrame::M5, 20, 2.0),
                    hbar("SBER@MISX", TimeFrame::M5, 30, 3.0),
                ],
            );

        let requests = vec![
            FeedRequest::new("SBER@MISX", TimeFrame::M1, TimeRange::new(0, 100)),
            FeedRequest::new("SBER@MISX", TimeFrame::M5, TimeRange::new(0, 100)),
        ];
        let got: Vec<(i64, TimeFrame)> = FeedCursor::new(&src, &requests)
            .unwrap()
            .map(|b| (b.ts, b.tf))
            .collect();
        assert_eq!(
            got,
            vec![
                (10, TimeFrame::M1),
                (20, TimeFrame::M5),
                (30, TimeFrame::M5),
                (40, TimeFrame::M1),
            ]
        );
    }

    #[test]
    fn multi_ticker_merge_is_deterministic_across_runs() {
        let src = FakeSource::new()
            .with(
                "GAZP@MISX",
                TimeFrame::M1,
                vec![hbar("GAZP@MISX", TimeFrame::M1, 100, 1.0)],
            )
            .with(
                "SBER@MISX",
                TimeFrame::M1,
                vec![hbar("SBER@MISX", TimeFrame::M1, 100, 2.0)],
            )
            .with(
                "AFLT@MISX",
                TimeFrame::M1,
                vec![hbar("AFLT@MISX", TimeFrame::M1, 100, 3.0)],
            );

        let requests = vec![
            FeedRequest::new("SBER@MISX", TimeFrame::M1, TimeRange::new(0, 200)),
            FeedRequest::new("GAZP@MISX", TimeFrame::M1, TimeRange::new(0, 200)),
            FeedRequest::new("AFLT@MISX", TimeFrame::M1, TimeRange::new(0, 200)),
        ];

        let run = || {
            FeedCursor::new(&src, &requests)
                .unwrap()
                .map(|b| b.secid.clone())
                .collect::<Vec<_>>()
        };
        let first = run();
        let second = run();
        // Все три бара на одном ts=100: тай-брейк по тикеру (лексикографически).
        assert_eq!(first, vec!["AFLT@MISX", "GAZP@MISX", "SBER@MISX"]);
        assert_eq!(first, second);
    }

    #[test]
    fn equal_timestamp_tie_break_prefers_shorter_timeframe() {
        let src = FakeSource::new()
            .with(
                "SBER@MISX",
                TimeFrame::D1,
                vec![hbar("SBER@MISX", TimeFrame::D1, 100, 9.0)],
            )
            .with(
                "SBER@MISX",
                TimeFrame::M1,
                vec![hbar("SBER@MISX", TimeFrame::M1, 100, 1.0)],
            )
            .with(
                "SBER@MISX",
                TimeFrame::H1,
                vec![hbar("SBER@MISX", TimeFrame::H1, 100, 5.0)],
            );

        let requests = vec![
            FeedRequest::new("SBER@MISX", TimeFrame::D1, TimeRange::new(0, 200)),
            FeedRequest::new("SBER@MISX", TimeFrame::M1, TimeRange::new(0, 200)),
            FeedRequest::new("SBER@MISX", TimeFrame::H1, TimeRange::new(0, 200)),
        ];
        let got: Vec<TimeFrame> = FeedCursor::new(&src, &requests)
            .unwrap()
            .map(|b| b.tf)
            .collect();
        assert_eq!(got, vec![TimeFrame::M1, TimeFrame::H1, TimeFrame::D1]);
    }
}
