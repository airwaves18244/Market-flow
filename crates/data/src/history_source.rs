//! Абстракция источника исторических данных (фаза 11.1).
//!
//! [`HistorySource`] — единый асинхронный контракт «дай бары за диапазон» поверх
//! разных транспортов: Finam gRPC ([`FinamHistory`], фича `grpc`) и MOEX
//! ALGOPACK ([`MoexHistory`], фича `moex`). Наружу источник отдаёт уже доменную
//! [`HistoryBar`] с явным источником и тайм-фреймом — storage историзации и
//! оркестратор дозагрузки (`app`, фаза 11.3) опираются на этот трейт, а не на
//! конкретный транспорт.
//!
//! Диапазон запроса — **полуоткрытый** `[from_ts, till_ts)` в UNIX-секундах UTC,
//! как [`domain::history::TimeRange`], который питает планировщик дозагрузки
//! (`missing_ranges`). Адаптеры сами переводят его в конвенцию своего
//! транспорта (Finam `bars` — включительный `[from, to]`; ALGOPACK — даты MSK).
//!
//! Сигнатуры повторяют паттерн [`crate::MarketData`]/[`crate::moex::AlgoSource`]:
//! методы возвращают `impl Future` (RPITIT), поэтому трейт используется через
//! дженерики, а не `dyn` — тот же компромисс, что и у остального слоя `data`
//! (без зависимости от `async-trait`).

use domain::history::{DataSource, HistoryBar};
use domain::{Bar, TimeFrame};

use crate::DataError;

/// Источник исторических баров: асинхронная загрузка диапазона и явный код
/// источника (для партиционирования датасетов в хранилище).
pub trait HistorySource {
    /// Загрузить бары `ticker`/`tf` за полуоткрытый диапазон `[from_ts, till_ts)`
    /// (UNIX-секунды UTC). Результат — по возрастанию `ts`; пустой, если данных
    /// нет или диапазон вырожден.
    fn load(
        &self,
        ticker: &str,
        tf: TimeFrame,
        from_ts: i64,
        till_ts: i64,
    ) -> impl std::future::Future<Output = Result<Vec<HistoryBar>, DataError>> + Send;

    /// Какой это источник (`Finam`/`MoexAlgo`).
    fn source(&self) -> DataSource;
}

/// Чистая конверсия обычного бара (`domain::Bar`) в историческую свечу.
///
/// ALGOPACK-поля (VWAP/дисбаланс/OI/HI2) остаются `None` — их несёт только
/// источник MOEX ALGOPACK. Аналог `app::feed::bar_to_history_bar`, продублирован
/// в `data`, чтобы источник истории не зависел от слоя `app`.
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

/// Фейковый источник истории для тестов оркестрации (фаза 11.3, T10) — отдаёт
/// заранее заданный набор баров, отфильтрованный по запрошенному диапазону
/// `[from_ts, till_ts)`. Тикер/тайм-фрейм не учитываются (как игнорирует
/// параметры [`crate::moex::FakeAlgoSource`]), поэтому набор задаётся под
/// конкретный тест.
#[derive(Debug, Clone)]
pub struct FakeHistorySource {
    /// Код источника, который вернёт [`HistorySource::source`].
    pub source: DataSource,
    /// Заранее заданный ответ (успех со списком баров или ошибка транспорта).
    pub bars: Result<Vec<HistoryBar>, DataError>,
}

impl FakeHistorySource {
    /// Успешный источник с заданными барами.
    pub fn new(source: DataSource, bars: Vec<HistoryBar>) -> Self {
        Self {
            source,
            bars: Ok(bars),
        }
    }

    /// Источник, всегда возвращающий ошибку транспорта.
    pub fn failing(source: DataSource, error: DataError) -> Self {
        Self {
            source,
            bars: Err(error),
        }
    }
}

impl HistorySource for FakeHistorySource {
    async fn load(
        &self,
        _ticker: &str,
        _tf: TimeFrame,
        from_ts: i64,
        till_ts: i64,
    ) -> Result<Vec<HistoryBar>, DataError> {
        let all = self.bars.clone()?;
        Ok(all
            .into_iter()
            .filter(|b| b.ts >= from_ts && b.ts < till_ts)
            .collect())
    }

    fn source(&self) -> DataSource {
        self.source
    }
}

// ─────────────────────────── Finam (фича `grpc`) ───────────────────────────

/// Консервативный размер страницы запроса баров Finam (число баров в одном
/// `MarketDataService.Bars`). Диапазон загрузки бьётся на страницы этого
/// размера, чтобы не упереться в лимит числа баров на ответ.
#[cfg(feature = "grpc")]
pub const FINAM_MAX_BARS_PER_REQUEST: usize = 500;

/// Разбить полуоткрытый диапазон `[from_ts, till_ts)` на включительные страницы
/// `(from, to)` не более `max_bars` баров — под лимит числа баров в одном ответе
/// `MarketData::bars`. Страницы идут по возрастанию времени и не пересекаются.
///
/// Чистый дубликат логики [`crate::backfill`-аналога `storage::backfill::chunk_range`]
/// (её оригинал в крейте `storage`): тянуть `storage` в `data` — обратная и
/// лишняя зависимость (слои независимы), а выносить общую функцию в `domain`
/// потребовало бы переехать и тип `FetchRange` из `storage`, тронув его
/// публичный API. Поэтому здесь — маленькая самостоятельная копия под
/// полуоткрытый доменный диапазон, покрытая своим тестом.
#[cfg(feature = "grpc")]
fn chunk_pages(from_ts: i64, till_ts: i64, tf: TimeFrame, max_bars: usize) -> Vec<(i64, i64)> {
    let step = tf.seconds();
    if step <= 0 || max_bars == 0 || till_ts <= from_ts {
        return Vec::new();
    }
    let last = till_ts - 1; // включительный конец для `MarketData::bars`
    let span = step * (max_bars as i64); // длительность одной страницы
    let mut out = Vec::new();
    let mut start = from_ts;
    while start <= last {
        let end = (start + span - step).min(last);
        out.push((start, end));
        start = end + step;
    }
    out
}

/// Источник истории поверх [`MarketData`](crate::MarketData) (Finam gRPC).
///
/// Диапазон `[from_ts, till_ts)` режется на страницы по
/// [`FINAM_MAX_BARS_PER_REQUEST`] баров, каждая страница тянется через
/// `MarketData::bars`, результаты склеиваются. Лимиты/ретраи наследуются от
/// самого транспорта (`FinamMarketData` уже держит rate-limit и backoff).
#[cfg(feature = "grpc")]
pub struct FinamHistory<M: crate::MarketData> {
    inner: M,
    max_bars: usize,
}

#[cfg(feature = "grpc")]
impl<M: crate::MarketData> FinamHistory<M> {
    /// Источник со страницей по умолчанию ([`FINAM_MAX_BARS_PER_REQUEST`]).
    pub fn new(inner: M) -> Self {
        Self {
            inner,
            max_bars: FINAM_MAX_BARS_PER_REQUEST,
        }
    }

    /// Источник с явным размером страницы (тесты/тюнинг). `0` зажимается до `1`.
    pub fn with_max_bars(inner: M, max_bars: usize) -> Self {
        Self {
            inner,
            max_bars: max_bars.max(1),
        }
    }
}

#[cfg(feature = "grpc")]
impl<M: crate::MarketData + Sync> HistorySource for FinamHistory<M> {
    async fn load(
        &self,
        ticker: &str,
        tf: TimeFrame,
        from_ts: i64,
        till_ts: i64,
    ) -> Result<Vec<HistoryBar>, DataError> {
        let mut out = Vec::new();
        for (page_from, page_to) in chunk_pages(from_ts, till_ts, tf, self.max_bars) {
            let bars = self.inner.bars(ticker, tf, page_from, page_to).await?;
            out.extend(
                bars.iter()
                    .map(|b| bar_to_history_bar(b, DataSource::Finam, ticker, tf)),
            );
        }
        Ok(out)
    }

    fn source(&self) -> DataSource {
        DataSource::Finam
    }
}

// ─────────────────────────── MOEX ALGOPACK (фича `moex`) ────────────────────

/// Чистая конверсия Super Candle ALGOPACK (`tradestats`) в историческую свечу.
///
/// Переносит OHLCV и доступные ALGOPACK-поля: `vwap` (`pr_vwap`) и дисбаланс
/// потока (`disb`). Открытого интереса/индекса концентрации в `tradestats` нет —
/// `oi`/`hi2` остаются `None`.
#[cfg(feature = "moex")]
pub fn super_candle_to_history_bar(
    candle: &domain::algo::SuperCandle,
    tf: TimeFrame,
) -> HistoryBar {
    let mut bar = HistoryBar::ohlcv(
        DataSource::MoexAlgo,
        &candle.secid,
        tf,
        candle.ts,
        candle.pr_open,
        candle.pr_high,
        candle.pr_low,
        candle.pr_close,
        candle.vol,
    );
    bar.vwap = Some(candle.pr_vwap);
    bar.disb = Some(candle.disb);
    bar
}

/// Чистая конверсия простой свечи ISS (`candles`, задел под историю) в
/// историческую свечу: только OHLCV, без ALGOPACK-полей. `secid` передаётся
/// снаружи (`IssCandle` его не несёт).
#[cfg(feature = "moex")]
pub fn iss_candle_to_history_bar(
    candle: &crate::moex::IssCandle,
    secid: &str,
    tf: TimeFrame,
) -> HistoryBar {
    HistoryBar::ohlcv(
        DataSource::MoexAlgo,
        secid,
        tf,
        candle.ts,
        candle.open,
        candle.high,
        candle.low,
        candle.close,
        candle.volume,
    )
}

/// UNIX-секунды UTC → дата `YYYY-MM-DD` московского времени (UTC+3, без перевода
/// часов) для параметров `from`/`till` ISS. Обратна
/// `crate::moex::parse::moex_datetime_to_unix`; алгоритм Ховарда Хинанта
/// (`civil_from_days`), без внешних зависимостей.
#[cfg(feature = "moex")]
fn unix_to_msk_date(ts: i64) -> String {
    const MSK_OFFSET_SECS: i64 = 3 * 3600;
    let days = (ts + MSK_OFFSET_SECS).div_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Григорианская дата (год, месяц, день) из числа дней от эпохи (1970-01-01).
/// Алгоритм Ховарда Хинанта (`civil_from_days`) — обратный к `days_from_civil`
/// из парсера ISS.
#[cfg(feature = "moex")]
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Источник истории поверх [`AlgoSource`](crate::moex::AlgoSource) (MOEX
/// ALGOPACK). Загружает Super Candles (`tradestats`) заданного рынка и переводит
/// их в [`HistoryBar`] с ALGOPACK-полями.
///
/// Диапазон `[from_ts, till_ts)` переводится в даты MSK для параметров `from`/
/// `till` ISS (гранулярность датасета — сутки), затем ответ точно усекается по
/// `ts` до запрошенного полуоткрытого окна. Метки времени приводятся к UTC ещё
/// в парсере ISS (`row_ts`), поэтому здесь дополнительная нормализация не нужна.
#[cfg(feature = "moex")]
pub struct MoexHistory<S: crate::moex::AlgoSource> {
    inner: S,
    market: crate::moex::Market,
}

#[cfg(feature = "moex")]
impl<S: crate::moex::AlgoSource> MoexHistory<S> {
    /// Источник истории для рынка `market` (обычно [`Market::Eq`]).
    ///
    /// [`Market::Eq`]: crate::moex::Market::Eq
    pub fn new(inner: S, market: crate::moex::Market) -> Self {
        Self { inner, market }
    }
}

#[cfg(feature = "moex")]
impl<S: crate::moex::AlgoSource + Sync> HistorySource for MoexHistory<S> {
    async fn load(
        &self,
        ticker: &str,
        tf: TimeFrame,
        from_ts: i64,
        till_ts: i64,
    ) -> Result<Vec<HistoryBar>, DataError> {
        if till_ts <= from_ts {
            return Ok(Vec::new());
        }
        let from_date = unix_to_msk_date(from_ts);
        // Последняя включённая секунда окна — `till_ts - 1`; её дата и есть
        // верхняя граница `till` датасета (гранулярность — сутки).
        let till_date = unix_to_msk_date(till_ts - 1);
        let range = crate::moex::DateRange::new(from_date, till_date);
        let candles = self
            .inner
            .tradestats(self.market, Some(ticker.to_owned()), range)
            .await?;
        Ok(candles
            .into_iter()
            .filter(|c| c.secid == ticker && c.ts >= from_ts && c.ts < till_ts)
            .map(|c| super_candle_to_history_bar(&c, tf))
            .collect())
    }

    fn source(&self) -> DataSource {
        DataSource::MoexAlgo
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn bar_to_history_bar_carries_ohlcv_without_algopack() {
        let h = bar_to_history_bar(&bar(300, 100.5), DataSource::Finam, "SBER", TimeFrame::M5);
        assert_eq!(h.key(), (DataSource::Finam, "SBER", TimeFrame::M5, 300));
        assert_eq!((h.open, h.close, h.volume), (100.5, 100.5, 100.0));
        assert!(h.vwap.is_none() && h.disb.is_none() && h.oi.is_none() && h.hi2.is_none());
    }
}

// Тесты, требующие async-runtime, — за фичами, которые тянут `tokio`
// (`grpc`/`moex`). Базовый кросс-платформенный прогон (`cargo test --workspace`)
// покрывает чистые конверсии выше.
#[cfg(all(test, feature = "grpc"))]
mod grpc_tests {
    use super::*;
    use crate::{DataError, MarketData};
    use domain::{Instrument, Quote, Trade};
    use std::sync::Mutex;

    const DAY: i64 = 86_400;

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

    #[test]
    fn chunk_pages_splits_half_open_range_without_overlap() {
        // [0, 10*DAY) при шаге сутки, 4 бара на страницу → 4 + 4 + 2 = 10 баров.
        let pages = chunk_pages(0, 10 * DAY, TimeFrame::D1, 4);
        assert_eq!(
            pages,
            vec![(0, 3 * DAY), (4 * DAY, 7 * DAY), (8 * DAY, 10 * DAY - 1)]
        );
    }

    #[test]
    fn chunk_pages_single_page_and_degenerate() {
        assert_eq!(
            chunk_pages(0, 3 * DAY, TimeFrame::D1, 100),
            vec![(0, 3 * DAY - 1)]
        );
        assert!(chunk_pages(10, 10, TimeFrame::D1, 4).is_empty());
        assert!(chunk_pages(0, DAY, TimeFrame::D1, 0).is_empty());
    }

    /// Фейковый `MarketData`: `bars` отдаёт бары из заранее заданного набора,
    /// попавшие в запрошенное включительное окно, и записывает границы каждой
    /// страницы — для проверки чанкинга.
    struct FakeMarketData {
        all: Vec<Bar>,
        calls: Mutex<Vec<(i64, i64)>>,
    }

    impl FakeMarketData {
        fn new(all: Vec<Bar>) -> Self {
            Self {
                all,
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    impl MarketData for FakeMarketData {
        async fn assets(&self, _mic: &str) -> Result<Vec<Instrument>, DataError> {
            unreachable!()
        }

        async fn bars(
            &self,
            _symbol: &str,
            _tf: TimeFrame,
            from_ts: i64,
            to_ts: i64,
        ) -> Result<Vec<Bar>, DataError> {
            self.calls.lock().unwrap().push((from_ts, to_ts));
            Ok(self
                .all
                .iter()
                .filter(|b| b.ts >= from_ts && b.ts <= to_ts)
                .copied()
                .collect())
        }

        async fn last_quote(&self, _symbol: &str) -> Result<Quote, DataError> {
            unreachable!()
        }

        async fn latest_trades(&self, _symbol: &str) -> Result<Vec<Trade>, DataError> {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn finam_history_chunks_range_and_concatenates_full_result() {
        // 10 суточных баров ts = 0..10*DAY.
        let all: Vec<Bar> = (0..10).map(|i| bar(i * DAY, 100.0 + i as f64)).collect();
        let md = FakeMarketData::new(all);
        let src = FinamHistory::with_max_bars(md, 4);
        assert_eq!(src.source(), DataSource::Finam);

        let got = src.load("SBER", TimeFrame::D1, 0, 10 * DAY).await.unwrap();
        // Полная склейка: все 10 баров, по возрастанию ts, каждый — с источником Finam.
        assert_eq!(got.len(), 10);
        assert_eq!(got.first().unwrap().ts, 0);
        assert_eq!(got.last().unwrap().ts, 9 * DAY);
        assert!(got.iter().all(|b| b.source == DataSource::Finam));
        assert!(got.windows(2).all(|w| w[0].ts < w[1].ts));

        // Диапазон побит ровно на 3 страницы (4 + 4 + 2).
        let calls = src.inner.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0], (0, 3 * DAY));
        assert_eq!(calls[2].1, 10 * DAY - 1);
    }

    #[tokio::test]
    async fn fake_history_source_filters_to_requested_window() {
        let bars = vec![
            bar_to_history_bar(&bar(0, 1.0), DataSource::Finam, "SBER", TimeFrame::D1),
            bar_to_history_bar(&bar(DAY, 2.0), DataSource::Finam, "SBER", TimeFrame::D1),
            bar_to_history_bar(&bar(2 * DAY, 3.0), DataSource::Finam, "SBER", TimeFrame::D1),
        ];
        let src = FakeHistorySource::new(DataSource::Finam, bars);
        let got = src.load("SBER", TimeFrame::D1, DAY, 5 * DAY).await.unwrap();
        assert_eq!(
            got.iter().map(|b| b.ts).collect::<Vec<_>>(),
            vec![DAY, 2 * DAY]
        );

        let failing =
            FakeHistorySource::failing(DataSource::Finam, DataError::Auth("нет доступа".into()));
        assert!(failing.load("X", TimeFrame::D1, 0, 9).await.is_err());
    }
}

#[cfg(all(test, feature = "moex"))]
mod moex_tests {
    use super::*;
    use crate::moex::{DateRange, FakeAlgoSource, IssCandle, Market};
    use domain::algo::SuperCandle;

    fn super_candle(secid: &str, ts: i64, close: f64) -> SuperCandle {
        SuperCandle {
            secid: secid.to_owned(),
            ts,
            pr_open: close,
            pr_high: close + 1.0,
            pr_low: close - 1.0,
            pr_close: close,
            pr_std: 0.1,
            vol: 100.0,
            val: close * 100.0,
            trades: 10.0,
            pr_vwap: close + 0.25,
            pr_change: 0.0,
            vol_b: 60.0,
            vol_s: 40.0,
            val_b: close * 60.0,
            val_s: close * 40.0,
            trades_b: 6.0,
            trades_s: 4.0,
            disb: 0.2,
            pr_vwap_b: close,
            pr_vwap_s: close,
        }
    }

    #[test]
    fn super_candle_conversion_carries_algopack_fields() {
        let h = super_candle_to_history_bar(&super_candle("SBER", 300, 100.0), TimeFrame::M5);
        assert_eq!(h.source, DataSource::MoexAlgo);
        assert_eq!(h.open, 100.0);
        assert_eq!(h.vwap, Some(100.25));
        assert_eq!(h.disb, Some(0.2));
        assert!(h.oi.is_none() && h.hi2.is_none());
    }

    #[test]
    fn iss_candle_conversion_is_plain_ohlcv() {
        let c = IssCandle {
            ts: 300,
            open: 10.0,
            high: 11.0,
            low: 9.0,
            close: 10.5,
            volume: 42.0,
        };
        let h = iss_candle_to_history_bar(&c, "GAZP", TimeFrame::M5);
        assert_eq!(h.key(), (DataSource::MoexAlgo, "GAZP", TimeFrame::M5, 300));
        assert_eq!(h.close, 10.5);
        assert!(h.vwap.is_none() && h.disb.is_none());
    }

    #[test]
    fn unix_msk_date_roundtrips_with_parser() {
        // 2024-01-15 12:00:00 MSK == 1_705_309_200 UTC (см. parse.rs).
        let ts = 1_705_309_200;
        assert_eq!(unix_to_msk_date(ts), "2024-01-15");
        // Полдень MSK и та же дата в MSK → согласованность обеих функций.
        let back = crate::moex::parse::moex_datetime_to_unix("2024-01-15", "12:00:00").unwrap();
        assert_eq!(back, ts);
    }

    #[tokio::test]
    async fn moex_history_loads_and_filters_tradestats() {
        // Три свечи; окно запроса должно оставить только среднюю.
        let day = 86_400;
        let base = 1_705_309_200; // 2024-01-15 12:00 MSK
        let fake = FakeAlgoSource {
            tradestats: Ok(vec![
                super_candle("SBER", base, 100.0),
                super_candle("SBER", base + day, 101.0),
                super_candle("GAZP", base + day, 200.0), // другой тикер — отфильтруется
            ]),
            ..FakeAlgoSource::default()
        };
        let src = MoexHistory::new(fake, Market::Eq);
        assert_eq!(src.source(), DataSource::MoexAlgo);

        let got = src
            .load("SBER", TimeFrame::M5, base + day, base + 2 * day)
            .await
            .unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].secid, "SBER");
        assert_eq!(got[0].ts, base + day);
        assert_eq!(got[0].vwap, Some(101.25));

        // Вырожденное окно — пустой результат без обращения к источнику.
        assert!(src
            .load("SBER", TimeFrame::M5, base, base)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn moex_history_propagates_source_error() {
        let fake = FakeAlgoSource {
            tradestats: Err(DataError::Transport("сеть".into())),
            ..FakeAlgoSource::default()
        };
        let src = MoexHistory::new(fake, Market::Eq);
        assert!(src.load("SBER", TimeFrame::M5, 0, 86_400).await.is_err());
        // DateRange конструируется корректно из ts (проверка на компиляцию/вызов).
        let _ = DateRange::new("2024-01-01", "2024-01-02");
    }
}
