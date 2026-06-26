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

use domain::{AssetClass, Bar, Instrument, TimeFrame};

use crate::migrate;
use crate::schema::SCHEMA_VERSION;
use crate::store::{SectorEntry, Store, TurnoverSnapshot};
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
}
