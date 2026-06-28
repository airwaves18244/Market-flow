//! Replay-источник рыночных данных (Фаза 7, фича `ingest`).
//!
//! Реализует тот же трейт [`data::MarketData`], что и боевой gRPC-клиент, но
//! отдаёт заранее загруженные бары (например, выгруженные ранее в хранилище).
//! Это даёт offline/replay-режим: тот же путь `MarketData → ingest → store →
//! аналитика` гоняется без сети, на исторических данных.

use std::collections::HashMap;

use data::{DataError, MarketData};
use domain::{Bar, Instrument, Quote, TimeFrame, Trade};
use storage::{StorageError, Store};

/// Источник-реплей: бары по символам в памяти.
pub struct ReplaySource {
    instruments: Vec<Instrument>,
    bars: HashMap<String, Vec<Bar>>,
}

impl ReplaySource {
    /// Из готовых данных (инструменты + бары по символу, отсортированы по `ts`).
    pub fn new(instruments: Vec<Instrument>, bars: HashMap<String, Vec<Bar>>) -> Self {
        Self { instruments, bars }
    }

    /// Построить из хранилища: справочник + бары всех инструментов в `tf`.
    pub fn from_store(store: &dyn Store, tf: TimeFrame) -> Result<Self, StorageError> {
        let instruments = store.instruments()?;
        let mut bars = HashMap::new();
        for inst in &instruments {
            let series = store.bars(&inst.symbol, tf, i64::MIN, i64::MAX)?;
            if !series.is_empty() {
                bars.insert(inst.symbol.clone(), series);
            }
        }
        Ok(Self::new(instruments, bars))
    }
}

impl MarketData for ReplaySource {
    async fn assets(&self, mic: &str) -> Result<Vec<Instrument>, DataError> {
        Ok(self
            .instruments
            .iter()
            .filter(|i| mic.is_empty() || i.symbol.ends_with(mic))
            .cloned()
            .collect())
    }

    async fn bars(
        &self,
        symbol: &str,
        _tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<Bar>, DataError> {
        Ok(self
            .bars
            .get(symbol)
            .map(|series| {
                series
                    .iter()
                    .filter(|b| b.ts >= from_ts && b.ts <= to_ts)
                    .copied()
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn last_quote(&self, symbol: &str) -> Result<Quote, DataError> {
        let last = self
            .bars
            .get(symbol)
            .and_then(|s| s.last())
            .ok_or_else(|| DataError::Other(format!("нет данных реплея для {symbol}")))?;
        Ok(Quote {
            ts: last.ts,
            last: last.close,
            bid: last.close,
            ask: last.close,
            volume: last.volume,
        })
    }

    async fn latest_trades(&self, _symbol: &str) -> Result<Vec<Trade>, DataError> {
        // Реплей хранит агрегаты (бары), а не ленту сделок.
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::AssetClass;
    use storage::{ingest::Writer, MemStore};

    fn bar(ts: i64, c: f64) -> Bar {
        Bar {
            ts,
            open: c,
            high: c,
            low: c,
            close: c,
            volume: 100.0,
        }
    }

    fn instrument(symbol: &str) -> Instrument {
        Instrument {
            symbol: symbol.into(),
            ticker: symbol.split('@').next().unwrap_or(symbol).into(),
            name: symbol.into(),
            asset_class: AssetClass::Equity,
            sector: None,
            lot_size: 1,
            isin: None,
        }
    }

    #[tokio::test]
    async fn bars_are_windowed() {
        let mut bars = HashMap::new();
        bars.insert(
            "SBER@MISX".to_string(),
            vec![bar(10, 1.0), bar(20, 2.0), bar(30, 3.0)],
        );
        let src = ReplaySource::new(vec![instrument("SBER@MISX")], bars);

        let got = src.bars("SBER@MISX", TimeFrame::D1, 15, 30).await.unwrap();
        assert_eq!(got.iter().map(|b| b.ts).collect::<Vec<_>>(), [20, 30]);
        // Неизвестный символ → пусто, не ошибка.
        assert!(src
            .bars("X@MISX", TimeFrame::D1, 0, 99)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn last_quote_from_last_bar() {
        let mut bars = HashMap::new();
        bars.insert("SBER@MISX".to_string(), vec![bar(10, 1.0), bar(20, 2.5)]);
        let src = ReplaySource::new(vec![instrument("SBER@MISX")], bars);
        let q = src.last_quote("SBER@MISX").await.unwrap();
        assert_eq!((q.ts, q.last), (20, 2.5));
        assert!(src.last_quote("missing").await.is_err());
    }

    #[tokio::test]
    async fn from_store_roundtrips_through_storage() {
        let mut store = MemStore::new();
        store.migrate().unwrap();
        store
            .upsert_instruments(&[instrument("SBER@MISX")])
            .unwrap();
        {
            let mut w = Writer::new(&mut store);
            w.bars("SBER@MISX", TimeFrame::D1, &[bar(1, 10.0), bar(2, 11.0)])
                .unwrap();
        }
        let src = ReplaySource::from_store(&store, TimeFrame::D1).unwrap();
        assert_eq!(src.assets("MISX").await.unwrap().len(), 1);
        assert_eq!(
            src.bars("SBER@MISX", TimeFrame::D1, 0, 9)
                .await
                .unwrap()
                .len(),
            2
        );
    }
}
