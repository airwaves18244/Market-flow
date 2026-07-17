//! Асинхронный планировщик ингеста ALGOPACK (фича `moex`, фаза 10.6.4).
//!
//! Симметричен [`crate::ingest::IngestService`] (тот же приём: круговой
//! курсор [`BatchCursor`] по вотчлисту, per-method лимит [`RateLimiter`],
//! источник абстрактный — [`AlgoSource`] вместо `MarketData`), но опрашивает
//! пять датасетов ALGOPACK вместо баров:
//! - `tradestats` (Super Candles) — по каждому тикеру батча;
//! - `hi2` (индекс концентрации) — сводно по рынку, один запрос на такт (а не
//!   на тикер — датасет уже покрывает весь рынок за один вызов);
//! - `futoi` (открытый интерес физ/юр) — по каждому тикеру батча, только для
//!   рынка `fo` (единственного, где определён этот датасет);
//! - `obstats` (статистика стакана) — по каждому тикеру батча;
//! - `orderstats` (статистика заявок) — по каждому тикеру батча.
//!
//! Один такт ([`AlgoIngestService::tick`]) тестируется детерминированно на
//! [`data::moex::FakeAlgoSource`], без сети. Боевой источник —
//! `data::moex::MoexAlgo<ReqwestTransport>` (см. [`crate::live`]).

use std::sync::Arc;
use std::time::Duration;

use data::moex::{AlgoSource, DateRange, Market};
use data::{DataError, Method, RateLimiter};
use storage::ingest::BatchCursor;
use storage::StorageError;

use crate::cancel::CancelFlag;
use crate::state::AppState;

/// Ошибка такта ингеста ALGOPACK: сбой источника или хранилища.
#[derive(Debug, thiserror::Error)]
pub enum AlgoIngestError {
    #[error("источник ALGOPACK: {0}")]
    Source(#[from] DataError),
    #[error("хранилище: {0}")]
    Storage(#[from] StorageError),
}

/// Настройки планировщика ингеста ALGOPACK.
#[derive(Debug, Clone)]
pub struct AlgoIngestConfig {
    /// Рынок ALGOPACK (`eq|fo|fx`).
    pub market: Market,
    /// Диапазон дат запроса (`DateRange::all()` — «последние доступные»).
    pub range: DateRange,
    /// Сколько тикеров опрашивать за один такт (под лимит запросов на метод).
    pub batch: usize,
    /// Период между тактами планировщика.
    pub interval: Duration,
}

impl Default for AlgoIngestConfig {
    fn default() -> Self {
        Self {
            market: Market::Eq,
            range: DateRange::all(),
            batch: 10,
            interval: Duration::from_secs(60),
        }
    }
}

/// Планировщик батч-поллинга датасетов ALGOPACK в хранилище.
pub struct AlgoIngestService<S: AlgoSource> {
    source: S,
    state: Arc<AppState>,
    cursor: BatchCursor,
    limiter: RateLimiter,
    config: AlgoIngestConfig,
}

impl<S: AlgoSource> AlgoIngestService<S> {
    /// Создать планировщик для заданного вотчлиста.
    pub fn new(
        source: S,
        state: Arc<AppState>,
        symbols: Vec<String>,
        config: AlgoIngestConfig,
    ) -> Self {
        let cursor = BatchCursor::new(symbols, config.batch);
        Self {
            source,
            state,
            cursor,
            limiter: RateLimiter::finam_default(),
            config,
        }
    }

    /// Подменить ограничитель частоты (например, в тестах).
    pub fn with_limiter(mut self, limiter: RateLimiter) -> Self {
        self.limiter = limiter;
        self
    }

    /// Один такт опроса: `hi2` — один раз на такт (сводно по рынку), затем по
    /// каждому тикеру очередной порции — `tradestats`/`obstats`/`orderstats`,
    /// а для рынка `fo` ещё и `futoi`. Возвращает суммарное число записанных
    /// строк по всем датасетам.
    ///
    /// Тикер/датасет, по которому исчерпан лимит метода, пропускается до
    /// следующего такта (а не копит ошибку) — тот же приём, что у
    /// [`crate::ingest::IngestService::tick`].
    pub async fn tick(&mut self) -> Result<usize, AlgoIngestError> {
        let market_code = self.config.market.code();
        let mut written = 0usize;

        if self.limiter.try_acquire(Method::MoexHi2).is_ok() {
            let points = self
                .source
                .hi2(self.config.market, self.config.range.clone())
                .await?;
            if !points.is_empty() {
                written += self.state.ingest_algo_hi2(market_code, &points)?;
            }
        }

        for symbol in self.cursor.next_batch() {
            if self.limiter.try_acquire(Method::MoexTradestats).is_ok() {
                let candles = self
                    .source
                    .tradestats(
                        self.config.market,
                        Some(symbol.clone()),
                        self.config.range.clone(),
                    )
                    .await?;
                if !candles.is_empty() {
                    written += self.state.ingest_algo_tradestats(market_code, &candles)?;
                }
            }

            if self.limiter.try_acquire(Method::MoexObstats).is_ok() {
                let points = self
                    .source
                    .obstats(
                        self.config.market,
                        Some(symbol.clone()),
                        self.config.range.clone(),
                    )
                    .await?;
                if !points.is_empty() {
                    written += self.state.ingest_algo_obstats(market_code, &points)?;
                }
            }

            if self.limiter.try_acquire(Method::MoexOrderstats).is_ok() {
                let points = self
                    .source
                    .orderstats(
                        self.config.market,
                        Some(symbol.clone()),
                        self.config.range.clone(),
                    )
                    .await?;
                if !points.is_empty() {
                    written += self.state.ingest_algo_orderstats(market_code, &points)?;
                }
            }

            if self.config.market == Market::Fo
                && self.limiter.try_acquire(Method::MoexFutoi).is_ok()
            {
                let points = self
                    .source
                    .futoi(Some(symbol.clone()), self.config.range.clone())
                    .await?;
                if !points.is_empty() {
                    written += self.state.ingest_algo_futoi(market_code, &points)?;
                }
            }
        }

        Ok(written)
    }

    /// Бесконечный цикл планировщика: такт каждые `config.interval`.
    ///
    /// Ошибки отдельного такта логируются и не останавливают цикл (сетевые
    /// сбои транзиентны) — как [`crate::ingest::IngestService::run`].
    /// Завершается по кооперативной отмене (`cancel`), проверяемой на каждом
    /// пробуждении таймера — см. [`crate::cancel::CancelFlag`].
    pub async fn run(mut self, cancel: CancelFlag) {
        let mut ticker = tokio::time::interval(self.config.interval);
        loop {
            ticker.tick().await;
            if cancel.is_cancelled() {
                tracing::debug!("такт ингеста ALGOPACK остановлен: отмена");
                break;
            }
            match self.tick().await {
                Ok(n) => tracing::debug!(rows = n, "такт ингеста ALGOPACK завершён"),
                Err(e) => tracing::warn!(error = %e, "такт ингеста ALGOPACK завершился ошибкой"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::moex::FakeAlgoSource;
    use domain::algo::{ClientGroup, FutoiPoint, Hi2Point, SuperCandle};
    use storage::{MemStore, Store};

    fn state_with_migrated_store() -> Arc<AppState> {
        let mut store = MemStore::new();
        store.migrate().unwrap();
        Arc::new(AppState::new(store))
    }

    fn cfg(market: Market, batch: usize) -> AlgoIngestConfig {
        AlgoIngestConfig {
            market,
            range: DateRange::all(),
            batch,
            interval: Duration::from_secs(60),
        }
    }

    fn candle(secid: &str, ts: i64) -> SuperCandle {
        SuperCandle {
            secid: secid.into(),
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

    fn futoi_point(secid: &str, ts: i64) -> FutoiPoint {
        FutoiPoint {
            ts,
            secid: secid.into(),
            clgroup: ClientGroup::Fiz,
            pos: 100.0,
            pos_long: 60.0,
            pos_short: 40.0,
            pos_long_num: 6.0,
            pos_short_num: 4.0,
        }
    }

    #[tokio::test]
    async fn tick_writes_tradestats_and_hi2_on_eq_market() {
        let state = state_with_migrated_store();
        let fake = FakeAlgoSource {
            tradestats: Ok(vec![candle("SBER", 1), candle("SBER", 2)]),
            hi2: Ok(vec![Hi2Point {
                ts: 1,
                secid: "SBER".into(),
                concentration: 0.2,
            }]),
            ..FakeAlgoSource::default()
        };
        let mut svc = AlgoIngestService::new(
            fake,
            Arc::clone(&state),
            vec!["SBER".into()],
            cfg(Market::Eq, 10),
        );

        let written = svc.tick().await.unwrap();
        assert_eq!(written, 3); // 2 свечи + 1 точка hi2

        assert_eq!(state.algo_tradestats("eq", "SBER", 0, 9).unwrap().len(), 2);
        assert_eq!(state.algo_hi2("eq", "SBER", 0, 9).unwrap().len(), 1);
        // eq — не fo, поэтому futoi не запрашивался и не записан.
        assert!(state.algo_futoi("eq", "SBER", 0, 9).unwrap().is_empty());
    }

    #[tokio::test]
    async fn tick_also_writes_futoi_on_fo_market() {
        let state = state_with_migrated_store();
        let fake = FakeAlgoSource {
            futoi: Ok(vec![futoi_point("RIH5", 1)]),
            ..FakeAlgoSource::default()
        };
        let mut svc = AlgoIngestService::new(
            fake,
            Arc::clone(&state),
            vec!["RIH5".into()],
            cfg(Market::Fo, 10),
        );

        let written = svc.tick().await.unwrap();
        assert_eq!(written, 1);
        assert_eq!(state.algo_futoi("fo", "RIH5", 0, 9).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn hi2_is_fetched_once_per_tick_not_per_symbol() {
        let state = state_with_migrated_store();
        let fake = FakeAlgoSource {
            hi2: Ok(vec![Hi2Point {
                ts: 1,
                secid: "SBER".into(),
                concentration: 0.1,
            }]),
            ..FakeAlgoSource::default()
        };
        // Батч из двух тикеров за один такт — hi2 всё равно один вызов.
        let mut svc = AlgoIngestService::new(
            fake,
            Arc::clone(&state),
            vec!["SBER".into(), "GAZP".into()],
            cfg(Market::Eq, 10),
        );
        let written = svc.tick().await.unwrap();
        // insert_algo_hi2 идемпотентна по (secid, ts, market) — повторной
        // записи той же точки при batch>1 не происходит, т.к. вызывается один раз.
        assert_eq!(written, 1);
    }

    #[tokio::test]
    async fn empty_responses_write_nothing() {
        let state = state_with_migrated_store();
        let fake = FakeAlgoSource::default();
        let mut svc = AlgoIngestService::new(
            fake,
            Arc::clone(&state),
            vec!["SBER".into()],
            cfg(Market::Eq, 10),
        );
        assert_eq!(svc.tick().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn tick_round_robins_across_ticks() {
        let state = state_with_migrated_store();
        let fake = FakeAlgoSource {
            tradestats: Ok(vec![candle("X", 1)]),
            ..FakeAlgoSource::default()
        };
        let mut svc = AlgoIngestService::new(
            fake,
            Arc::clone(&state),
            vec!["A".into(), "B".into(), "C".into()],
            cfg(Market::Eq, 2), // 2 за такт → за 2 такта обойдём всех троих
        );
        svc.tick().await.unwrap();
        svc.tick().await.unwrap();
        // Все три тикера должны были быть опрошены (fake отдаёт одну свечу
        // "X" независимо от тикера — важен сам факт вызова по каждому символу
        // батча, проверяем через cursor round-robin поведение BatchCursor).
        assert_eq!(svc.cursor.next_batch(), vec!["B", "C"]);
    }

    #[tokio::test]
    async fn rate_limit_skips_when_exhausted() {
        let state = state_with_migrated_store();
        let fake = FakeAlgoSource {
            tradestats: Ok(vec![candle("SBER", 1)]),
            hi2: Ok(vec![Hi2Point {
                ts: 1,
                secid: "SBER".into(),
                concentration: 0.1,
            }]),
            ..FakeAlgoSource::default()
        };
        // Лимит 0 доступных вызовов для tradestats (лимит 1, уже занят вручную).
        let limiter = RateLimiter::per_minute(1);
        limiter.try_acquire(Method::MoexTradestats).unwrap(); // исчерпать лимит заранее
        let mut svc = AlgoIngestService::new(
            fake,
            Arc::clone(&state),
            vec!["SBER".into()],
            cfg(Market::Eq, 10),
        )
        .with_limiter(limiter);

        let written = svc.tick().await.unwrap();
        // tradestats пропущен (лимит исчерпан), но hi2 всё равно записан.
        assert_eq!(written, 1);
        assert!(state
            .algo_tradestats("eq", "SBER", 0, 9)
            .unwrap()
            .is_empty());
        assert_eq!(state.algo_hi2("eq", "SBER", 0, 9).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn tick_also_writes_obstats_and_orderstats() {
        // Раньше эти датасеты только ингестились (`Store::insert_algo_*`
        // умел их принять), но такт планировщика их не опрашивал вовсе —
        // проверяем, что `tick` теперь дёргает оба источника по каждому
        // тикеру батча и пишет результат.
        use domain::algo::{ObstatsPoint, OrderstatsPoint};

        let state = state_with_migrated_store();
        let fake = FakeAlgoSource {
            obstats: Ok(vec![ObstatsPoint {
                spread_bbo: Some(0.01),
                ..ObstatsPoint::at(1, "SBER")
            }]),
            orderstats: Ok(vec![OrderstatsPoint {
                put_orders_b: Some(5.0),
                ..OrderstatsPoint::at(1, "SBER")
            }]),
            ..FakeAlgoSource::default()
        };
        let mut svc = AlgoIngestService::new(
            fake,
            Arc::clone(&state),
            vec!["SBER".into()],
            cfg(Market::Eq, 10),
        );

        let written = svc.tick().await.unwrap();
        assert_eq!(written, 2); // 1 obstats + 1 orderstats
    }

    #[tokio::test]
    async fn run_stops_promptly_once_cancelled() {
        let state = state_with_migrated_store();
        let mut fast_cfg = cfg(Market::Eq, 10);
        fast_cfg.interval = Duration::from_millis(5);
        let svc = AlgoIngestService::new(
            FakeAlgoSource::default(),
            Arc::clone(&state),
            vec!["SBER".into()],
            fast_cfg,
        );

        let cancel = CancelFlag::new();
        cancel.cancel();

        let outcome = tokio::time::timeout(Duration::from_secs(2), svc.run(cancel)).await;
        assert!(
            outcome.is_ok(),
            "run() должен завершиться сам по отмене, не по таймауту теста"
        );
    }
}
