//! Трейт-контракт источника данных MOEX ALGOPACK ([`AlgoSource`]) и фейковая
//! реализация ([`FakeAlgoSource`]) для тестов оркестрации в других слоях
//! (`app`/`storage`) — без сети, с заранее заданными результатами.
//!
//! Сигнатуры повторяют паттерн [`crate::MarketData`]: методы возвращают
//! `impl Future` (RPITIT) вместо `Box<dyn Future>`, поэтому трейт (как и
//! `MarketData`) используется через дженерики, а не `dyn AlgoSource` — тот же
//! компромисс, что и у остального транспортного слоя `data` (без зависимости
//! от `async-trait`).

use domain::algo::{FutoiPoint, Hi2Point, ObstatsPoint, OrderstatsPoint, SuperCandle};

use crate::http::HttpTransport;
use crate::DataError;

use super::client::{DateRange, Market, MoexAlgo};
use super::options::{MoexIss, OptionsBoardSnapshot};

/// Источник данных ALGOPACK: пять датасетов `10.0.1`. Тикер передаётся
/// владеющей строкой (а не `&str`), чтобы сигнатура была совместима с
/// `impl Future + Send` без спора о временах жизни параметров.
pub trait AlgoSource {
    /// `tradestats` (Super Candles).
    fn tradestats(
        &self,
        market: Market,
        ticker: Option<String>,
        range: DateRange,
    ) -> impl std::future::Future<Output = Result<Vec<SuperCandle>, DataError>> + Send;

    /// `orderstats`.
    fn orderstats(
        &self,
        market: Market,
        ticker: Option<String>,
        range: DateRange,
    ) -> impl std::future::Future<Output = Result<Vec<OrderstatsPoint>, DataError>> + Send;

    /// `obstats`.
    fn obstats(
        &self,
        market: Market,
        ticker: Option<String>,
        range: DateRange,
    ) -> impl std::future::Future<Output = Result<Vec<ObstatsPoint>, DataError>> + Send;

    /// `hi2` (сводно по рынку).
    fn hi2(
        &self,
        market: Market,
        range: DateRange,
    ) -> impl std::future::Future<Output = Result<Vec<Hi2Point>, DataError>> + Send;

    /// `futoi` (только рынок `fo`).
    fn futoi(
        &self,
        ticker: Option<String>,
        range: DateRange,
    ) -> impl std::future::Future<Output = Result<Vec<FutoiPoint>, DataError>> + Send;
}

impl<T: HttpTransport + Send + Sync> AlgoSource for MoexAlgo<T> {
    async fn tradestats(
        &self,
        market: Market,
        ticker: Option<String>,
        range: DateRange,
    ) -> Result<Vec<SuperCandle>, DataError> {
        MoexAlgo::tradestats(self, market, ticker.as_deref(), range).await
    }

    async fn orderstats(
        &self,
        market: Market,
        ticker: Option<String>,
        range: DateRange,
    ) -> Result<Vec<OrderstatsPoint>, DataError> {
        MoexAlgo::orderstats(self, market, ticker.as_deref(), range).await
    }

    async fn obstats(
        &self,
        market: Market,
        ticker: Option<String>,
        range: DateRange,
    ) -> Result<Vec<ObstatsPoint>, DataError> {
        MoexAlgo::obstats(self, market, ticker.as_deref(), range).await
    }

    async fn hi2(&self, market: Market, range: DateRange) -> Result<Vec<Hi2Point>, DataError> {
        MoexAlgo::hi2(self, market, range).await
    }

    async fn futoi(
        &self,
        ticker: Option<String>,
        range: DateRange,
    ) -> Result<Vec<FutoiPoint>, DataError> {
        MoexAlgo::futoi(self, ticker.as_deref(), range).await
    }
}

/// Фейковый источник ALGOPACK: каждый метод отдаёт заранее заданный
/// результат, игнорируя параметры запроса. Для тестов оркестрации в `app`/
/// `storage`, где нужен предсказуемый `AlgoSource` без сети.
#[derive(Debug, Clone)]
pub struct FakeAlgoSource {
    pub tradestats: Result<Vec<SuperCandle>, DataError>,
    pub orderstats: Result<Vec<OrderstatsPoint>, DataError>,
    pub obstats: Result<Vec<ObstatsPoint>, DataError>,
    pub hi2: Result<Vec<Hi2Point>, DataError>,
    pub futoi: Result<Vec<FutoiPoint>, DataError>,
}

impl Default for FakeAlgoSource {
    /// Пустые (но успешные) ответы по всем датасетам.
    fn default() -> Self {
        Self {
            tradestats: Ok(Vec::new()),
            orderstats: Ok(Vec::new()),
            obstats: Ok(Vec::new()),
            hi2: Ok(Vec::new()),
            futoi: Ok(Vec::new()),
        }
    }
}

impl AlgoSource for FakeAlgoSource {
    async fn tradestats(
        &self,
        _market: Market,
        _ticker: Option<String>,
        _range: DateRange,
    ) -> Result<Vec<SuperCandle>, DataError> {
        self.tradestats.clone()
    }

    async fn orderstats(
        &self,
        _market: Market,
        _ticker: Option<String>,
        _range: DateRange,
    ) -> Result<Vec<OrderstatsPoint>, DataError> {
        self.orderstats.clone()
    }

    async fn obstats(
        &self,
        _market: Market,
        _ticker: Option<String>,
        _range: DateRange,
    ) -> Result<Vec<ObstatsPoint>, DataError> {
        self.obstats.clone()
    }

    async fn hi2(&self, _market: Market, _range: DateRange) -> Result<Vec<Hi2Point>, DataError> {
        self.hi2.clone()
    }

    async fn futoi(
        &self,
        _ticker: Option<String>,
        _range: DateRange,
    ) -> Result<Vec<FutoiPoint>, DataError> {
        self.futoi.clone()
    }
}

/// Источник опционной доски (фаза 12.4, `12.4.3`): один метод — снимок доски
/// по коду базового актива. Отдельный трейт от [`AlgoSource`], так как доска
/// читается с другого хоста без авторизации ([`MoexIss`], не [`MoexAlgo`]).
pub trait OptionsSource {
    /// Доска опционов + best-effort форвард базового актива (см.
    /// [`MoexIss::options_board_snapshot`]).
    fn options_board(
        &self,
        underlying: String,
    ) -> impl std::future::Future<Output = Result<OptionsBoardSnapshot, DataError>> + Send;
}

impl<T: HttpTransport + Send + Sync> OptionsSource for MoexIss<T> {
    async fn options_board(&self, underlying: String) -> Result<OptionsBoardSnapshot, DataError> {
        MoexIss::options_board_snapshot(self, &underlying).await
    }
}

/// Фейковый источник опционной доски: заранее заданный результат, игнорируя
/// параметры запроса. Для тестов оркестрации в `app` без сети.
#[derive(Debug, Clone)]
pub struct FakeOptionsSource {
    pub options_board: Result<OptionsBoardSnapshot, DataError>,
}

impl Default for FakeOptionsSource {
    /// Пустая (но успешная) доска без форварда.
    fn default() -> Self {
        Self {
            options_board: Ok(OptionsBoardSnapshot::default()),
        }
    }
}

impl OptionsSource for FakeOptionsSource {
    async fn options_board(&self, _underlying: String) -> Result<OptionsBoardSnapshot, DataError> {
        self.options_board.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candle(secid: &str, ts: i64) -> SuperCandle {
        SuperCandle {
            secid: secid.to_owned(),
            ts,
            pr_open: 1.0,
            pr_high: 1.0,
            pr_low: 1.0,
            pr_close: 1.0,
            pr_std: 0.0,
            vol: 1.0,
            val: 1.0,
            trades: 1.0,
            pr_vwap: 1.0,
            pr_change: 0.0,
            vol_b: 1.0,
            vol_s: 0.0,
            val_b: 1.0,
            val_s: 0.0,
            trades_b: 1.0,
            trades_s: 0.0,
            disb: 1.0,
            pr_vwap_b: 1.0,
            pr_vwap_s: 1.0,
        }
    }

    #[tokio::test]
    async fn fake_source_returns_configured_result_ignoring_params() {
        let fake = FakeAlgoSource {
            tradestats: Ok(vec![candle("SBER", 1)]),
            ..FakeAlgoSource::default()
        };
        let out = fake
            .tradestats(Market::Eq, Some("ANYTHING".into()), DateRange::all())
            .await
            .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].secid, "SBER");
        // hi2 не настроен явно — успешный пустой ответ по умолчанию.
        assert_eq!(
            fake.hi2(Market::Eq, DateRange::all()).await.unwrap(),
            vec![]
        );
    }

    #[tokio::test]
    async fn fake_source_can_simulate_errors() {
        let fake = FakeAlgoSource {
            hi2: Err(DataError::Auth("нет доступа".into())),
            ..FakeAlgoSource::default()
        };
        let err = fake.hi2(Market::Eq, DateRange::all()).await.unwrap_err();
        assert!(matches!(err, DataError::Auth(_)));
    }

    /// Контракт используется через дженерики (как `MarketData`), без `dyn`.
    fn _assert_generic_bound<S: AlgoSource>(_s: &S) {}

    // ── FakeOptionsSource ──────────────────────────────────────────────────

    #[tokio::test]
    async fn fake_options_source_default_is_empty_board_without_forward() {
        let fake = FakeOptionsSource::default();
        let snapshot = fake.options_board("RIH5".into()).await.unwrap();
        assert!(snapshot.quotes.is_empty());
        assert_eq!(snapshot.forward, None);
    }

    #[tokio::test]
    async fn fake_options_source_returns_configured_snapshot_ignoring_underlying() {
        let fake = FakeOptionsSource {
            options_board: Ok(OptionsBoardSnapshot {
                quotes: Vec::new(),
                forward: Some(50_500.0),
            }),
        };
        let snapshot = fake.options_board("ANYTHING".into()).await.unwrap();
        assert_eq!(snapshot.forward, Some(50_500.0));
    }

    #[tokio::test]
    async fn fake_options_source_can_simulate_errors() {
        let fake = FakeOptionsSource {
            options_board: Err(DataError::Transport("недоступен".into())),
        };
        let err = fake.options_board("RIH5".into()).await.unwrap_err();
        assert!(matches!(err, DataError::Transport(_)));
    }

    /// Контракт используется через дженерики, без `dyn` (как [`AlgoSource`]).
    fn _assert_options_generic_bound<S: OptionsSource>(_s: &S) {}
}
