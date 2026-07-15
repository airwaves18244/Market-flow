//! Реализация [`Store`] на нативном **DuckDB** (фича `duckdb`).
//!
//! DuckDB — встраиваемый колоночный OLAP: один файл, без сервера, быстрый на
//! агрегатах «оборот по секторам по дням». Крейт `duckdb` с фичей `bundled`
//! компилирует движок из исходников, поэтому зависимость по умолчанию выключена
//! (см. `Cargo.toml`), чтобы базовая сборка/CI оставались кросс-платформенными.
//!
//! Записи идемпотентны (`INSERT OR REPLACE` по первичным ключам схемы), что
//! совпадает с семантикой [`crate::mem::MemStore`].

use duckdb::{params, Connection};

use domain::algo::{ClientGroup, FutoiPoint, Hi2Point, SuperCandle};
use domain::history::{DataSource, DatasetMeta, HistoryBar, TimeRange};
use domain::{AssetClass, Bar, Instrument, TimeFrame, Trade};

use crate::migrate;
use crate::schema::SCHEMA_VERSION;
use crate::store::{AlgoObstatsRecord, AlgoOrderstatsRecord, SectorEntry, Store, TurnoverSnapshot};
use crate::StorageError;

/// Хранилище поверх DuckDB.
pub struct DuckStore {
    conn: Connection,
}

/// Привести ошибку DuckDB к [`StorageError`].
fn db(e: impl std::fmt::Display) -> StorageError {
    StorageError::Db(e.to_string())
}

impl DuckStore {
    /// Открыть (или создать) файловую БД по пути.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, StorageError> {
        let conn = Connection::open(path).map_err(db)?;
        Ok(Self { conn })
    }

    /// БД в памяти — для тестов и эфемерных сессий.
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory().map_err(db)?;
        Ok(Self { conn })
    }
}

impl Store for DuckStore {
    fn migrate(&mut self) -> Result<(), StorageError> {
        for stmt in migrate::statements() {
            self.conn.execute_batch(stmt).map_err(db)?;
        }
        if migrate::pending(self.schema_version()?) {
            self.conn
                .execute("DELETE FROM schema_version", [])
                .map_err(db)?;
            self.conn
                .execute(
                    "INSERT INTO schema_version (version) VALUES (?)",
                    params![SCHEMA_VERSION],
                )
                .map_err(db)?;
        }
        Ok(())
    }

    fn schema_version(&self) -> Result<Option<i32>, StorageError> {
        // До миграции таблицы нет — это не ошибка, а «версии ещё нет».
        let mut stmt = match self.conn.prepare("SELECT max(version) FROM schema_version") {
            Ok(s) => s,
            Err(_) => return Ok(None),
        };
        let v: Option<i32> = stmt
            .query_row([], |row| row.get::<_, Option<i32>>(0))
            .map_err(db)?;
        Ok(v)
    }

    fn upsert_instruments(&mut self, items: &[Instrument]) -> Result<usize, StorageError> {
        let now = unix_now();
        let tx = self.conn.transaction().map_err(db)?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT OR REPLACE INTO instruments \
                     (symbol, ticker, name, asset_class, sector, lot_size, isin, updated_at) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .map_err(db)?;
            for it in items {
                stmt.execute(params![
                    it.symbol,
                    it.ticker,
                    it.name,
                    it.asset_class.code(),
                    it.sector,
                    it.lot_size as i32,
                    it.isin,
                    now,
                ])
                .map_err(db)?;
            }
        }
        tx.commit().map_err(db)?;
        Ok(items.len())
    }

    fn instruments(&self) -> Result<Vec<Instrument>, StorageError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT symbol, ticker, name, asset_class, sector, lot_size, isin \
                 FROM instruments",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map([], |row| {
                let asset_class: String = row.get(3)?;
                let lot_size: i32 = row.get(5)?;
                Ok(Instrument {
                    symbol: row.get(0)?,
                    ticker: row.get(1)?,
                    name: row.get(2)?,
                    asset_class: AssetClass::from_code(&asset_class).unwrap_or(AssetClass::Equity),
                    sector: row.get(4)?,
                    lot_size: lot_size as u32,
                    isin: row.get(6)?,
                })
            })
            .map_err(db)?;
        rows.collect::<Result<_, _>>().map_err(db)
    }

    fn insert_bars(
        &mut self,
        symbol: &str,
        tf: TimeFrame,
        bars: &[Bar],
    ) -> Result<usize, StorageError> {
        let tx = self.conn.transaction().map_err(db)?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT OR REPLACE INTO bars \
                     (symbol, timeframe, ts, open, high, low, close, volume) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .map_err(db)?;
            for b in bars {
                stmt.execute(params![
                    symbol,
                    tf.code(),
                    b.ts,
                    b.open,
                    b.high,
                    b.low,
                    b.close,
                    b.volume,
                ])
                .map_err(db)?;
            }
        }
        tx.commit().map_err(db)?;
        Ok(bars.len())
    }

    fn bars(
        &self,
        symbol: &str,
        tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<Bar>, StorageError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT ts, open, high, low, close, volume FROM bars \
                 WHERE symbol = ? AND timeframe = ? AND ts BETWEEN ? AND ? \
                 ORDER BY ts",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map(params![symbol, tf.code(), from_ts, to_ts], |row| {
                Ok(Bar {
                    ts: row.get(0)?,
                    open: row.get(1)?,
                    high: row.get(2)?,
                    low: row.get(3)?,
                    close: row.get(4)?,
                    volume: row.get(5)?,
                })
            })
            .map_err(db)?;
        rows.collect::<Result<_, _>>().map_err(db)
    }

    fn last_bar_ts(&self, symbol: &str, tf: TimeFrame) -> Result<Option<i64>, StorageError> {
        let mut stmt = self
            .conn
            .prepare("SELECT max(ts) FROM bars WHERE symbol = ? AND timeframe = ?")
            .map_err(db)?;
        let v: Option<i64> = stmt
            .query_row(params![symbol, tf.code()], |row| {
                row.get::<_, Option<i64>>(0)
            })
            .map_err(db)?;
        Ok(v)
    }

    fn insert_snapshot(
        &mut self,
        symbol: &str,
        snap: &TurnoverSnapshot,
    ) -> Result<(), StorageError> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO turnover_snapshots \
                 (symbol, ts, turnover, net_flow, change) VALUES (?, ?, ?, ?, ?)",
                params![symbol, snap.ts, snap.turnover, snap.net_flow, snap.change],
            )
            .map_err(db)?;
        Ok(())
    }

    fn snapshots(
        &self,
        symbol: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<TurnoverSnapshot>, StorageError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT ts, turnover, net_flow, change FROM turnover_snapshots \
                 WHERE symbol = ? AND ts BETWEEN ? AND ? ORDER BY ts",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map(params![symbol, from_ts, to_ts], |row| {
                Ok(TurnoverSnapshot {
                    ts: row.get(0)?,
                    turnover: row.get(1)?,
                    net_flow: row.get(2)?,
                    change: row.get(3)?,
                })
            })
            .map_err(db)?;
        rows.collect::<Result<_, _>>().map_err(db)
    }

    fn insert_trades(&mut self, symbol: &str, trades: &[Trade]) -> Result<usize, StorageError> {
        let tx = self.conn.transaction().map_err(db)?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO trades (symbol, ts, price, size, buyer_initiated) \
                     VALUES (?, ?, ?, ?, ?)",
                )
                .map_err(db)?;
            for t in trades {
                stmt.execute(params![symbol, t.ts, t.price, t.size, t.buyer_initiated])
                    .map_err(db)?;
            }
        }
        tx.commit().map_err(db)?;
        Ok(trades.len())
    }

    fn trades(&self, symbol: &str, from_ts: i64, to_ts: i64) -> Result<Vec<Trade>, StorageError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT ts, price, size, buyer_initiated FROM trades \
                 WHERE symbol = ? AND ts BETWEEN ? AND ? ORDER BY ts",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map(params![symbol, from_ts, to_ts], |row| {
                Ok(Trade {
                    ts: row.get(0)?,
                    price: row.get(1)?,
                    size: row.get(2)?,
                    buyer_initiated: row.get(3)?,
                })
            })
            .map_err(db)?;
        rows.collect::<Result<_, _>>().map_err(db)
    }

    fn upsert_sector_map(&mut self, entries: &[SectorEntry]) -> Result<usize, StorageError> {
        let tx = self.conn.transaction().map_err(db)?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT OR REPLACE INTO sector_map (key, sector, is_isin) \
                     VALUES (?, ?, ?)",
                )
                .map_err(db)?;
            for e in entries {
                stmt.execute(params![e.key, e.sector, e.is_isin])
                    .map_err(db)?;
            }
        }
        tx.commit().map_err(db)?;
        Ok(entries.len())
    }

    fn sector_map(&self) -> Result<Vec<SectorEntry>, StorageError> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, sector, is_isin FROM sector_map")
            .map_err(db)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(SectorEntry {
                    key: row.get(0)?,
                    sector: row.get(1)?,
                    is_isin: row.get(2)?,
                })
            })
            .map_err(db)?;
        rows.collect::<Result<_, _>>().map_err(db)
    }

    fn insert_algo_tradestats(
        &mut self,
        market: &str,
        candles: &[SuperCandle],
    ) -> Result<usize, StorageError> {
        let tx = self.conn.transaction().map_err(db)?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT OR REPLACE INTO algo_tradestats \
                     (secid, ts, market, pr_open, pr_high, pr_low, pr_close, pr_std, \
                      vol, val, trades, pr_vwap, pr_change, vol_b, vol_s, val_b, val_s, \
                      trades_b, trades_s, disb, pr_vwap_b, pr_vwap_s) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .map_err(db)?;
            for c in candles {
                stmt.execute(params![
                    c.secid,
                    c.ts,
                    market,
                    c.pr_open,
                    c.pr_high,
                    c.pr_low,
                    c.pr_close,
                    c.pr_std,
                    c.vol,
                    c.val,
                    c.trades,
                    c.pr_vwap,
                    c.pr_change,
                    c.vol_b,
                    c.vol_s,
                    c.val_b,
                    c.val_s,
                    c.trades_b,
                    c.trades_s,
                    c.disb,
                    c.pr_vwap_b,
                    c.pr_vwap_s,
                ])
                .map_err(db)?;
            }
        }
        tx.commit().map_err(db)?;
        Ok(candles.len())
    }

    fn algo_tradestats(
        &self,
        market: &str,
        secid: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<SuperCandle>, StorageError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT secid, ts, pr_open, pr_high, pr_low, pr_close, pr_std, vol, val, \
                 trades, pr_vwap, pr_change, vol_b, vol_s, val_b, val_s, trades_b, trades_s, \
                 disb, pr_vwap_b, pr_vwap_s \
                 FROM algo_tradestats \
                 WHERE market = ? AND secid = ? AND ts BETWEEN ? AND ? ORDER BY ts",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map(params![market, secid, from_ts, to_ts], |row| {
                Ok(SuperCandle {
                    secid: row.get(0)?,
                    ts: row.get(1)?,
                    pr_open: row.get(2)?,
                    pr_high: row.get(3)?,
                    pr_low: row.get(4)?,
                    pr_close: row.get(5)?,
                    pr_std: row.get(6)?,
                    vol: row.get(7)?,
                    val: row.get(8)?,
                    trades: row.get(9)?,
                    pr_vwap: row.get(10)?,
                    pr_change: row.get(11)?,
                    vol_b: row.get(12)?,
                    vol_s: row.get(13)?,
                    val_b: row.get(14)?,
                    val_s: row.get(15)?,
                    trades_b: row.get(16)?,
                    trades_s: row.get(17)?,
                    disb: row.get(18)?,
                    pr_vwap_b: row.get(19)?,
                    pr_vwap_s: row.get(20)?,
                })
            })
            .map_err(db)?;
        rows.collect::<Result<_, _>>().map_err(db)
    }

    fn insert_algo_futoi(
        &mut self,
        market: &str,
        points: &[FutoiPoint],
    ) -> Result<usize, StorageError> {
        let tx = self.conn.transaction().map_err(db)?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT OR REPLACE INTO algo_futoi \
                     (secid, ts, market, clgroup, pos, pos_long, pos_short, \
                      pos_long_num, pos_short_num) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .map_err(db)?;
            for p in points {
                stmt.execute(params![
                    p.secid,
                    p.ts,
                    market,
                    p.clgroup.code(),
                    p.pos,
                    p.pos_long,
                    p.pos_short,
                    p.pos_long_num,
                    p.pos_short_num,
                ])
                .map_err(db)?;
            }
        }
        tx.commit().map_err(db)?;
        Ok(points.len())
    }

    fn algo_futoi(
        &self,
        market: &str,
        secid: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<FutoiPoint>, StorageError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT secid, ts, clgroup, pos, pos_long, pos_short, \
                 pos_long_num, pos_short_num \
                 FROM algo_futoi \
                 WHERE market = ? AND secid = ? AND ts BETWEEN ? AND ? ORDER BY ts",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map(params![market, secid, from_ts, to_ts], |row| {
                let clgroup: String = row.get(2)?;
                Ok(FutoiPoint {
                    secid: row.get(0)?,
                    ts: row.get(1)?,
                    clgroup: ClientGroup::from_code(&clgroup).unwrap_or(ClientGroup::Fiz),
                    pos: row.get(3)?,
                    pos_long: row.get(4)?,
                    pos_short: row.get(5)?,
                    pos_long_num: row.get(6)?,
                    pos_short_num: row.get(7)?,
                })
            })
            .map_err(db)?;
        rows.collect::<Result<_, _>>().map_err(db)
    }

    fn insert_algo_hi2(
        &mut self,
        market: &str,
        points: &[Hi2Point],
    ) -> Result<usize, StorageError> {
        let tx = self.conn.transaction().map_err(db)?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT OR REPLACE INTO algo_hi2 (secid, ts, market, concentration) \
                     VALUES (?, ?, ?, ?)",
                )
                .map_err(db)?;
            for p in points {
                stmt.execute(params![p.secid, p.ts, market, p.concentration])
                    .map_err(db)?;
            }
        }
        tx.commit().map_err(db)?;
        Ok(points.len())
    }

    fn algo_hi2(
        &self,
        market: &str,
        secid: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<Hi2Point>, StorageError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT secid, ts, concentration FROM algo_hi2 \
                 WHERE market = ? AND secid = ? AND ts BETWEEN ? AND ? ORDER BY ts",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map(params![market, secid, from_ts, to_ts], |row| {
                Ok(Hi2Point {
                    secid: row.get(0)?,
                    ts: row.get(1)?,
                    concentration: row.get(2)?,
                })
            })
            .map_err(db)?;
        rows.collect::<Result<_, _>>().map_err(db)
    }

    fn insert_algo_obstats(
        &mut self,
        records: &[AlgoObstatsRecord],
    ) -> Result<usize, StorageError> {
        let tx = self.conn.transaction().map_err(db)?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT OR REPLACE INTO algo_obstats \
                     (secid, ts, market, spread_bbo, spread_lv10, levels_b, levels_s, \
                      vol_b, vol_s, val_b, val_s, imbalance_vol_bbo, imbalance_val_bbo) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .map_err(db)?;
            for r in records {
                stmt.execute(params![
                    r.secid,
                    r.ts,
                    r.market,
                    r.spread_bbo,
                    r.spread_lv10,
                    r.levels_b,
                    r.levels_s,
                    r.vol_b,
                    r.vol_s,
                    r.val_b,
                    r.val_s,
                    r.imbalance_vol_bbo,
                    r.imbalance_val_bbo,
                ])
                .map_err(db)?;
            }
        }
        tx.commit().map_err(db)?;
        Ok(records.len())
    }

    fn algo_obstats(
        &self,
        market: &str,
        secid: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<AlgoObstatsRecord>, StorageError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT secid, ts, market, spread_bbo, spread_lv10, levels_b, levels_s, \
                 vol_b, vol_s, val_b, val_s, imbalance_vol_bbo, imbalance_val_bbo \
                 FROM algo_obstats \
                 WHERE market = ? AND secid = ? AND ts BETWEEN ? AND ? ORDER BY ts",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map(params![market, secid, from_ts, to_ts], |row| {
                Ok(AlgoObstatsRecord {
                    secid: row.get(0)?,
                    ts: row.get(1)?,
                    market: row.get(2)?,
                    spread_bbo: row.get(3)?,
                    spread_lv10: row.get(4)?,
                    levels_b: row.get(5)?,
                    levels_s: row.get(6)?,
                    vol_b: row.get(7)?,
                    vol_s: row.get(8)?,
                    val_b: row.get(9)?,
                    val_s: row.get(10)?,
                    imbalance_vol_bbo: row.get(11)?,
                    imbalance_val_bbo: row.get(12)?,
                })
            })
            .map_err(db)?;
        rows.collect::<Result<_, _>>().map_err(db)
    }

    fn insert_algo_orderstats(
        &mut self,
        records: &[AlgoOrderstatsRecord],
    ) -> Result<usize, StorageError> {
        let tx = self.conn.transaction().map_err(db)?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT OR REPLACE INTO algo_orderstats \
                     (secid, ts, market, put_orders_b, put_orders_s, put_val_b, put_val_s, \
                      put_vol_b, put_vol_s, cancel_orders_b, cancel_orders_s, \
                      cancel_val_b, cancel_val_s, cancel_vol_b, cancel_vol_s) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .map_err(db)?;
            for r in records {
                stmt.execute(params![
                    r.secid,
                    r.ts,
                    r.market,
                    r.put_orders_b,
                    r.put_orders_s,
                    r.put_val_b,
                    r.put_val_s,
                    r.put_vol_b,
                    r.put_vol_s,
                    r.cancel_orders_b,
                    r.cancel_orders_s,
                    r.cancel_val_b,
                    r.cancel_val_s,
                    r.cancel_vol_b,
                    r.cancel_vol_s,
                ])
                .map_err(db)?;
            }
        }
        tx.commit().map_err(db)?;
        Ok(records.len())
    }

    fn algo_orderstats(
        &self,
        market: &str,
        secid: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<AlgoOrderstatsRecord>, StorageError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT secid, ts, market, put_orders_b, put_orders_s, put_val_b, put_val_s, \
                 put_vol_b, put_vol_s, cancel_orders_b, cancel_orders_s, \
                 cancel_val_b, cancel_val_s, cancel_vol_b, cancel_vol_s \
                 FROM algo_orderstats \
                 WHERE market = ? AND secid = ? AND ts BETWEEN ? AND ? ORDER BY ts",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map(params![market, secid, from_ts, to_ts], |row| {
                Ok(AlgoOrderstatsRecord {
                    secid: row.get(0)?,
                    ts: row.get(1)?,
                    market: row.get(2)?,
                    put_orders_b: row.get(3)?,
                    put_orders_s: row.get(4)?,
                    put_val_b: row.get(5)?,
                    put_val_s: row.get(6)?,
                    put_vol_b: row.get(7)?,
                    put_vol_s: row.get(8)?,
                    cancel_orders_b: row.get(9)?,
                    cancel_orders_s: row.get(10)?,
                    cancel_val_b: row.get(11)?,
                    cancel_val_s: row.get(12)?,
                    cancel_vol_b: row.get(13)?,
                    cancel_vol_s: row.get(14)?,
                })
            })
            .map_err(db)?;
        rows.collect::<Result<_, _>>().map_err(db)
    }

    fn insert_history_bars(&mut self, bars: &[HistoryBar]) -> Result<usize, StorageError> {
        let tx = self.conn.transaction().map_err(db)?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT OR REPLACE INTO history_bars \
                     (source, secid, tf, ts, open, high, low, close, volume, \
                      vwap, disb, oi, hi2) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .map_err(db)?;
            for b in bars {
                stmt.execute(params![
                    b.source.code(),
                    b.secid,
                    b.tf.code(),
                    b.ts,
                    b.open,
                    b.high,
                    b.low,
                    b.close,
                    b.volume,
                    b.vwap,
                    b.disb,
                    b.oi,
                    b.hi2,
                ])
                .map_err(db)?;
            }
        }
        tx.commit().map_err(db)?;
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
        let mut stmt = self
            .conn
            .prepare(
                "SELECT source, secid, tf, ts, open, high, low, close, volume, \
                 vwap, disb, oi, hi2 FROM history_bars \
                 WHERE source = ? AND secid = ? AND tf = ? AND ts BETWEEN ? AND ? \
                 ORDER BY ts",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map(
                params![source.code(), secid, tf.code(), from_ts, to_ts],
                |row| {
                    let source_code: String = row.get(0)?;
                    let tf_code: String = row.get(2)?;
                    Ok(HistoryBar {
                        source: DataSource::from_code(&source_code).unwrap_or(DataSource::Finam),
                        secid: row.get(1)?,
                        tf: TimeFrame::from_code(&tf_code).unwrap_or(TimeFrame::M5),
                        ts: row.get(3)?,
                        open: row.get(4)?,
                        high: row.get(5)?,
                        low: row.get(6)?,
                        close: row.get(7)?,
                        volume: row.get(8)?,
                        vwap: row.get(9)?,
                        disb: row.get(10)?,
                        oi: row.get(11)?,
                        hi2: row.get(12)?,
                    })
                },
            )
            .map_err(db)?;
        rows.collect::<Result<_, _>>().map_err(db)
    }

    fn last_history_bar_ts(
        &self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
    ) -> Result<Option<i64>, StorageError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT max(ts) FROM history_bars \
                 WHERE source = ? AND secid = ? AND tf = ?",
            )
            .map_err(db)?;
        let v: Option<i64> = stmt
            .query_row(params![source.code(), secid, tf.code()], |row| {
                row.get::<_, Option<i64>>(0)
            })
            .map_err(db)?;
        Ok(v)
    }

    fn upsert_dataset(&mut self, meta: &DatasetMeta) -> Result<(), StorageError> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO history_datasets \
                 (source, secid, tf, range_from, range_till, bars, updated_ts) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![
                    meta.source.code(),
                    meta.secid,
                    meta.tf.code(),
                    meta.range.from,
                    meta.range.till,
                    meta.bars as i64,
                    meta.updated_ts,
                ],
            )
            .map_err(db)?;
        Ok(())
    }

    fn datasets(&self) -> Result<Vec<DatasetMeta>, StorageError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT source, secid, tf, range_from, range_till, bars, updated_ts \
                 FROM history_datasets",
            )
            .map_err(db)?;
        let rows = stmt.query_map([], row_to_dataset).map_err(db)?;
        rows.collect::<Result<_, _>>().map_err(db)
    }

    fn dataset(
        &self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
    ) -> Result<Option<DatasetMeta>, StorageError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT source, secid, tf, range_from, range_till, bars, updated_ts \
                 FROM history_datasets WHERE source = ? AND secid = ? AND tf = ?",
            )
            .map_err(db)?;
        let mut rows = stmt
            .query_map(params![source.code(), secid, tf.code()], row_to_dataset)
            .map_err(db)?;
        match rows.next() {
            Some(r) => Ok(Some(r.map_err(db)?)),
            None => Ok(None),
        }
    }

    fn remove_dataset(
        &mut self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
    ) -> Result<bool, StorageError> {
        let n = self
            .conn
            .execute(
                "DELETE FROM history_datasets WHERE source = ? AND secid = ? AND tf = ?",
                params![source.code(), secid, tf.code()],
            )
            .map_err(db)?;
        Ok(n > 0)
    }
}

impl DuckStore {
    /// Экспортировать историю ключа (source, secid, tf) в файл Parquet
    /// (`COPY ... TO ... (FORMAT PARQUET)`, `11.2.6`). Путь подставляется в SQL
    /// как строковый литерал (одинарные кавычки экранируются), фильтр — через
    /// параметры. Переносимый снимок для обмена/архива и воспроизводимости
    /// бэктеста.
    pub fn export_history_parquet(
        &self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), StorageError> {
        let path = sql_string_literal(&path.as_ref().to_string_lossy());
        let sql = format!(
            "COPY (SELECT source, secid, tf, ts, open, high, low, close, volume, \
             vwap, disb, oi, hi2 FROM history_bars \
             WHERE source = ? AND secid = ? AND tf = ?) \
             TO {path} (FORMAT PARQUET)"
        );
        self.conn
            .execute(&sql, params![source.code(), secid, tf.code()])
            .map_err(db)?;
        Ok(())
    }

    /// Импортировать историю из файла Parquet в `history_bars`
    /// (`read_parquet`, `11.2.7`). Идемпотентно по ключу (source, secid, tf, ts)
    /// через `INSERT OR REPLACE`, поэтому повторный импорт не плодит дублей.
    /// Возвращает число прочитанных из файла строк.
    pub fn import_history_parquet(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<usize, StorageError> {
        let path = sql_string_literal(&path.as_ref().to_string_lossy());
        let sql = format!(
            "INSERT OR REPLACE INTO history_bars \
             (source, secid, tf, ts, open, high, low, close, volume, vwap, disb, oi, hi2) \
             SELECT source, secid, tf, ts, open, high, low, close, volume, vwap, disb, oi, hi2 \
             FROM read_parquet({path})"
        );
        let n = self.conn.execute(&sql, []).map_err(db)?;
        Ok(n)
    }
}

/// Собрать [`DatasetMeta`] из строки `history_datasets`.
fn row_to_dataset(row: &duckdb::Row<'_>) -> duckdb::Result<DatasetMeta> {
    let source_code: String = row.get(0)?;
    let tf_code: String = row.get(2)?;
    let bars: i64 = row.get(5)?;
    Ok(DatasetMeta {
        source: DataSource::from_code(&source_code).unwrap_or(DataSource::Finam),
        secid: row.get(1)?,
        tf: TimeFrame::from_code(&tf_code).unwrap_or(TimeFrame::M5),
        range: TimeRange::new(row.get(3)?, row.get(4)?),
        bars: bars as u64,
        updated_ts: row.get(6)?,
    })
}

/// Обернуть значение в одинарные кавычки SQL с экранированием (`'` → `''`).
/// Пути/значения здесь контролируемые, но экранирование исключает поломку SQL
/// на путях с апострофом.
fn sql_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Текущее время в UNIX-секундах UTC (для `instruments.updated_at`).
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::SCHEMA_VERSION;

    fn store() -> DuckStore {
        let mut s = DuckStore::open_in_memory().unwrap();
        s.migrate().unwrap();
        s
    }

    fn bar(ts: i64, open: f64, close: f64, vol: f64) -> Bar {
        Bar {
            ts,
            open,
            high: open.max(close),
            low: open.min(close),
            close,
            volume: vol,
        }
    }

    #[test]
    fn migrate_is_idempotent_and_sets_version() {
        let mut s = DuckStore::open_in_memory().unwrap();
        assert_eq!(s.schema_version().unwrap(), None);
        s.migrate().unwrap();
        s.migrate().unwrap(); // повторно — без ошибок и дублей версии
        assert_eq!(s.schema_version().unwrap(), Some(SCHEMA_VERSION));
    }

    #[test]
    fn bars_roundtrip_ordered_and_upserted() {
        let mut s = store();
        s.insert_bars(
            "SBER@MISX",
            TimeFrame::D1,
            &[bar(3, 30.0, 31.0, 1.0), bar(1, 10.0, 11.0, 1.0)],
        )
        .unwrap();
        // перезапись ts=1
        s.insert_bars("SBER@MISX", TimeFrame::D1, &[bar(1, 10.0, 99.0, 1.0)])
            .unwrap();

        let got = s.bars("SBER@MISX", TimeFrame::D1, 0, 9).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].ts, 1);
        assert!((got[0].close - 99.0).abs() < 1e-9);
        assert_eq!(got[1].ts, 3);
        assert_eq!(s.last_bar_ts("SBER@MISX", TimeFrame::D1).unwrap(), Some(3));
        // другой тайм-фрейм изолирован
        assert!(s.bars("SBER@MISX", TimeFrame::H1, 0, 9).unwrap().is_empty());
    }

    #[test]
    fn instruments_roundtrip_with_nullable_fields() {
        let mut s = store();
        let items = [
            Instrument {
                symbol: "SBER@MISX".into(),
                ticker: "SBER".into(),
                name: "Сбербанк".into(),
                asset_class: AssetClass::Equity,
                sector: Some("Финансы".into()),
                lot_size: 10,
                isin: Some("RU0009029540".into()),
            },
            Instrument {
                symbol: "SiZ5@RTSX".into(),
                ticker: "SiZ5".into(),
                name: "Si фьючерс".into(),
                asset_class: AssetClass::Future,
                sector: None,
                lot_size: 1,
                isin: None,
            },
        ];
        s.upsert_instruments(&items).unwrap();
        let mut got = s.instruments().unwrap();
        got.sort_by(|a, b| a.symbol.cmp(&b.symbol));
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].asset_class, AssetClass::Equity);
        assert_eq!(got[0].sector.as_deref(), Some("Финансы"));
        assert_eq!(got[1].asset_class, AssetClass::Future);
        assert_eq!(got[1].sector, None);
        assert_eq!(got[1].isin, None);
    }

    #[test]
    fn snapshots_and_sector_map_roundtrip() {
        let mut s = store();
        let snap = TurnoverSnapshot {
            ts: 100,
            turnover: 1234.5,
            net_flow: -10.0,
            change: -0.02,
        };
        s.insert_snapshot("SBER@MISX", &snap).unwrap();
        assert_eq!(s.snapshots("SBER@MISX", 0, 200).unwrap(), vec![snap]);

        s.upsert_sector_map(&[SectorEntry {
            key: "SBER".into(),
            sector: "Финансы".into(),
            is_isin: false,
        }])
        .unwrap();
        let sm = s.sector_map().unwrap();
        assert_eq!(sm.len(), 1);
        assert!(!sm[0].is_isin);
    }

    #[test]
    fn trades_append_and_range_roundtrip() {
        let mut s = store();
        let t = |ts: i64, price: f64, size: f64, bi: Option<bool>| Trade {
            ts,
            price,
            size,
            buyer_initiated: bi,
        };
        s.insert_trades(
            "SBER@MISX",
            &[t(1, 10.0, 2.0, None), t(2, 20.0, 3.0, Some(false))],
        )
        .unwrap();
        s.insert_trades("SBER@MISX", &[t(2, 21.0, 4.0, Some(true))])
            .unwrap();

        let got = s.trades("SBER@MISX", 0, 9).unwrap();
        assert_eq!(got.len(), 3); // append-only, без перезаписи по (symbol, ts)
        assert_eq!(got[0].ts, 1);
        assert_eq!(got[0].buyer_initiated, None);
        // окно усекает
        assert_eq!(s.trades("SBER@MISX", 2, 2).unwrap().len(), 2);
        assert!(s.trades("GAZP@MISX", 0, 9).unwrap().is_empty());
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
    fn algo_tradestats_roundtrip_upserted_ordered_and_isolated_by_market() {
        let mut s = store();
        s.insert_algo_tradestats("fo", &[candle(3, "RIH5", 30.0), candle(1, "RIH5", 10.0)])
            .unwrap();
        s.insert_algo_tradestats("fo", &[candle(1, "RIH5", 99.0)])
            .unwrap(); // перезапись ts=1

        let got = s.algo_tradestats("fo", "RIH5", 0, 9).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].ts, 1);
        assert!((got[0].pr_close - 99.0).abs() < 1e-9);
        assert_eq!(got[1].ts, 3);
        assert!(s.algo_tradestats("stock", "RIH5", 0, 9).unwrap().is_empty());
    }

    #[test]
    fn algo_futoi_roundtrip_dedup_by_clgroup() {
        let mut s = store();
        let p = |ts: i64, g: ClientGroup, long: f64, short: f64| FutoiPoint {
            ts,
            secid: "RIH5".into(),
            clgroup: g,
            pos: long + short,
            pos_long: long,
            pos_short: short,
            pos_long_num: long / 10.0,
            pos_short_num: short / 10.0,
        };
        s.insert_algo_futoi(
            "fo",
            &[
                p(1, ClientGroup::Fiz, 100.0, 50.0),
                p(1, ClientGroup::Yur, 200.0, 20.0),
            ],
        )
        .unwrap();
        s.insert_algo_futoi("fo", &[p(1, ClientGroup::Fiz, 999.0, 1.0)])
            .unwrap(); // перезапись группы fiz

        let got = s.algo_futoi("fo", "RIH5", 0, 9).unwrap();
        assert_eq!(got.len(), 2);
        let fiz = got.iter().find(|x| x.clgroup == ClientGroup::Fiz).unwrap();
        assert!((fiz.pos_long - 999.0).abs() < 1e-9);
    }

    #[test]
    fn algo_hi2_roundtrip_upserted() {
        let mut s = store();
        let p = |ts: i64, c: f64| Hi2Point {
            ts,
            secid: "SBER".into(),
            concentration: c,
        };
        s.insert_algo_hi2("stock", &[p(1, 0.2), p(2, 0.3)]).unwrap();
        s.insert_algo_hi2("stock", &[p(1, 0.9)]).unwrap();

        let got = s.algo_hi2("stock", "SBER", 0, 9).unwrap();
        assert_eq!(got.len(), 2);
        assert!((got[0].concentration - 0.9).abs() < 1e-9);
        assert!(s.algo_hi2("stock", "GAZP", 0, 9).unwrap().is_empty());
    }

    #[test]
    fn algo_obstats_and_orderstats_roundtrip_upserted() {
        let mut s = store();
        let ob = AlgoObstatsRecord {
            secid: "SBER".into(),
            ts: 1,
            market: "stock".into(),
            spread_bbo: 0.001,
            spread_lv10: 0.002,
            levels_b: 5.0,
            levels_s: 5.0,
            vol_b: 100.0,
            vol_s: 90.0,
            val_b: 1000.0,
            val_s: 900.0,
            imbalance_vol_bbo: 0.05,
            imbalance_val_bbo: 0.04,
        };
        s.insert_algo_obstats(&[ob.clone()]).unwrap();
        let mut ob2 = ob.clone();
        ob2.spread_bbo = 0.5;
        s.insert_algo_obstats(&[ob2]).unwrap(); // перезапись по (secid, ts, market)
        let got = s.algo_obstats("stock", "SBER", 0, 9).unwrap();
        assert_eq!(got.len(), 1);
        assert!((got[0].spread_bbo - 0.5).abs() < 1e-9);

        let os = AlgoOrderstatsRecord {
            secid: "SBER".into(),
            ts: 1,
            market: "stock".into(),
            put_orders_b: 5.0,
            put_orders_s: 4.0,
            put_val_b: 1000.0,
            put_val_s: 900.0,
            put_vol_b: 100.0,
            put_vol_s: 90.0,
            cancel_orders_b: 1.0,
            cancel_orders_s: 1.0,
            cancel_val_b: 100.0,
            cancel_val_s: 90.0,
            cancel_vol_b: 10.0,
            cancel_vol_s: 9.0,
        };
        s.insert_algo_orderstats(&[os]).unwrap();
        let got_os = s.algo_orderstats("stock", "SBER", 0, 9).unwrap();
        assert_eq!(got_os.len(), 1);
        assert!((got_os[0].put_orders_b - 5.0).abs() < 1e-9);
        assert!(s.algo_orderstats("fo", "SBER", 0, 9).unwrap().is_empty());
    }

    /// Миграция v2→v3: создаём БД на старом наборе DDL (без algo_* таблиц) со
    /// `schema_version=2`, затем прогоняем текущую [`Store::migrate`] — версия
    /// должна подняться, а новые таблицы появиться, без потери старых данных.
    #[test]
    fn migration_v2_to_v3_adds_algo_tables_and_bumps_version() {
        let conn = Connection::open_in_memory().unwrap();
        // Старый (v2) набор DDL: те же таблицы, что и в ALL_DDL до фазы 10.5.
        conn.execute_batch(crate::schema::DDL_SCHEMA_VERSION)
            .unwrap();
        conn.execute_batch(crate::schema::DDL_INSTRUMENTS).unwrap();
        conn.execute_batch(crate::schema::DDL_BARS).unwrap();
        conn.execute_batch(crate::schema::DDL_TURNOVER_SNAPSHOTS)
            .unwrap();
        conn.execute_batch(crate::schema::DDL_TRADES).unwrap();
        conn.execute_batch(crate::schema::DDL_SECTOR_MAP).unwrap();
        conn.execute("INSERT INTO schema_version (version) VALUES (2)", [])
            .unwrap();
        conn.execute(
            "INSERT INTO bars (symbol, timeframe, ts, open, high, low, close, volume) \
             VALUES ('SBER@MISX', 'd1', 1, 10.0, 11.0, 9.0, 10.5, 100.0)",
            [],
        )
        .unwrap();

        let mut s = DuckStore { conn };
        assert_eq!(s.schema_version().unwrap(), Some(2));
        s.migrate().unwrap();
        assert_eq!(s.schema_version().unwrap(), Some(SCHEMA_VERSION));
        assert!(SCHEMA_VERSION >= 3);

        // старые данные не потеряны
        assert_eq!(s.bars("SBER@MISX", TimeFrame::D1, 0, 9).unwrap().len(), 1);
        // новые таблицы созданы и рабочие
        s.insert_algo_hi2(
            "stock",
            &[Hi2Point {
                ts: 1,
                secid: "SBER".into(),
                concentration: 0.3,
            }],
        )
        .unwrap();
        assert_eq!(s.algo_hi2("stock", "SBER", 0, 9).unwrap().len(), 1);
    }

    fn hbar(source: DataSource, secid: &str, tf: TimeFrame, ts: i64, close: f64) -> HistoryBar {
        HistoryBar::ohlcv(source, secid, tf, ts, close, close, close, close, 10.0)
    }

    #[test]
    fn history_bars_roundtrip_upserted_and_isolated_by_key() {
        let mut s = store();
        s.insert_history_bars(&[
            hbar(DataSource::Finam, "SBER", TimeFrame::M5, 300, 10.0),
            hbar(DataSource::Finam, "SBER", TimeFrame::M5, 600, 20.0),
        ])
        .unwrap();
        s.insert_history_bars(&[hbar(DataSource::Finam, "SBER", TimeFrame::M5, 300, 99.0)])
            .unwrap(); // перезапись ts=300

        let got = s
            .history_bars(DataSource::Finam, "SBER", TimeFrame::M5, 0, 1000)
            .unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].ts, 300);
        assert!((got[0].close - 99.0).abs() < 1e-9);
        assert_eq!(
            s.last_history_bar_ts(DataSource::Finam, "SBER", TimeFrame::M5)
                .unwrap(),
            Some(600)
        );
        // другой источник/тайм-фрейм изолирован
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
    fn history_bars_preserve_nullable_algo_fields() {
        let mut s = store();
        let mut b = hbar(DataSource::MoexAlgo, "GAZP", TimeFrame::M5, 300, 100.0);
        b.vwap = Some(100.5);
        b.disb = Some(-0.1);
        s.insert_history_bars(&[b]).unwrap();
        let got = s
            .history_bars(DataSource::MoexAlgo, "GAZP", TimeFrame::M5, 0, 1000)
            .unwrap();
        assert_eq!(got[0].vwap, Some(100.5));
        assert_eq!(got[0].disb, Some(-0.1));
        assert_eq!(got[0].oi, None);
        assert_eq!(got[0].hi2, None);
    }

    #[test]
    fn catalog_roundtrip_and_missing_ranges() {
        let mut s = store();
        s.upsert_dataset(&DatasetMeta {
            source: DataSource::Finam,
            secid: "SBER".into(),
            tf: TimeFrame::M5,
            range: TimeRange::new(0, 600),
            bars: 2,
            updated_ts: 600,
        })
        .unwrap();
        // перезапись по ключу
        s.upsert_dataset(&DatasetMeta {
            source: DataSource::Finam,
            secid: "SBER".into(),
            tf: TimeFrame::M5,
            range: TimeRange::new(0, 1200),
            bars: 4,
            updated_ts: 1200,
        })
        .unwrap();
        assert_eq!(s.datasets().unwrap().len(), 1);
        let d = s
            .dataset(DataSource::Finam, "SBER", TimeFrame::M5)
            .unwrap()
            .unwrap();
        assert_eq!(d.range, TimeRange::new(0, 1200));
        assert_eq!(d.bars, 4);

        // интеграция с missing_ranges: хвост за пределами покрытия
        assert_eq!(
            s.history_missing_ranges(
                DataSource::Finam,
                "SBER",
                TimeFrame::M5,
                TimeRange::new(0, 2000)
            )
            .unwrap(),
            vec![TimeRange::new(1200, 2000)]
        );

        assert!(s
            .remove_dataset(DataSource::Finam, "SBER", TimeFrame::M5)
            .unwrap());
        assert!(s.datasets().unwrap().is_empty());
    }

    /// Каталог датасетов переживает перезапуск: пишем в файловую БД, закрываем
    /// соединение, открываем заново — строки на месте (`11.2.4`).
    #[test]
    fn catalog_survives_reopen() {
        let dir = std::env::temp_dir().join(format!("mf-hist-cat-{}", unique_suffix()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("catalog.duckdb");
        {
            let mut s = DuckStore::open(&path).unwrap();
            s.migrate().unwrap();
            s.upsert_dataset(&DatasetMeta {
                source: DataSource::MoexAlgo,
                secid: "GAZP".into(),
                tf: TimeFrame::H1,
                range: TimeRange::new(100, 700),
                bars: 6,
                updated_ts: 700,
            })
            .unwrap();
            s.insert_history_bars(&[hbar(DataSource::MoexAlgo, "GAZP", TimeFrame::H1, 100, 5.0)])
                .unwrap();
        } // соединение закрыто (Drop)

        let s = DuckStore::open(&path).unwrap();
        let cat = s.catalog().unwrap();
        assert_eq!(cat.datasets.len(), 1);
        assert_eq!(cat.datasets[0].secid, "GAZP");
        assert_eq!(cat.datasets[0].range, TimeRange::new(100, 700));
        assert_eq!(
            s.history_bars(DataSource::MoexAlgo, "GAZP", TimeFrame::H1, 0, 1000)
                .unwrap()
                .len(),
            1
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Экспорт в Parquet и импорт обратно в чистую БД дают идентичные бары
    /// (`11.2.6`/`11.2.7`).
    #[test]
    fn parquet_export_import_roundtrip() {
        let dir = std::env::temp_dir().join(format!("mf-hist-pq-{}", unique_suffix()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("sber_m5.parquet");

        let mut src = store();
        let mut b1 = hbar(DataSource::Finam, "SBER", TimeFrame::M5, 300, 10.0);
        b1.vwap = Some(10.2);
        let b2 = hbar(DataSource::Finam, "SBER", TimeFrame::M5, 600, 20.0);
        src.insert_history_bars(&[b1.clone(), b2.clone()]).unwrap();
        src.export_history_parquet(DataSource::Finam, "SBER", TimeFrame::M5, &file)
            .unwrap();

        // чистая БД: импортируем из файла
        let mut dst = store();
        let n = dst.import_history_parquet(&file).unwrap();
        assert_eq!(n, 2);
        let got = dst
            .history_bars(DataSource::Finam, "SBER", TimeFrame::M5, 0, 1000)
            .unwrap();
        assert_eq!(got, vec![b1, b2]);

        // повторный импорт не плодит дублей (INSERT OR REPLACE по ключу)
        dst.import_history_parquet(&file).unwrap();
        assert_eq!(
            dst.history_bars(DataSource::Finam, "SBER", TimeFrame::M5, 0, 1000)
                .unwrap()
                .len(),
            2
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Миграция v3→v4 на заполненной v3-базе: добавляются `history_bars`/
    /// `history_datasets`, версия поднимается, старые данные целы.
    #[test]
    fn migration_v3_to_v4_adds_history_tables() {
        let conn = Connection::open_in_memory().unwrap();
        // v3-набор DDL (без history_*), как ALL_DDL до фазы 11.2.
        conn.execute_batch(crate::schema::DDL_SCHEMA_VERSION)
            .unwrap();
        for ddl in [
            crate::schema::DDL_INSTRUMENTS,
            crate::schema::DDL_BARS,
            crate::schema::DDL_TURNOVER_SNAPSHOTS,
            crate::schema::DDL_TRADES,
            crate::schema::DDL_SECTOR_MAP,
            crate::schema::DDL_ALGO_TRADESTATS,
            crate::schema::DDL_ALGO_FUTOI,
            crate::schema::DDL_ALGO_HI2,
            crate::schema::DDL_ALGO_OBSTATS,
            crate::schema::DDL_ALGO_ORDERSTATS,
        ] {
            conn.execute_batch(ddl).unwrap();
        }
        conn.execute("INSERT INTO schema_version (version) VALUES (3)", [])
            .unwrap();
        conn.execute(
            "INSERT INTO bars (symbol, timeframe, ts, open, high, low, close, volume) \
             VALUES ('SBER@MISX', 'd1', 1, 10.0, 11.0, 9.0, 10.5, 100.0)",
            [],
        )
        .unwrap();

        let mut s = DuckStore { conn };
        assert_eq!(s.schema_version().unwrap(), Some(3));
        s.migrate().unwrap();
        assert_eq!(s.schema_version().unwrap(), Some(SCHEMA_VERSION));
        assert_eq!(SCHEMA_VERSION, 4);

        // старые данные целы, новые таблицы рабочие
        assert_eq!(s.bars("SBER@MISX", TimeFrame::D1, 0, 9).unwrap().len(), 1);
        s.insert_history_bars(&[hbar(DataSource::Finam, "SBER", TimeFrame::M5, 300, 1.0)])
            .unwrap();
        assert_eq!(
            s.history_bars(DataSource::Finam, "SBER", TimeFrame::M5, 0, 1000)
                .unwrap()
                .len(),
            1
        );
    }

    /// Грубый уникальный суффикс для временных путей (без внешних крейтов).
    fn unique_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    }
}
