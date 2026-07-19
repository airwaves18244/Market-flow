//! Асинхронный планировщик ингеста (фича `ingest`).
//!
//! Связывает сетевой источник ([`data::MarketData`]) с хранилищем
//! ([`AppState`]): по таймеру обходит вотчлист круговым курсором
//! ([`BatchCursor`]), уважая per-method лимит Finam ([`RateLimiter`]), тянет
//! свежие бары и пишет их вместе со снимком оборота. Источник абстрактный
//! (трейт `MarketData`), поэтому один такт ([`IngestService::tick`]) тестируется
//! детерминированно на фейке, без сети и реальных задержек.
//!
//! Боевой источник — `data::FinamMarketData` (фича `grpc`); планировщик от его
//! конкретики не зависит.

use std::borrow::Borrow;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use data::{DataError, MarketData, Method, RateLimiter};
use domain::TimeFrame;
use storage::ingest::BatchCursor;
use storage::StorageError;

use crate::cancel::CancelFlag;
use crate::state::AppState;

/// Ошибка такта ингеста: сбой источника или хранилища.
#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("источник данных: {0}")]
    Source(#[from] DataError),
    #[error("хранилище: {0}")]
    Storage(#[from] StorageError),
}

/// Настройки планировщика ингеста.
#[derive(Debug, Clone)]
pub struct IngestConfig {
    /// Тайм-фрейм опрашиваемых баров.
    pub timeframe: TimeFrame,
    /// Глубина окна запроса баров (секунды назад от «сейчас»).
    pub lookback_secs: i64,
    /// Сколько символов опрашивать за один такт (под лимит запросов).
    pub batch: usize,
    /// Период между тактами планировщика.
    pub interval: Duration,
}

impl Default for IngestConfig {
    fn default() -> Self {
        Self {
            timeframe: TimeFrame::D1,
            lookback_secs: 24 * 60 * 60,
            batch: 20,
            interval: Duration::from_secs(60),
        }
    }
}

/// Планировщик батч-поллинга рыночных данных в хранилище.
///
/// Параметр `S` — держатель состояния. По умолчанию `Arc<AppState>` (владеющий
/// headless-путь [`crate::live::run`]), но подойдёт и `&AppState` — так GUI-путь
/// (Tauri-setup) наполняет тот же стор, которым владеет окно, не создавая
/// собственного (см. [`crate::live::run_ingest_into`]). Оба варианта
/// удовлетворяют `Borrow<AppState>`.
pub struct IngestService<M: MarketData, S = Arc<AppState>> {
    source: M,
    state: S,
    cursor: BatchCursor,
    limiter: RateLimiter,
    config: IngestConfig,
}

impl<M: MarketData, S: Borrow<AppState>> IngestService<M, S> {
    /// Создать планировщик для заданного вотчлиста.
    pub fn new(source: M, state: S, symbols: Vec<String>, config: IngestConfig) -> Self {
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

    /// Один такт опроса: взять очередную порцию символов, дотянуть их бары и
    /// записать (бар-серия + снимок оборота). Возвращает число записанных баров.
    ///
    /// `now_ts` — «текущее» время (UNIX-секунды); подаётся явно для
    /// детерминированных тестов. Символы, по которым исчерпан лимит метода
    /// `Bars`, пропускаются до следующего такта (а не копят ошибку).
    pub async fn tick(&mut self, now_ts: i64) -> Result<usize, IngestError> {
        let from_ts = now_ts - self.config.lookback_secs;
        let mut written = 0usize;
        for symbol in self.cursor.next_batch() {
            // Локальный лимит метода: если исчерпан — пропускаем символ.
            if self.limiter.try_acquire(Method::Bars).is_err() {
                continue;
            }
            // R-10: сбой одного символа (сеть/хранилище) не должен ронять весь
            // такт — логируем предупреждение и продолжаем батч. Такт возвращает
            // Ok с числом фактически записанных баров.
            let bars = match self
                .source
                .bars(&symbol, self.config.timeframe, from_ts, now_ts)
                .await
            {
                Ok(bars) => bars,
                Err(e) => {
                    tracing::warn!(symbol = %symbol, error = %e, "ингест баров: ошибка источника");
                    continue;
                }
            };
            if let Some(last) = bars.last() {
                let snapshot_ts = last.ts;
                match self.state.borrow().ingest_bars(
                    &symbol,
                    self.config.timeframe,
                    &bars,
                    snapshot_ts,
                ) {
                    Ok(_) => written += bars.len(),
                    Err(e) => {
                        tracing::warn!(symbol = %symbol, error = %e, "ингест баров: ошибка записи")
                    }
                }
            }
        }
        Ok(written)
    }

    /// Бесконечный цикл планировщика: такт каждые `config.interval`.
    ///
    /// Ошибки отдельного такта логируются и не останавливают цикл (сетевые сбои
    /// транзиентны). Завершается по кооперативной отмене (`cancel`) — флаг
    /// проверяется на каждом пробуждении таймера, до и после такта, поэтому
    /// цикл не начинает и не оставляет недописанный такт после отмены.
    pub async fn run(mut self, cancel: CancelFlag) {
        let mut ticker = tokio::time::interval(self.config.interval);
        loop {
            ticker.tick().await;
            if cancel.is_cancelled() {
                tracing::debug!("такт ингеста остановлен: отмена");
                break;
            }
            match self.tick(now_unix()).await {
                Ok(n) => tracing::debug!(bars = n, "такт ингеста завершён"),
                Err(e) => tracing::warn!(error = %e, "такт ингеста завершился ошибкой"),
            }
        }
    }
}

/// Текущее время в UNIX-секундах UTC (для боевого цикла `run`).
fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{Bar, Instrument, Quote, Trade};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use storage::{MemStore, Store};

    /// Фейковый источник: на `bars` отдаёт по 2 бара на символ и считает вызовы;
    /// остальные методы не используются планировщиком.
    struct FakeSource {
        bars_calls: AtomicUsize,
        empty: bool,
    }

    impl FakeSource {
        fn new() -> Self {
            Self {
                bars_calls: AtomicUsize::new(0),
                empty: false,
            }
        }
        fn empty() -> Self {
            Self {
                bars_calls: AtomicUsize::new(0),
                empty: true,
            }
        }
    }

    impl MarketData for FakeSource {
        async fn assets(&self, _mic: &str) -> Result<Vec<Instrument>, DataError> {
            Ok(Vec::new())
        }

        async fn bars(
            &self,
            _symbol: &str,
            _tf: TimeFrame,
            _from_ts: i64,
            to_ts: i64,
        ) -> Result<Vec<Bar>, DataError> {
            self.bars_calls.fetch_add(1, Ordering::SeqCst);
            if self.empty {
                return Ok(Vec::new());
            }
            let mk = |ts: i64, o: f64, c: f64| Bar {
                ts,
                open: o,
                high: o.max(c),
                low: o.min(c),
                close: c,
                volume: 1_000.0,
            };
            Ok(vec![mk(to_ts - 1, 100.0, 101.0), mk(to_ts, 101.0, 103.0)])
        }

        async fn last_quote(&self, _symbol: &str) -> Result<Quote, DataError> {
            Err(DataError::Other("не используется".into()))
        }

        async fn latest_trades(&self, _symbol: &str) -> Result<Vec<Trade>, DataError> {
            Ok(Vec::new())
        }
    }

    fn state_with_migrated_store() -> Arc<AppState> {
        let mut store = MemStore::new();
        store.migrate().unwrap();
        Arc::new(AppState::new(store))
    }

    fn cfg(batch: usize) -> IngestConfig {
        IngestConfig {
            timeframe: TimeFrame::D1,
            lookback_secs: 86_400,
            batch,
            interval: Duration::from_secs(60),
        }
    }

    #[tokio::test]
    async fn tick_fetches_writes_bars_and_snapshot() {
        let state = state_with_migrated_store();
        let mut svc = IngestService::new(
            FakeSource::new(),
            Arc::clone(&state),
            vec!["SBER@MISX".into(), "LKOH@MISX".into()],
            cfg(10),
        );

        let written = svc.tick(1_000_000).await.unwrap();
        assert_eq!(written, 4); // 2 символа × 2 бара

        // Бары и снимок попали в хранилище через AppState.
        let candles = state.bars("SBER@MISX", TimeFrame::D1, 0, i64::MAX).unwrap();
        assert_eq!(candles.len(), 2);
        let snaps = state.turnover_series("SBER@MISX", 0, i64::MAX).unwrap();
        assert_eq!(snaps.len(), 1);
    }

    #[tokio::test]
    async fn tick_round_robins_across_ticks() {
        let state = state_with_migrated_store();
        let mut svc = IngestService::new(
            FakeSource::new(),
            Arc::clone(&state),
            vec!["A@MISX".into(), "B@MISX".into(), "C@MISX".into()],
            cfg(2), // 2 за такт → за 2 такта обойдём всех троих (с переносом)
        );

        svc.tick(1_000_000).await.unwrap(); // A, B
        svc.tick(1_000_100).await.unwrap(); // C, A
        for sym in ["A@MISX", "B@MISX", "C@MISX"] {
            let n = state.bars(sym, TimeFrame::D1, 0, i64::MAX).unwrap().len();
            assert!(n > 0, "ожидались бары для {sym}");
        }
    }

    #[tokio::test]
    async fn empty_series_writes_nothing() {
        let state = state_with_migrated_store();
        let mut svc = IngestService::new(
            FakeSource::empty(),
            Arc::clone(&state),
            vec!["SBER@MISX".into()],
            cfg(10),
        );
        assert_eq!(svc.tick(1_000_000).await.unwrap(), 0);
        assert!(state
            .bars("SBER@MISX", TimeFrame::D1, 0, i64::MAX)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn rate_limit_skips_symbols_when_exhausted() {
        let state = state_with_migrated_store();
        // Лимит 1 запрос/мин на метод → из двух символов опросится только один.
        let mut svc = IngestService::new(
            FakeSource::new(),
            Arc::clone(&state),
            vec!["A@MISX".into(), "B@MISX".into()],
            cfg(10),
        )
        .with_limiter(RateLimiter::per_minute(1));

        let written = svc.tick(1_000_000).await.unwrap();
        assert_eq!(written, 2); // только один символ × 2 бара
        assert_eq!(svc.source.bars_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn one_symbol_error_does_not_abort_tick() {
        // R-10: источник падает на первом символе, но второй всё равно
        // ингестится — ошибка одного тикера не рвёт весь такт.
        struct FlakyBySymbol;
        impl MarketData for FlakyBySymbol {
            async fn assets(&self, _mic: &str) -> Result<Vec<Instrument>, DataError> {
                Ok(Vec::new())
            }
            async fn bars(
                &self,
                symbol: &str,
                _tf: TimeFrame,
                _from_ts: i64,
                to_ts: i64,
            ) -> Result<Vec<Bar>, DataError> {
                if symbol == "BAD@MISX" {
                    return Err(DataError::Transport("сбой сети".into()));
                }
                Ok(vec![Bar {
                    ts: to_ts,
                    open: 1.0,
                    high: 1.0,
                    low: 1.0,
                    close: 1.0,
                    volume: 1.0,
                }])
            }
            async fn last_quote(&self, _symbol: &str) -> Result<Quote, DataError> {
                Err(DataError::Other("не используется".into()))
            }
            async fn latest_trades(&self, _symbol: &str) -> Result<Vec<Trade>, DataError> {
                Ok(Vec::new())
            }
        }

        let state = state_with_migrated_store();
        let mut svc = IngestService::new(
            FlakyBySymbol,
            Arc::clone(&state),
            vec!["BAD@MISX".into(), "GOOD@MISX".into()],
            cfg(10),
        );

        let written = svc.tick(1_000_000).await.unwrap();
        assert_eq!(written, 1); // упал только BAD, GOOD записан
        assert!(state
            .bars("BAD@MISX", TimeFrame::D1, 0, i64::MAX)
            .unwrap()
            .is_empty());
        assert_eq!(
            state
                .bars("GOOD@MISX", TimeFrame::D1, 0, i64::MAX)
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn run_stops_promptly_once_cancelled() {
        // Флаг отменён ещё до старта цикла: `run` должен выйти на первом же
        // пробуждении таймера, не сделав ни одного такта (см. `bars_calls`).
        let state = state_with_migrated_store();
        let mut fast_cfg = cfg(10);
        fast_cfg.interval = Duration::from_millis(5);
        let svc = IngestService::new(
            FakeSource::new(),
            Arc::clone(&state),
            vec!["SBER@MISX".into()],
            fast_cfg,
        );

        let cancel = crate::cancel::CancelFlag::new();
        cancel.cancel();

        let outcome = tokio::time::timeout(Duration::from_secs(2), svc.run(cancel)).await;
        assert!(
            outcome.is_ok(),
            "run() должен завершиться сам по отмене, не по таймауту теста"
        );
    }
}
