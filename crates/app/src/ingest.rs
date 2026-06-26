//! Оркестрация ингеста (§ 1.4, § 1.6).
//!
//! Тянет данные из абстрактного источника [`data::MarketData`] и пишет их в
//! хранилище [`storage::Db`]. Реальный источник — gRPC-клиент Finam (Фаза 0);
//! здесь — только связывающая логика, не зависящая от транспорта.
//!
//! Функции не знают про rate-limit и refresh токена — это забота реализации
//! `MarketData`; оркестратор лишь вызывает методы и складывает результат.

use data::{DataError, MarketData, TimeFrame};
use storage::Db;

/// Ошибка шага ингеста.
#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("источник данных: {0}")]
    Data(#[from] DataError),
    #[error("хранилище: {0}")]
    Storage(#[from] storage::StorageError),
}

/// Итог пакетного прохода по нескольким инструментам. Проход устойчив к сбоям:
/// ошибка по одному символу не прерывает остальные, а попадает в [`failures`].
///
/// [`failures`]: PollSummary::failures
#[derive(Debug, Default, PartialEq)]
pub struct PollSummary {
    /// Сколько баров записано суммарно.
    pub bars_written: usize,
    /// Сколько инструментов обработано без ошибок.
    pub symbols_ok: usize,
    /// Сбои по символам: `(symbol, текст ошибки)`.
    pub failures: Vec<(String, String)>,
}

/// Синхронизировать справочник инструментов биржи в хранилище
/// (`AssetsService.Assets` → upsert). Возвращает число записей.
pub async fn sync_instruments<M: MarketData>(
    src: &M,
    db: &Db,
    mic: &str,
) -> Result<usize, IngestError> {
    let instruments = src.assets(mic).await?;
    Ok(db.upsert_instruments(&instruments)?)
}

/// Бэкфилл исторических баров одного инструмента за `[from_ts, to_ts]` (§ 1.6).
/// Идемпотентен на уровне хранилища (PK `symbol+timeframe+ts`).
pub async fn backfill_bars<M: MarketData>(
    src: &M,
    db: &Db,
    symbol: &str,
    tf: TimeFrame,
    from_ts: i64,
    to_ts: i64,
) -> Result<usize, IngestError> {
    let bars = src.bars(symbol, tf, from_ts, to_ts).await?;
    Ok(db.insert_bars(symbol, map_timeframe(tf), &bars)?)
}

/// Пакетный бэкфилл/поллинг набора инструментов одного тайм-фрейма (§ 1.4).
/// Не прерывается на ошибке отдельного символа — сводка в [`PollSummary`].
pub async fn backfill_symbols<M: MarketData>(
    src: &M,
    db: &Db,
    symbols: &[&str],
    tf: TimeFrame,
    from_ts: i64,
    to_ts: i64,
) -> PollSummary {
    let mut summary = PollSummary::default();
    for &symbol in symbols {
        match backfill_bars(src, db, symbol, tf, from_ts, to_ts).await {
            Ok(n) => {
                summary.bars_written += n;
                summary.symbols_ok += 1;
            }
            Err(e) => summary.failures.push((symbol.to_string(), e.to_string())),
        }
    }
    summary
}

/// Сопоставление тайм-фрейма API ([`data::TimeFrame`]) с кодом хранилища
/// ([`storage::TimeFrame`]). Оба перечисления независимы по слоям, но значения
/// совпадают.
fn map_timeframe(tf: TimeFrame) -> storage::TimeFrame {
    match tf {
        TimeFrame::M1 => storage::TimeFrame::M1,
        TimeFrame::M5 => storage::TimeFrame::M5,
        TimeFrame::M15 => storage::TimeFrame::M15,
        TimeFrame::H1 => storage::TimeFrame::H1,
        TimeFrame::D1 => storage::TimeFrame::D1,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use domain::{AssetClass, Bar, Instrument, Quote, Trade};

    use super::*;

    /// Мок источника рыночных данных для тестов оркестрации.
    #[derive(Default)]
    struct MockSource {
        instruments: Vec<Instrument>,
        bars: HashMap<String, Vec<Bar>>,
        /// Символы, по которым источник имитирует сбой транспорта.
        fail: HashSet<String>,
    }

    impl MarketData for MockSource {
        async fn assets(&self, _mic: &str) -> Result<Vec<Instrument>, DataError> {
            Ok(self.instruments.clone())
        }

        async fn bars(
            &self,
            symbol: &str,
            _tf: TimeFrame,
            _from_ts: i64,
            _to_ts: i64,
        ) -> Result<Vec<Bar>, DataError> {
            if self.fail.contains(symbol) {
                return Err(DataError::Transport("имитация обрыва".into()));
            }
            Ok(self.bars.get(symbol).cloned().unwrap_or_default())
        }

        async fn last_quote(&self, _symbol: &str) -> Result<Quote, DataError> {
            Err(DataError::Other("не используется в тестах".into()))
        }

        async fn latest_trades(&self, _symbol: &str) -> Result<Vec<Trade>, DataError> {
            Ok(Vec::new())
        }
    }

    fn instrument(symbol: &str, ticker: &str) -> Instrument {
        Instrument {
            symbol: symbol.to_string(),
            ticker: ticker.to_string(),
            name: format!("{ticker} name"),
            asset_class: AssetClass::Equity,
            sector: None,
            lot_size: 1,
            isin: None,
        }
    }

    fn bar(ts: i64) -> Bar {
        Bar {
            ts,
            open: 100.0,
            high: 102.0,
            low: 99.0,
            close: 101.0,
            volume: 10.0,
        }
    }

    #[tokio::test]
    async fn sync_instruments_writes_assets() {
        let db = Db::open_in_memory().unwrap();
        let src = MockSource {
            instruments: vec![
                instrument("SBER@MISX", "SBER"),
                instrument("GAZP@MISX", "GAZP"),
            ],
            ..Default::default()
        };
        let n = sync_instruments(&src, &db, "MISX").await.unwrap();
        assert_eq!(n, 2);
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM instruments", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn backfill_bars_persists_and_is_idempotent() {
        let db = Db::open_in_memory().unwrap();
        let mut bars = HashMap::new();
        bars.insert("SBER@MISX".to_string(), vec![bar(1000), bar(2000)]);
        let src = MockSource {
            bars,
            ..Default::default()
        };

        let n = backfill_bars(&src, &db, "SBER@MISX", TimeFrame::D1, 0, 10_000)
            .await
            .unwrap();
        assert_eq!(n, 2);
        // Повторный бэкфилл того же окна не плодит дубликаты.
        backfill_bars(&src, &db, "SBER@MISX", TimeFrame::D1, 0, 10_000)
            .await
            .unwrap();
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM bars", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn backfill_symbols_is_resilient_to_per_symbol_failure() {
        let db = Db::open_in_memory().unwrap();
        let mut bars = HashMap::new();
        bars.insert("SBER@MISX".to_string(), vec![bar(1000)]);
        bars.insert("LKOH@MISX".to_string(), vec![bar(1000), bar(2000)]);
        let src = MockSource {
            bars,
            fail: HashSet::from(["GAZP@MISX".to_string()]),
            ..Default::default()
        };

        let summary = backfill_symbols(
            &src,
            &db,
            &["SBER@MISX", "GAZP@MISX", "LKOH@MISX"],
            TimeFrame::D1,
            0,
            10_000,
        )
        .await;

        assert_eq!(summary.symbols_ok, 2);
        assert_eq!(summary.bars_written, 3); // 1 + 2
        assert_eq!(summary.failures.len(), 1);
        assert_eq!(summary.failures[0].0, "GAZP@MISX");
    }
}
