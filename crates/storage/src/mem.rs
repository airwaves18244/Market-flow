//! Реализация [`Store`] в памяти.
//!
//! Не требует нативных библиотек, поэтому собирается и тестируется на любой
//! платформе (включая CI на Linux) — это эталон поведения хранилища и удобный
//! бэкенд для юнит-тестов ингеста/бэкфилла. Семантика записи совпадает с
//! DuckDB-версией: ключи перезаписываются (upsert), запросы по диапазону
//! возвращают данные по возрастанию `ts`.

use std::collections::BTreeMap;
use std::collections::HashMap;

use domain::{Bar, Instrument, TimeFrame, Trade};

use crate::schema::SCHEMA_VERSION;
use crate::store::{SectorEntry, Store, TurnoverSnapshot};
use crate::StorageError;

/// Хранилище в оперативной памяти.
#[derive(Debug, Default)]
pub struct MemStore {
    version: Option<i32>,
    instruments: HashMap<String, Instrument>,
    /// (symbol, timeframe) → (ts → bar). `BTreeMap` держит порядок по `ts`.
    bars: HashMap<(String, &'static str), BTreeMap<i64, Bar>>,
    /// symbol → (ts → snapshot).
    snapshots: HashMap<String, BTreeMap<i64, TurnoverSnapshot>>,
    /// symbol → (ts → сделки на этой секунде, в порядке поступления). Тики
    /// append-only, поэтому внутри секунды держим `Vec`, а не перезапись.
    trades: HashMap<String, BTreeMap<i64, Vec<Trade>>>,
    sector_map: HashMap<String, SectorEntry>,
}

impl MemStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Store for MemStore {
    fn migrate(&mut self) -> Result<(), StorageError> {
        self.version = Some(SCHEMA_VERSION);
        Ok(())
    }

    fn schema_version(&self) -> Result<Option<i32>, StorageError> {
        Ok(self.version)
    }

    fn upsert_instruments(&mut self, items: &[Instrument]) -> Result<usize, StorageError> {
        for it in items {
            self.instruments.insert(it.symbol.clone(), it.clone());
        }
        Ok(items.len())
    }

    fn instruments(&self) -> Result<Vec<Instrument>, StorageError> {
        Ok(self.instruments.values().cloned().collect())
    }

    fn insert_bars(
        &mut self,
        symbol: &str,
        tf: TimeFrame,
        bars: &[Bar],
    ) -> Result<usize, StorageError> {
        let entry = self
            .bars
            .entry((symbol.to_string(), tf.code()))
            .or_default();
        for b in bars {
            entry.insert(b.ts, *b);
        }
        Ok(bars.len())
    }

    fn bars(
        &self,
        symbol: &str,
        tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<Bar>, StorageError> {
        Ok(self
            .bars
            .get(&(symbol.to_string(), tf.code()))
            .into_iter()
            .flat_map(|m| m.range(from_ts..=to_ts).map(|(_, b)| *b))
            .collect())
    }

    fn last_bar_ts(&self, symbol: &str, tf: TimeFrame) -> Result<Option<i64>, StorageError> {
        Ok(self
            .bars
            .get(&(symbol.to_string(), tf.code()))
            .and_then(|m| m.keys().next_back().copied()))
    }

    fn insert_snapshot(
        &mut self,
        symbol: &str,
        snap: &TurnoverSnapshot,
    ) -> Result<(), StorageError> {
        self.snapshots
            .entry(symbol.to_string())
            .or_default()
            .insert(snap.ts, *snap);
        Ok(())
    }

    fn snapshots(
        &self,
        symbol: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<TurnoverSnapshot>, StorageError> {
        Ok(self
            .snapshots
            .get(symbol)
            .into_iter()
            .flat_map(|m| m.range(from_ts..=to_ts).map(|(_, s)| *s))
            .collect())
    }

    fn insert_trades(&mut self, symbol: &str, trades: &[Trade]) -> Result<usize, StorageError> {
        let by_ts = self.trades.entry(symbol.to_string()).or_default();
        for t in trades {
            by_ts.entry(t.ts).or_default().push(*t);
        }
        Ok(trades.len())
    }

    fn trades(&self, symbol: &str, from_ts: i64, to_ts: i64) -> Result<Vec<Trade>, StorageError> {
        Ok(self
            .trades
            .get(symbol)
            .into_iter()
            .flat_map(|m| {
                m.range(from_ts..=to_ts)
                    .flat_map(|(_, v)| v.iter().copied())
            })
            .collect())
    }

    fn upsert_sector_map(&mut self, entries: &[SectorEntry]) -> Result<usize, StorageError> {
        for e in entries {
            self.sector_map.insert(e.key.clone(), e.clone());
        }
        Ok(entries.len())
    }

    fn sector_map(&self) -> Result<Vec<SectorEntry>, StorageError> {
        Ok(self.sector_map.values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::AssetClass;

    fn bar(ts: i64, close: f64) -> Bar {
        Bar {
            ts,
            open: close,
            high: close,
            low: close,
            close,
            volume: 1.0,
        }
    }

    #[test]
    fn migrate_sets_schema_version() {
        let mut s = MemStore::new();
        assert_eq!(s.schema_version().unwrap(), None);
        s.migrate().unwrap();
        assert_eq!(s.schema_version().unwrap(), Some(SCHEMA_VERSION));
    }

    #[test]
    fn bars_upsert_and_range_query_is_ordered() {
        let mut s = MemStore::new();
        // вставляем вразнобой, в т.ч. дубль ts=2 (должен перезаписаться)
        s.insert_bars("SBER@MISX", TimeFrame::D1, &[bar(3, 30.0), bar(1, 10.0)])
            .unwrap();
        s.insert_bars("SBER@MISX", TimeFrame::D1, &[bar(2, 20.0), bar(2, 99.0)])
            .unwrap();

        let got = s.bars("SBER@MISX", TimeFrame::D1, 1, 3).unwrap();
        let closes: Vec<f64> = got.iter().map(|b| b.close).collect();
        assert_eq!(closes, vec![10.0, 99.0, 30.0]); // по возрастанию ts, ts=2 перезаписан
        assert_eq!(s.last_bar_ts("SBER@MISX", TimeFrame::D1).unwrap(), Some(3));
    }

    #[test]
    fn bars_are_isolated_by_symbol_and_timeframe() {
        let mut s = MemStore::new();
        s.insert_bars("SBER@MISX", TimeFrame::D1, &[bar(1, 10.0)])
            .unwrap();
        s.insert_bars("SBER@MISX", TimeFrame::H1, &[bar(1, 11.0)])
            .unwrap();
        assert_eq!(s.bars("SBER@MISX", TimeFrame::D1, 0, 9).unwrap().len(), 1);
        assert_eq!(s.bars("GAZP@MISX", TimeFrame::D1, 0, 9).unwrap().len(), 0);
        assert_eq!(s.last_bar_ts("SBER@MISX", TimeFrame::H1).unwrap(), Some(1));
    }

    #[test]
    fn range_query_excludes_out_of_window() {
        let mut s = MemStore::new();
        s.insert_bars(
            "X",
            TimeFrame::M1,
            &[bar(10, 1.0), bar(20, 2.0), bar(30, 3.0)],
        )
        .unwrap();
        let got = s.bars("X", TimeFrame::M1, 15, 25).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].ts, 20);
    }

    #[test]
    fn instruments_upsert_overwrites() {
        let mut s = MemStore::new();
        let mut inst = Instrument {
            symbol: "SBER@MISX".into(),
            ticker: "SBER".into(),
            name: "Сбербанк".into(),
            asset_class: AssetClass::Equity,
            sector: None,
            lot_size: 10,
            isin: Some("RU0009029540".into()),
        };
        s.upsert_instruments(std::slice::from_ref(&inst)).unwrap();
        inst.sector = Some("Финансы".into());
        s.upsert_instruments(&[inst]).unwrap();

        let all = s.instruments().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].sector.as_deref(), Some("Финансы"));
    }

    #[test]
    fn snapshots_upsert_and_range() {
        let mut s = MemStore::new();
        let snap = TurnoverSnapshot {
            ts: 100,
            turnover: 1000.0,
            net_flow: 50.0,
            change: 0.01,
        };
        s.insert_snapshot("SBER@MISX", &snap).unwrap();
        let got = s.snapshots("SBER@MISX", 0, 200).unwrap();
        assert_eq!(got, vec![snap]);
        assert!(s.snapshots("SBER@MISX", 101, 200).unwrap().is_empty());
    }

    #[test]
    fn trades_append_and_range_ordered() {
        let mut s = MemStore::new();
        let t = |ts: i64, price: f64, size: f64, bi: Option<bool>| Trade {
            ts,
            price,
            size,
            buyer_initiated: bi,
        };
        // вставляем вразнобой, в т.ч. два тика на одной секунде (append, не upsert)
        s.insert_trades(
            "SBER@MISX",
            &[t(3, 30.0, 1.0, Some(true)), t(1, 10.0, 2.0, None)],
        )
        .unwrap();
        s.insert_trades(
            "SBER@MISX",
            &[t(2, 20.0, 3.0, Some(false)), t(2, 21.0, 4.0, Some(true))],
        )
        .unwrap();

        let got = s.trades("SBER@MISX", 1, 3).unwrap();
        // 4 тика по возрастанию ts; внутри ts=2 — порядок поступления (20,21)
        let prices: Vec<f64> = got.iter().map(|t| t.price).collect();
        assert_eq!(prices, vec![10.0, 20.0, 21.0, 30.0]);
        // окно усекает
        assert_eq!(s.trades("SBER@MISX", 2, 2).unwrap().len(), 2);
        assert!(s.trades("GAZP@MISX", 0, 9).unwrap().is_empty());
    }

    #[test]
    fn sector_map_upsert_and_read() {
        let mut s = MemStore::new();
        s.upsert_sector_map(&[
            SectorEntry {
                key: "SBER".into(),
                sector: "Финансы".into(),
                is_isin: false,
            },
            SectorEntry {
                key: "RU0009029540".into(),
                sector: "Финансы".into(),
                is_isin: true,
            },
        ])
        .unwrap();
        assert_eq!(s.sector_map().unwrap().len(), 2);
    }
}
