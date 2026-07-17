//! Реализация [`Store`] в памяти.
//!
//! Не требует нативных библиотек, поэтому собирается и тестируется на любой
//! платформе (включая CI на Linux) — это эталон поведения хранилища и удобный
//! бэкенд для юнит-тестов ингеста/бэкфилла. Семантика записи совпадает с
//! DuckDB-версией: ключи перезаписываются (upsert), запросы по диапазону
//! возвращают данные по возрастанию `ts`.

use std::collections::BTreeMap;
use std::collections::HashMap;

use domain::algo::{FutoiPoint, Hi2Point, SuperCandle};
use domain::history::{DataSource, DatasetMeta, HistoryBar};
use domain::{Bar, Instrument, TimeFrame, Trade};

use crate::schema::SCHEMA_VERSION;
use crate::store::{AlgoObstatsRecord, AlgoOrderstatsRecord, SectorEntry, Store, TurnoverSnapshot};
use crate::StorageError;

/// Ключ таблиц ALGOPACK в памяти: (рынок, SECID).
type AlgoKey = (String, String);
/// Ключ истории/каталога в памяти: (код источника, SECID, код тайм-фрейма).
type HistoryKey = (&'static str, String, &'static str);
/// Точки FUTOI на момент `ts`, по группе клиентов (код `fiz`/`yur` → точка).
type FutoiByGroup = HashMap<&'static str, FutoiPoint>;

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
    /// (market, secid) → (ts → свеча).
    algo_tradestats: HashMap<AlgoKey, BTreeMap<i64, SuperCandle>>,
    /// (market, secid) → (ts → (код группы клиентов → точка)).
    algo_futoi: HashMap<AlgoKey, BTreeMap<i64, FutoiByGroup>>,
    /// (market, secid) → (ts → точка).
    algo_hi2: HashMap<AlgoKey, BTreeMap<i64, Hi2Point>>,
    /// (market, secid) → (ts → запись).
    algo_obstats: HashMap<AlgoKey, BTreeMap<i64, AlgoObstatsRecord>>,
    /// (market, secid) → (ts → запись).
    algo_orderstats: HashMap<AlgoKey, BTreeMap<i64, AlgoOrderstatsRecord>>,
    /// (источник, secid, tf) → (ts → историческая свеча).
    history_bars: HashMap<HistoryKey, BTreeMap<i64, HistoryBar>>,
    /// (источник, secid, tf) → метаданные датасета в каталоге.
    history_datasets: HashMap<HistoryKey, DatasetMeta>,
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

    fn insert_algo_tradestats(
        &mut self,
        market: &str,
        candles: &[SuperCandle],
    ) -> Result<usize, StorageError> {
        for c in candles {
            self.algo_tradestats
                .entry((market.to_string(), c.secid.clone()))
                .or_default()
                .insert(c.ts, c.clone());
        }
        Ok(candles.len())
    }

    fn algo_tradestats(
        &self,
        market: &str,
        secid: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<SuperCandle>, StorageError> {
        Ok(self
            .algo_tradestats
            .get(&(market.to_string(), secid.to_string()))
            .into_iter()
            .flat_map(|m| m.range(from_ts..=to_ts).map(|(_, c)| c.clone()))
            .collect())
    }

    fn insert_algo_futoi(
        &mut self,
        market: &str,
        points: &[FutoiPoint],
    ) -> Result<usize, StorageError> {
        for p in points {
            self.algo_futoi
                .entry((market.to_string(), p.secid.clone()))
                .or_default()
                .entry(p.ts)
                .or_default()
                .insert(p.clgroup.code(), p.clone());
        }
        Ok(points.len())
    }

    fn algo_futoi(
        &self,
        market: &str,
        secid: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<FutoiPoint>, StorageError> {
        Ok(self
            .algo_futoi
            .get(&(market.to_string(), secid.to_string()))
            .into_iter()
            .flat_map(|m| {
                m.range(from_ts..=to_ts)
                    .flat_map(|(_, groups)| groups.values().cloned())
            })
            .collect())
    }

    fn insert_algo_hi2(
        &mut self,
        market: &str,
        points: &[Hi2Point],
    ) -> Result<usize, StorageError> {
        for p in points {
            self.algo_hi2
                .entry((market.to_string(), p.secid.clone()))
                .or_default()
                .insert(p.ts, p.clone());
        }
        Ok(points.len())
    }

    fn algo_hi2(
        &self,
        market: &str,
        secid: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<Hi2Point>, StorageError> {
        Ok(self
            .algo_hi2
            .get(&(market.to_string(), secid.to_string()))
            .into_iter()
            .flat_map(|m| m.range(from_ts..=to_ts).map(|(_, p)| p.clone()))
            .collect())
    }

    fn algo_hi2_latest(&self, market: &str, secid: &str) -> Result<Option<Hi2Point>, StorageError> {
        Ok(self
            .algo_hi2
            .get(&(market.to_string(), secid.to_string()))
            .and_then(|m| m.values().next_back())
            .cloned())
    }

    fn insert_algo_obstats(
        &mut self,
        records: &[AlgoObstatsRecord],
    ) -> Result<usize, StorageError> {
        for r in records {
            self.algo_obstats
                .entry((r.market.clone(), r.secid.clone()))
                .or_default()
                .insert(r.ts, r.clone());
        }
        Ok(records.len())
    }

    fn algo_obstats(
        &self,
        market: &str,
        secid: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<AlgoObstatsRecord>, StorageError> {
        Ok(self
            .algo_obstats
            .get(&(market.to_string(), secid.to_string()))
            .into_iter()
            .flat_map(|m| m.range(from_ts..=to_ts).map(|(_, r)| r.clone()))
            .collect())
    }

    fn insert_algo_orderstats(
        &mut self,
        records: &[AlgoOrderstatsRecord],
    ) -> Result<usize, StorageError> {
        for r in records {
            self.algo_orderstats
                .entry((r.market.clone(), r.secid.clone()))
                .or_default()
                .insert(r.ts, r.clone());
        }
        Ok(records.len())
    }

    fn algo_orderstats(
        &self,
        market: &str,
        secid: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<AlgoOrderstatsRecord>, StorageError> {
        Ok(self
            .algo_orderstats
            .get(&(market.to_string(), secid.to_string()))
            .into_iter()
            .flat_map(|m| m.range(from_ts..=to_ts).map(|(_, r)| r.clone()))
            .collect())
    }

    fn insert_history_bars(&mut self, bars: &[HistoryBar]) -> Result<usize, StorageError> {
        for b in bars {
            self.history_bars
                .entry((b.source.code(), b.secid.clone(), b.tf.code()))
                .or_default()
                .insert(b.ts, b.clone());
        }
        Ok(bars.len())
    }

    fn history_bars(
        &self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<HistoryBar>, StorageError> {
        Ok(self
            .history_bars
            .get(&(source.code(), secid.to_string(), tf.code()))
            .into_iter()
            .flat_map(|m| m.range(from_ts..=to_ts).map(|(_, b)| b.clone()))
            .collect())
    }

    fn last_history_bar_ts(
        &self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
    ) -> Result<Option<i64>, StorageError> {
        Ok(self
            .history_bars
            .get(&(source.code(), secid.to_string(), tf.code()))
            .and_then(|m| m.keys().next_back().copied()))
    }

    fn delete_history_bars(
        &mut self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
    ) -> Result<usize, StorageError> {
        Ok(self
            .history_bars
            .remove(&(source.code(), secid.to_string(), tf.code()))
            .map(|m| m.len())
            .unwrap_or(0))
    }

    fn count_history_bars(
        &self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<u64, StorageError> {
        Ok(self
            .history_bars
            .get(&(source.code(), secid.to_string(), tf.code()))
            .map(|m| m.range(from_ts..=to_ts).count() as u64)
            .unwrap_or(0))
    }

    fn upsert_dataset(&mut self, meta: &DatasetMeta) -> Result<(), StorageError> {
        self.history_datasets.insert(
            (meta.source.code(), meta.secid.clone(), meta.tf.code()),
            meta.clone(),
        );
        Ok(())
    }

    fn datasets(&self) -> Result<Vec<DatasetMeta>, StorageError> {
        Ok(self.history_datasets.values().cloned().collect())
    }

    fn dataset(
        &self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
    ) -> Result<Option<DatasetMeta>, StorageError> {
        Ok(self
            .history_datasets
            .get(&(source.code(), secid.to_string(), tf.code()))
            .cloned())
    }

    fn remove_dataset(
        &mut self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
    ) -> Result<bool, StorageError> {
        Ok(self
            .history_datasets
            .remove(&(source.code(), secid.to_string(), tf.code()))
            .is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::algo::ClientGroup;
    use domain::history::TimeRange;
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

    fn candle(ts: i64, secid: &str, close: f64) -> SuperCandle {
        SuperCandle {
            secid: secid.into(),
            ts,
            pr_open: close,
            pr_high: close,
            pr_low: close,
            pr_close: close,
            pr_std: 0.1,
            vol: 100.0,
            val: close * 100.0,
            trades: 10.0,
            pr_vwap: close,
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
    fn algo_tradestats_upsert_and_range_ordered() {
        let mut s = MemStore::new();
        s.insert_algo_tradestats("fo", &[candle(3, "RIH5", 30.0), candle(1, "RIH5", 10.0)])
            .unwrap();
        // повторная вставка с тем же (secid, ts, market) перезаписывает, не дублирует
        s.insert_algo_tradestats("fo", &[candle(1, "RIH5", 99.0)])
            .unwrap();

        let got = s.algo_tradestats("fo", "RIH5", 0, 9).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].ts, 1);
        assert!((got[0].pr_close - 99.0).abs() < 1e-12);
        assert_eq!(got[1].ts, 3);
        // изоляция по рынку
        assert!(s.algo_tradestats("stock", "RIH5", 0, 9).unwrap().is_empty());
    }

    #[test]
    fn algo_tradestats_empty_range_is_empty() {
        let s = MemStore::new();
        assert!(s.algo_tradestats("fo", "GAZP", 0, 100).unwrap().is_empty());
    }

    fn futoi(ts: i64, secid: &str, group: ClientGroup, long: f64, short: f64) -> FutoiPoint {
        FutoiPoint {
            ts,
            secid: secid.into(),
            clgroup: group,
            pos: long + short,
            pos_long: long,
            pos_short: short,
            pos_long_num: long / 10.0,
            pos_short_num: short / 10.0,
        }
    }

    #[test]
    fn algo_futoi_upsert_dedup_by_ts_market_clgroup() {
        let mut s = MemStore::new();
        s.insert_algo_futoi(
            "fo",
            &[
                futoi(1, "RIH5", ClientGroup::Fiz, 100.0, 50.0),
                futoi(1, "RIH5", ClientGroup::Yur, 200.0, 20.0),
            ],
        )
        .unwrap();
        // перезапись группы Fiz на той же ts — не дублирует
        s.insert_algo_futoi("fo", &[futoi(1, "RIH5", ClientGroup::Fiz, 999.0, 1.0)])
            .unwrap();

        let got = s.algo_futoi("fo", "RIH5", 0, 9).unwrap();
        assert_eq!(got.len(), 2); // одна строка на группу, без дублей
        let fiz = got.iter().find(|p| p.clgroup == ClientGroup::Fiz).unwrap();
        assert_eq!(fiz.pos_long, 999.0);
    }

    #[test]
    fn algo_hi2_upsert_and_range() {
        let mut s = MemStore::new();
        let p = |ts: i64, c: f64| Hi2Point {
            ts,
            secid: "SBER".into(),
            concentration: c,
        };
        s.insert_algo_hi2("stock", &[p(1, 0.2), p(2, 0.3)]).unwrap();
        s.insert_algo_hi2("stock", &[p(1, 0.9)]).unwrap(); // перезапись ts=1

        let got = s.algo_hi2("stock", "SBER", 0, 9).unwrap();
        assert_eq!(got.len(), 2);
        assert!((got[0].concentration - 0.9).abs() < 1e-12);
        assert!(s.algo_hi2("stock", "GAZP", 0, 9).unwrap().is_empty());
    }

    #[test]
    fn algo_hi2_latest_returns_last_point_without_full_range() {
        let mut s = MemStore::new();
        let p = |ts: i64, c: f64| Hi2Point {
            ts,
            secid: "SBER".into(),
            concentration: c,
        };
        assert_eq!(s.algo_hi2_latest("stock", "SBER").unwrap(), None);

        s.insert_algo_hi2("stock", &[p(1, 0.2), p(3, 0.4), p(2, 0.3)])
            .unwrap();
        let latest = s.algo_hi2_latest("stock", "SBER").unwrap().unwrap();
        assert_eq!(latest.ts, 3);
        assert!((latest.concentration - 0.4).abs() < 1e-12);

        // Изоляция по рынку/тикеру — как и у algo_hi2.
        assert_eq!(s.algo_hi2_latest("stock", "GAZP").unwrap(), None);
        assert_eq!(s.algo_hi2_latest("fo", "SBER").unwrap(), None);
    }

    fn obstats(ts: i64, secid: &str, market: &str, spread: f64) -> AlgoObstatsRecord {
        AlgoObstatsRecord {
            secid: secid.into(),
            ts,
            market: market.into(),
            spread_bbo: spread,
            spread_lv10: spread * 2.0,
            levels_b: 5.0,
            levels_s: 5.0,
            vol_b: 100.0,
            vol_s: 90.0,
            val_b: 1000.0,
            val_s: 900.0,
            imbalance_vol_bbo: 0.05,
            imbalance_val_bbo: 0.04,
        }
    }

    #[test]
    fn algo_obstats_upsert_and_range() {
        let mut s = MemStore::new();
        s.insert_algo_obstats(&[
            obstats(1, "SBER", "stock", 0.001),
            obstats(2, "SBER", "stock", 0.002),
        ])
        .unwrap();
        s.insert_algo_obstats(&[obstats(1, "SBER", "stock", 0.5)])
            .unwrap(); // перезапись ts=1

        let got = s.algo_obstats("stock", "SBER", 0, 9).unwrap();
        assert_eq!(got.len(), 2);
        assert!((got[0].spread_bbo - 0.5).abs() < 1e-12);
        assert!(s.algo_obstats("fo", "SBER", 0, 9).unwrap().is_empty());
    }

    fn orderstats(ts: i64, secid: &str, market: &str, put_b: f64) -> AlgoOrderstatsRecord {
        AlgoOrderstatsRecord {
            secid: secid.into(),
            ts,
            market: market.into(),
            put_orders_b: put_b,
            put_orders_s: 10.0,
            put_val_b: 1000.0,
            put_val_s: 900.0,
            put_vol_b: 100.0,
            put_vol_s: 90.0,
            cancel_orders_b: 3.0,
            cancel_orders_s: 2.0,
            cancel_val_b: 300.0,
            cancel_val_s: 200.0,
            cancel_vol_b: 30.0,
            cancel_vol_s: 20.0,
        }
    }

    #[test]
    fn algo_orderstats_upsert_and_range() {
        let mut s = MemStore::new();
        s.insert_algo_orderstats(&[
            orderstats(1, "SBER", "stock", 5.0),
            orderstats(2, "SBER", "stock", 6.0),
        ])
        .unwrap();
        s.insert_algo_orderstats(&[orderstats(1, "SBER", "stock", 42.0)])
            .unwrap(); // перезапись ts=1

        let got = s.algo_orderstats("stock", "SBER", 0, 9).unwrap();
        assert_eq!(got.len(), 2);
        assert!((got[0].put_orders_b - 42.0).abs() < 1e-12);
        assert!(s.algo_orderstats("fo", "SBER", 0, 9).unwrap().is_empty());
    }

    fn hbar(source: DataSource, secid: &str, tf: TimeFrame, ts: i64, close: f64) -> HistoryBar {
        HistoryBar::ohlcv(source, secid, tf, ts, close, close, close, close, 10.0)
    }

    #[test]
    fn history_bars_upsert_dedup_and_isolated_by_key() {
        let mut s = MemStore::new();
        s.insert_history_bars(&[
            hbar(DataSource::Finam, "SBER", TimeFrame::M5, 300, 10.0),
            hbar(DataSource::Finam, "SBER", TimeFrame::M5, 600, 20.0),
        ])
        .unwrap();
        // повторный ингест того же ключа (source, secid, tf, ts) перезаписывает
        s.insert_history_bars(&[hbar(DataSource::Finam, "SBER", TimeFrame::M5, 300, 99.0)])
            .unwrap();

        let got = s
            .history_bars(DataSource::Finam, "SBER", TimeFrame::M5, 0, 1000)
            .unwrap();
        assert_eq!(got.len(), 2); // без дублей
        assert_eq!(got[0].ts, 300);
        assert!((got[0].close - 99.0).abs() < 1e-12);
        assert_eq!(
            s.last_history_bar_ts(DataSource::Finam, "SBER", TimeFrame::M5)
                .unwrap(),
            Some(600)
        );

        // изоляция по источнику и по тайм-фрейму
        assert!(s
            .history_bars(DataSource::MoexAlgo, "SBER", TimeFrame::M5, 0, 1000)
            .unwrap()
            .is_empty());
        assert!(s
            .history_bars(DataSource::Finam, "SBER", TimeFrame::H1, 0, 1000)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn history_bars_preserve_optional_algo_fields() {
        let mut s = MemStore::new();
        let mut b = hbar(DataSource::MoexAlgo, "GAZP", TimeFrame::M5, 300, 100.0);
        b.vwap = Some(100.5);
        b.disb = Some(0.2);
        s.insert_history_bars(&[b]).unwrap();
        let got = s
            .history_bars(DataSource::MoexAlgo, "GAZP", TimeFrame::M5, 0, 1000)
            .unwrap();
        assert_eq!(got[0].vwap, Some(100.5));
        assert_eq!(got[0].disb, Some(0.2));
        assert_eq!(got[0].oi, None);
    }

    fn meta(source: DataSource, secid: &str, tf: TimeFrame, from: i64, till: i64) -> DatasetMeta {
        DatasetMeta {
            source,
            secid: secid.into(),
            tf,
            range: TimeRange::new(from, till),
            bars: ((till - from) / tf.seconds().max(1)) as u64,
            updated_ts: till,
        }
    }

    #[test]
    fn catalog_upsert_list_get_remove() {
        let mut s = MemStore::new();
        s.upsert_dataset(&meta(DataSource::Finam, "SBER", TimeFrame::M5, 0, 3600))
            .unwrap();
        s.upsert_dataset(&meta(DataSource::Finam, "SBER", TimeFrame::H1, 0, 7200))
            .unwrap();
        assert_eq!(s.datasets().unwrap().len(), 2);

        // перезапись по ключу не плодит строк
        s.upsert_dataset(&meta(DataSource::Finam, "SBER", TimeFrame::M5, 0, 7200))
            .unwrap();
        assert_eq!(s.datasets().unwrap().len(), 2);
        let d = s
            .dataset(DataSource::Finam, "SBER", TimeFrame::M5)
            .unwrap()
            .unwrap();
        assert_eq!(d.range, TimeRange::new(0, 7200));

        let cat = s.catalog().unwrap();
        assert_eq!(cat.datasets.len(), 2);

        assert!(s
            .remove_dataset(DataSource::Finam, "SBER", TimeFrame::M5)
            .unwrap());
        assert!(!s
            .remove_dataset(DataSource::Finam, "SBER", TimeFrame::M5)
            .unwrap());
        assert_eq!(s.datasets().unwrap().len(), 1);
    }

    #[test]
    fn history_missing_ranges_uses_bar_coverage() {
        let mut s = MemStore::new();
        // ничего не покрыто → весь запрос считается недостающим
        assert_eq!(
            s.history_missing_ranges(
                DataSource::Finam,
                "SBER",
                TimeFrame::M5,
                TimeRange::new(0, 1000)
            )
            .unwrap(),
            vec![TimeRange::new(0, 1000)]
        );

        // Бары M5 (шаг 300) на 0 и 300 покрывают [0,300)+[300,600) → [0,600).
        s.insert_history_bars(&[
            hbar(DataSource::Finam, "SBER", TimeFrame::M5, 0, 1.0),
            hbar(DataSource::Finam, "SBER", TimeFrame::M5, 300, 2.0),
        ])
        .unwrap();
        // покрыто [0,600); дозагрузить хвост [600,1000)
        assert_eq!(
            s.history_missing_ranges(
                DataSource::Finam,
                "SBER",
                TimeFrame::M5,
                TimeRange::new(0, 1000)
            )
            .unwrap(),
            vec![TimeRange::new(600, 1000)]
        );
    }

    #[test]
    fn history_missing_ranges_reports_interior_gap() {
        // «Несмежная догрузка»: бары на 0 и 3000 (шаг 300) дают покрытие
        // [0,300)+[3000,3300); план для [0,3300) — ровно внутренняя дыра.
        let mut s = MemStore::new();
        s.insert_history_bars(&[
            hbar(DataSource::Finam, "SBER", TimeFrame::M5, 0, 1.0),
            hbar(DataSource::Finam, "SBER", TimeFrame::M5, 3000, 2.0),
        ])
        .unwrap();
        assert_eq!(
            s.history_missing_ranges(
                DataSource::Finam,
                "SBER",
                TimeFrame::M5,
                TimeRange::new(0, 3300)
            )
            .unwrap(),
            vec![TimeRange::new(300, 3000)]
        );
    }

    #[test]
    fn delete_and_count_history_bars() {
        let mut s = MemStore::new();
        s.insert_history_bars(&[
            hbar(DataSource::Finam, "SBER", TimeFrame::M5, 0, 1.0),
            hbar(DataSource::Finam, "SBER", TimeFrame::M5, 300, 2.0),
            hbar(DataSource::Finam, "SBER", TimeFrame::M5, 600, 3.0),
        ])
        .unwrap();
        assert_eq!(
            s.count_history_bars(DataSource::Finam, "SBER", TimeFrame::M5, 0, 600)
                .unwrap(),
            3
        );
        assert_eq!(
            s.count_history_bars(DataSource::Finam, "SBER", TimeFrame::M5, 0, 300)
                .unwrap(),
            2
        );
        // Удаление чистит бары и не трогает другой ключ.
        s.insert_history_bars(&[hbar(DataSource::Finam, "SBER", TimeFrame::H1, 0, 9.0)])
            .unwrap();
        assert_eq!(
            s.delete_history_bars(DataSource::Finam, "SBER", TimeFrame::M5)
                .unwrap(),
            3
        );
        assert_eq!(
            s.count_history_bars(DataSource::Finam, "SBER", TimeFrame::M5, 0, i64::MAX)
                .unwrap(),
            0
        );
        assert_eq!(
            s.count_history_bars(DataSource::Finam, "SBER", TimeFrame::H1, 0, i64::MAX)
                .unwrap(),
            1
        );
    }
}
