//! Источники исторических данных для бэктестера (фаза 11.1).
//!
//! Единый контракт [`HistorySource`] (`load(ticker, tf, from, till) ->
//! Vec<HistoryBar>`) и адаптеры поверх уже готовых транспортов:
//! - [`FinamHistory`] — поверх [`crate::MarketData`] (gRPC bars) с чанкингом
//!   диапазона на страницы через [`storage::backfill::chunk_range`] под лимит
//!   числа баров в ответе;
//! - [`MoexHistory`] — поверх [`crate::moex::AlgoSource`] (датасет
//!   `tradestats`, 5-мин расширенные свечи) за фичей `moex`;
//! - [`FakeHistorySource`] — предсказуемый источник для тестов оркестрации в
//!   `app`/`storage` без сети.
//!
//! Все таймстампы нормализуются к UTC (UNIX-секунды): Finam отдаёт бары уже в
//! UTC, а парсер MOEX ISS переводит биржевое MSK-время в UTC (см.
//! [`crate::moex::parse`]). Лимиты/ретраи наследуются от нижележащих
//! транспортов (`RateLimiter`/`Backoff` внутри `MarketData`/`AlgoSource`).

use std::collections::BTreeMap;

use domain::history::{DataSource, HistoryBar};
use domain::TimeFrame;
use storage::backfill::{chunk_range, FetchRange};

use crate::{DataError, MarketData};

/// Контракт источника исторических баров. Возвращает бары ключа
/// (source, secid, tf) в полуоткрытом окне `[from, till)` (бар ровно на `till`
/// не попадает), нормализованные к UTC и упорядоченные по возрастанию `ts`, без
/// дублей по `ts`. Полуоткрытая семантика согласована с
/// [`domain::history::TimeRange`] и планом дыр (`missing_ranges`), поэтому
/// смежные диапазоны стыкуются без перекрытия на границе.
///
/// Как и [`MarketData`]/[`crate::moex::AlgoSource`], метод возвращает
/// `impl Future + Send` (RPITIT), поэтому контракт используется через
/// дженерики, а не `dyn HistorySource` — тот же компромисс, что и у остального
/// транспортного слоя `data` (без зависимости от `async-trait`).
pub trait HistorySource {
    /// Загрузить историю инструмента `ticker` в тайм-фрейме `tf` за окно
    /// `[from, till)` (UNIX-секунды UTC, полуоткрыто: `till` исключается).
    fn load(
        &self,
        ticker: &str,
        tf: TimeFrame,
        from: i64,
        till: i64,
    ) -> impl std::future::Future<Output = Result<Vec<HistoryBar>, DataError>> + Send;
}

/// Адаптер истории поверх Finam Trade API ([`MarketData`]).
///
/// Диапазон запроса режется на страницы не более `max_bars` баров каждая
/// ([`chunk_range`]) — под ограничение числа баров в одном ответе `Bars`;
/// страницы склеиваются и дедуплицируются по `ts`.
#[derive(Debug, Clone)]
pub struct FinamHistory<M: MarketData> {
    market: M,
    max_bars: usize,
}

impl<M: MarketData> FinamHistory<M> {
    /// Предел числа баров на страницу по умолчанию (консервативная оценка под
    /// лимит ответа Finam `Bars`).
    pub const DEFAULT_MAX_BARS: usize = 500;

    /// Адаптер с пределом страницы по умолчанию.
    pub fn new(market: M) -> Self {
        Self {
            market,
            max_bars: Self::DEFAULT_MAX_BARS,
        }
    }

    /// Адаптер с явным пределом числа баров на страницу (зажимается в `>= 1`).
    pub fn with_max_bars(market: M, max_bars: usize) -> Self {
        Self {
            market,
            max_bars: max_bars.max(1),
        }
    }
}

impl<M: MarketData + Sync> HistorySource for FinamHistory<M> {
    async fn load(
        &self,
        ticker: &str,
        tf: TimeFrame,
        from: i64,
        till: i64,
    ) -> Result<Vec<HistoryBar>, DataError> {
        if till < from {
            return Ok(Vec::new());
        }
        let pages = chunk_range(
            FetchRange {
                from_ts: from,
                to_ts: till,
            },
            tf,
            self.max_bars,
        );
        // BTreeMap по `ts` даёт склейку страниц, дедуп и порядок «за один проход».
        let mut merged: BTreeMap<i64, HistoryBar> = BTreeMap::new();
        for page in pages {
            let bars = self
                .market
                .bars(ticker, tf, page.from_ts, page.to_ts)
                .await?;
            for b in bars {
                // Finam отдаёт `ts` уже в UTC; окно полуоткрыто — бар ровно на
                // `till` отбрасываем (MarketData::bars включает границу).
                if b.ts < from || b.ts >= till {
                    continue;
                }
                merged.insert(
                    b.ts,
                    HistoryBar::from_bar(DataSource::Finam, ticker, tf, &b),
                );
            }
        }
        Ok(merged.into_values().collect())
    }
}

/// Адаптер истории поверх MOEX ALGOPACK ([`crate::moex::AlgoSource`]).
///
/// Источник — датасет `tradestats` (расширенные 5-мин свечи): OHLCV берётся из
/// `pr_open/pr_high/pr_low/pr_close`, объём — `vol`, а поля `pr_vwap`/`disb`
/// сохраняются в опциональные поля [`HistoryBar`]. Окно `[from, till]` (UTC)
/// переводится в диапазон дат ISS (`YYYY-MM-DD`, UTC), результат
/// дополнительно усекается по `ts`.
#[cfg(feature = "moex")]
#[derive(Debug, Clone)]
pub struct MoexHistory<S: crate::moex::AlgoSource> {
    source: S,
    market: crate::moex::Market,
}

#[cfg(feature = "moex")]
impl<S: crate::moex::AlgoSource> MoexHistory<S> {
    /// Адаптер над источником ALGOPACK для конкретного рынка (`eq`/`fo`/`fx`).
    pub fn new(source: S, market: crate::moex::Market) -> Self {
        Self { source, market }
    }
}

#[cfg(feature = "moex")]
impl<S: crate::moex::AlgoSource + Sync> HistorySource for MoexHistory<S> {
    async fn load(
        &self,
        ticker: &str,
        tf: TimeFrame,
        from: i64,
        till: i64,
    ) -> Result<Vec<HistoryBar>, DataError> {
        if till < from {
            return Ok(Vec::new());
        }
        let range = crate::moex::DateRange {
            from: Some(unix_to_utc_date(from)),
            till: Some(unix_to_utc_date(till)),
        };
        let candles = self
            .source
            .tradestats(self.market, Some(ticker.to_owned()), range)
            .await?;
        let mut merged: BTreeMap<i64, HistoryBar> = BTreeMap::new();
        for c in candles {
            // `ts` уже в UTC (перевод MSK→UTC выполнил парсер ISS). Окно
            // полуоткрыто: свеча ровно на `till` в результат не попадает.
            if c.ts < from || c.ts >= till {
                continue;
            }
            let mut bar = HistoryBar::ohlcv(
                DataSource::MoexAlgo,
                c.secid,
                tf,
                c.ts,
                c.pr_open,
                c.pr_high,
                c.pr_low,
                c.pr_close,
                c.vol,
            );
            bar.vwap = Some(c.pr_vwap);
            bar.disb = Some(c.disb);
            merged.insert(c.ts, bar);
        }
        Ok(merged.into_values().collect())
    }
}

/// Дата `YYYY-MM-DD` (UTC) из UNIX-секунд — граница диапазона для ISS-запроса.
#[cfg(feature = "moex")]
fn unix_to_utc_date(ts: i64) -> String {
    let days = ts.div_euclid(86_400);
    let (y, m, d) = domain::calendar::civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Фейковый источник истории: отдаёт заранее заданные бары, фильтруя по
/// (ticker, tf) и полуоткрытому окну `[from, till)`. Для тестов оркестрации в
/// `app`/`storage` без сети. Установленная `error` перекрывает выдачу.
#[derive(Debug, Clone, Default)]
pub struct FakeHistorySource {
    /// Пул баров, из которого выбираются подходящие запросу.
    pub bars: Vec<HistoryBar>,
    /// Если задана — любой вызов вернёт эту ошибку (симуляция сбоя транспорта).
    pub error: Option<DataError>,
}

impl FakeHistorySource {
    /// Источник с готовым набором баров и без ошибок.
    pub fn with_bars(bars: Vec<HistoryBar>) -> Self {
        Self { bars, error: None }
    }
}

impl HistorySource for FakeHistorySource {
    async fn load(
        &self,
        ticker: &str,
        tf: TimeFrame,
        from: i64,
        till: i64,
    ) -> Result<Vec<HistoryBar>, DataError> {
        if let Some(err) = &self.error {
            return Err(err.clone());
        }
        let mut out: Vec<HistoryBar> = self
            .bars
            .iter()
            // Полуоткрытое окно `[from, till)`: бар ровно на `till` исключается.
            .filter(|b| b.secid == ticker && b.tf == tf && b.ts >= from && b.ts < till)
            .cloned()
            .collect();
        out.sort_by_key(|b| b.ts);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{Bar, Instrument, Quote, Trade};
    use std::sync::Mutex;

    /// Минимальный синхронный исполнитель фьючерсов без tokio: все фьючерсы
    /// здесь завершаются немедленно (фейки не уходят в реальный I/O), поэтому
    /// достаточно опрашивать до готовности с no-op waker. Позволяет гонять
    /// async-тесты адаптеров в дефолтной сборке `data` (без фичи `http`/tokio).
    fn block_on<F: std::future::Future>(fut: F) -> F::Output {
        use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
        fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        fn noop(_: *const ()) {}
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
        let raw = RawWaker::new(std::ptr::null(), &VTABLE);
        let waker = unsafe { Waker::from_raw(raw) };
        let mut cx = Context::from_waker(&waker);
        let mut fut = std::pin::pin!(fut);
        loop {
            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                return v;
            }
            std::hint::spin_loop();
        }
    }

    fn bar(ts: i64, close: f64) -> Bar {
        Bar {
            ts,
            open: close,
            high: close + 1.0,
            low: close - 1.0,
            close,
            volume: 100.0,
        }
    }

    /// Фейковый [`MarketData`], записывающий запрошенные диапазоны и отдающий
    /// по одному бару на каждую границу окна страницы (для проверки чанкинга и
    /// склейки).
    struct FakeMarket {
        /// Захваченные окна `(from_ts, to_ts)` вызовов `bars`.
        calls: Mutex<Vec<(i64, i64)>>,
        /// Шаг тайм-фрейма, чтобы синтезировать бары внутри окна.
        step: i64,
    }

    impl FakeMarket {
        fn new(step: i64) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                step,
            }
        }
    }

    impl MarketData for FakeMarket {
        async fn assets(&self, _mic: &str) -> Result<Vec<Instrument>, DataError> {
            Ok(Vec::new())
        }

        async fn bars(
            &self,
            _symbol: &str,
            _tf: TimeFrame,
            from_ts: i64,
            to_ts: i64,
        ) -> Result<Vec<Bar>, DataError> {
            self.calls.lock().unwrap().push((from_ts, to_ts));
            // По бару на каждый шаг внутри окна страницы.
            let mut out = Vec::new();
            let mut ts = from_ts;
            while ts <= to_ts {
                out.push(bar(ts, ts as f64));
                ts += self.step;
            }
            Ok(out)
        }

        async fn last_quote(&self, _symbol: &str) -> Result<Quote, DataError> {
            Err(DataError::Other("не поддерживается".into()))
        }

        async fn latest_trades(&self, _symbol: &str) -> Result<Vec<Trade>, DataError> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn finam_history_chunks_range_into_pages() {
        // Окно чанкуется на 3 страницы (предел 4 на страницу). Семантика
        // полуоткрытая: бар ровно на `till = 9*day` в результат не попадает.
        let day = TimeFrame::D1.seconds();
        let market = FakeMarket::new(day);
        let hist = FinamHistory::with_max_bars(market, 4);
        let bars = block_on(hist.load("SBER", TimeFrame::D1, 0, 9 * day)).unwrap();

        // Склейка: по бару на сутки [0, 9*day) → 9 баров (без бара на границе).
        assert_eq!(bars.len(), 9);
        assert!(bars.iter().all(|b| b.source == DataSource::Finam));
        assert_eq!(bars.first().unwrap().ts, 0);
        assert_eq!(bars.last().unwrap().ts, 8 * day);
        // порядок строго по возрастанию
        assert!(bars.windows(2).all(|w| w[0].ts < w[1].ts));

        // Чанкинг диапазона не зависит от полуоткрытого пост-фильтра.
        let calls = hist.market.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0], (0, 3 * day));
        assert_eq!(calls[1], (4 * day, 7 * day));
        assert_eq!(calls[2], (8 * day, 9 * day));
    }

    #[test]
    fn finam_history_dedups_overlapping_and_out_of_window_bars() {
        // Полуоткрытое окно [day, 3*day): бар ровно на `till = 3*day` отсечён.
        let day = TimeFrame::D1.seconds();
        let market = FakeMarket::new(day);
        let hist = FinamHistory::new(market);
        let bars = block_on(hist.load("SBER", TimeFrame::D1, day, 3 * day)).unwrap();
        assert_eq!(
            bars.iter().map(|b| b.ts).collect::<Vec<_>>(),
            vec![day, 2 * day]
        );
    }

    #[test]
    fn finam_history_excludes_bar_exactly_on_till() {
        // Явная проверка полуоткрытости: запрос [0, 2*day) над барами 0,day,2day
        // не должен включать бар на границе 2*day.
        let day = TimeFrame::D1.seconds();
        let market = FakeMarket::new(day);
        let hist = FinamHistory::new(market);
        let bars = block_on(hist.load("SBER", TimeFrame::D1, 0, 2 * day)).unwrap();
        assert_eq!(bars.iter().map(|b| b.ts).collect::<Vec<_>>(), vec![0, day]);
    }

    #[test]
    fn finam_history_empty_for_inverted_window() {
        let market = FakeMarket::new(60);
        let hist = FinamHistory::new(market);
        let bars = block_on(hist.load("SBER", TimeFrame::M1, 100, 50)).unwrap();
        assert!(bars.is_empty());
    }

    #[test]
    fn fake_history_source_filters_by_key_and_window() {
        let src = FakeHistorySource::with_bars(vec![
            HistoryBar::ohlcv(
                DataSource::Finam,
                "SBER",
                TimeFrame::M5,
                300,
                1.0,
                1.0,
                1.0,
                1.0,
                1.0,
            ),
            HistoryBar::ohlcv(
                DataSource::Finam,
                "SBER",
                TimeFrame::M5,
                600,
                2.0,
                2.0,
                2.0,
                2.0,
                1.0,
            ),
            HistoryBar::ohlcv(
                DataSource::Finam,
                "GAZP",
                TimeFrame::M5,
                300,
                3.0,
                3.0,
                3.0,
                3.0,
                1.0,
            ),
            HistoryBar::ohlcv(
                DataSource::Finam,
                "SBER",
                TimeFrame::H1,
                300,
                4.0,
                4.0,
                4.0,
                4.0,
                1.0,
            ),
        ]);
        let out = block_on(src.load("SBER", TimeFrame::M5, 0, 500)).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].ts, 300);
    }

    #[test]
    fn fake_history_source_propagates_error() {
        let src = FakeHistorySource {
            bars: Vec::new(),
            error: Some(DataError::Transport("сбой".into())),
        };
        let err = block_on(src.load("SBER", TimeFrame::M5, 0, 100)).unwrap_err();
        assert!(matches!(err, DataError::Transport(_)));
    }

    #[cfg(feature = "moex")]
    #[test]
    fn civil_date_matches_known_utc_instant() {
        // 2024-01-15T09:00:00Z → дата в UTC 2024-01-15.
        assert_eq!(unix_to_utc_date(1_705_309_200), "2024-01-15");
        // ровно эпоха
        assert_eq!(unix_to_utc_date(0), "1970-01-01");
        // конец 1999 года (проверка високосных правил)
        assert_eq!(unix_to_utc_date(946_684_799), "1999-12-31");
    }

    #[cfg(feature = "moex")]
    #[test]
    fn moex_history_maps_tradestats_preserving_utc_and_fields() {
        use crate::moex::{FakeAlgoSource, Market};
        use domain::algo::SuperCandle;

        let candle = |ts: i64, close: f64| SuperCandle {
            secid: "SBER".into(),
            ts,
            pr_open: close,
            pr_high: close + 1.0,
            pr_low: close - 1.0,
            pr_close: close,
            pr_std: 0.1,
            vol: 100.0,
            val: close * 100.0,
            trades: 10.0,
            pr_vwap: close + 0.5,
            pr_change: 0.0,
            vol_b: 60.0,
            vol_s: 40.0,
            val_b: 0.0,
            val_s: 0.0,
            trades_b: 6.0,
            trades_s: 4.0,
            disb: 0.2,
            pr_vwap_b: close,
            pr_vwap_s: close,
        };
        let fake = FakeAlgoSource {
            // ts=300 в окне, ts=10_000 за верхней границей — должен отсечься.
            tradestats: Ok(vec![candle(300, 10.0), candle(10_000, 20.0)]),
            ..FakeAlgoSource::default()
        };
        let hist = MoexHistory::new(fake, Market::Eq);
        let out = block_on(hist.load("SBER", TimeFrame::M5, 0, 1000)).unwrap();
        assert_eq!(out.len(), 1);
        let b = &out[0];
        assert_eq!(b.source, DataSource::MoexAlgo);
        assert_eq!(b.ts, 300); // UTC сохранён
        assert_eq!(b.vwap, Some(10.5));
        assert_eq!(b.disb, Some(0.2));
    }
}
