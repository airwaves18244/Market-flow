//! Локальное аналитическое хранилище на DuckDB.
//!
//! `Storage` — тонкий адаптер над встроенным DuckDB: применяет схему при первом
//! открытии и предоставляет методы ингеста и чтения доменных типов.
//! Вся аналитическая математика остаётся в `domain::metrics`.

pub mod schema;

use domain::{Bar, Instrument};
use duckdb::{params, Connection};

/// Ошибки хранилища.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("ошибка БД: {0}")]
    Db(String),
    #[error("миграция не применена: {0}")]
    Migration(String),
}

impl From<duckdb::Error> for StorageError {
    fn from(e: duckdb::Error) -> Self {
        StorageError::Db(e.to_string())
    }
}

/// Снимок оборота по инструменту — записывается планировщиком ингеста.
#[derive(Debug, Clone)]
pub struct TurnoverSnapshot {
    pub symbol: String,
    pub ts: i64,
    pub turnover: f64,
    pub net_flow: f64,
    pub change: f64,
}

/// Локальное аналитическое хранилище.
pub struct Storage {
    conn: Connection,
}

impl Storage {
    /// Открыть или создать файловое хранилище. Применяет DDL-схему.
    pub fn open(path: &std::path::Path) -> Result<Self, StorageError> {
        let conn = Connection::open(path)?;
        let s = Self { conn };
        s.apply_schema()?;
        Ok(s)
    }

    /// In-memory хранилище — для тестов и режима dry-run.
    pub fn in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        let s = Self { conn };
        s.apply_schema()?;
        Ok(s)
    }

    fn apply_schema(&self) -> Result<(), StorageError> {
        for ddl in schema::ALL_DDL {
            self.conn
                .execute_batch(ddl)
                .map_err(|e| StorageError::Migration(e.to_string()))?;
        }
        Ok(())
    }

    // ── Запись ──────────────────────────────────────────────────────────────

    /// Вставить или обновить инструмент в справочнике.
    pub fn upsert_instrument(&self, inst: &Instrument, now_ts: i64) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO instruments (symbol, ticker, name, asset_class, sector, lot_size, isin, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (symbol) DO UPDATE SET
               ticker     = excluded.ticker,
               name       = excluded.name,
               asset_class= excluded.asset_class,
               sector     = excluded.sector,
               lot_size   = excluded.lot_size,
               isin       = excluded.isin,
               updated_at = excluded.updated_at",
            params![
                inst.symbol,
                inst.ticker,
                inst.name,
                inst.asset_class.code(),
                inst.sector.as_deref(),
                inst.lot_size,
                inst.isin.as_deref(),
                now_ts
            ],
        )?;
        Ok(())
    }

    /// Вставить бары (уже известные дубли обновляются).
    ///
    /// `timeframe` — строковый код: `m1|m5|m15|h1|d1`.
    pub fn insert_bars(
        &self,
        symbol: &str,
        timeframe: &str,
        bars: &[Bar],
    ) -> Result<(), StorageError> {
        let mut stmt = self.conn.prepare(
            "INSERT INTO bars (symbol, timeframe, ts, open, high, low, close, volume)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (symbol, timeframe, ts) DO UPDATE SET
               open   = excluded.open,
               high   = excluded.high,
               low    = excluded.low,
               close  = excluded.close,
               volume = excluded.volume",
        )?;
        for b in bars {
            stmt.execute(params![
                symbol, timeframe, b.ts, b.open, b.high, b.low, b.close, b.volume
            ])?;
        }
        Ok(())
    }

    /// Записать снимок агрегированного оборота.
    pub fn insert_turnover_snapshot(&self, snap: &TurnoverSnapshot) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO turnover_snapshots (symbol, ts, turnover, net_flow, change)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (symbol, ts) DO UPDATE SET
               turnover = excluded.turnover,
               net_flow = excluded.net_flow,
               change   = excluded.change",
            params![
                snap.symbol,
                snap.ts,
                snap.turnover,
                snap.net_flow,
                snap.change
            ],
        )?;
        Ok(())
    }

    /// Добавить или обновить запись в таблице секторной классификации.
    pub fn upsert_sector_entry(
        &self,
        key: &str,
        sector: &str,
        is_isin: bool,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO sector_map (key, sector, is_isin) VALUES (?, ?, ?)
             ON CONFLICT (key) DO UPDATE SET sector = excluded.sector, is_isin = excluded.is_isin",
            params![key, sector, is_isin],
        )?;
        Ok(())
    }

    // ── Чтение ──────────────────────────────────────────────────────────────

    /// Бары по символу и тайм-фрейму, начиная с `since_ts` включительно.
    pub fn bars_since(
        &self,
        symbol: &str,
        timeframe: &str,
        since_ts: i64,
    ) -> Result<Vec<Bar>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT ts, open, high, low, close, volume
             FROM bars
             WHERE symbol = ? AND timeframe = ? AND ts >= ?
             ORDER BY ts ASC",
        )?;
        let rows = stmt.query_map(params![symbol, timeframe, since_ts], |row| {
            Ok(Bar {
                ts: row.get(0)?,
                open: row.get(1)?,
                high: row.get(2)?,
                low: row.get(3)?,
                close: row.get(4)?,
                volume: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| StorageError::Db(e.to_string()))
    }

    /// Снимки оборота, начиная с `since_ts` включительно.
    pub fn snapshots_since(&self, since_ts: i64) -> Result<Vec<TurnoverSnapshot>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT symbol, ts, turnover, net_flow, change
             FROM turnover_snapshots
             WHERE ts >= ?
             ORDER BY ts ASC",
        )?;
        let rows = stmt.query_map(params![since_ts], |row| {
            Ok(TurnoverSnapshot {
                symbol: row.get(0)?,
                ts: row.get(1)?,
                turnover: row.get(2)?,
                net_flow: row.get(3)?,
                change: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| StorageError::Db(e.to_string()))
    }

    /// Загрузить таблицу классификации секторов как список пар `(ключ, сектор, is_isin)`.
    ///
    /// Вызывающий код строит из них [`data::classify::SectorMap`].
    pub fn load_sector_pairs(&self) -> Result<Vec<(String, String, bool)>, StorageError> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, sector, is_isin FROM sector_map")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, bool>(2)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| StorageError::Db(e.to_string()))
    }

    /// Список всех символов заданного класса активов.
    pub fn symbols_by_class(&self, asset_class: &str) -> Result<Vec<String>, StorageError> {
        let mut stmt = self
            .conn
            .prepare("SELECT symbol FROM instruments WHERE asset_class = ? ORDER BY symbol")?;
        let rows = stmt.query_map(params![asset_class], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| StorageError::Db(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{AssetClass, Bar, Instrument};

    fn test_instrument(symbol: &str, sector: Option<&str>) -> Instrument {
        Instrument {
            symbol: symbol.to_string(),
            ticker: symbol.split('@').next().unwrap_or(symbol).to_string(),
            name: format!("{symbol} Inc."),
            asset_class: AssetClass::Equity,
            sector: sector.map(str::to_string),
            lot_size: 10,
            isin: None,
        }
    }

    #[test]
    fn schema_applied_on_open() {
        let s = Storage::in_memory().unwrap();
        // Если схема не применилась — SELECT упадёт
        let rows: Vec<String> = {
            let mut stmt = s
                .conn
                .prepare("SELECT symbol FROM instruments LIMIT 0")
                .unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert!(rows.is_empty());
    }

    #[test]
    fn upsert_and_read_instrument() {
        let s = Storage::in_memory().unwrap();
        let inst = test_instrument("SBER@MISX", Some("Финансы"));
        s.upsert_instrument(&inst, 1_000_000).unwrap();

        let symbols = s.symbols_by_class("equity").unwrap();
        assert_eq!(symbols, vec!["SBER@MISX"]);
    }

    #[test]
    fn upsert_instrument_updates_existing() {
        let s = Storage::in_memory().unwrap();
        let inst = test_instrument("SBER@MISX", Some("Финансы"));
        s.upsert_instrument(&inst, 1_000_000).unwrap();

        let updated = Instrument {
            sector: Some("Банки".to_string()),
            ..inst.clone()
        };
        s.upsert_instrument(&updated, 2_000_000).unwrap();

        // всё ещё один инструмент
        assert_eq!(s.symbols_by_class("equity").unwrap().len(), 1);
    }

    #[test]
    fn insert_and_retrieve_bars() {
        let s = Storage::in_memory().unwrap();
        let bars = vec![
            Bar {
                ts: 1000,
                open: 100.0,
                high: 110.0,
                low: 95.0,
                close: 105.0,
                volume: 500.0,
            },
            Bar {
                ts: 1060,
                open: 105.0,
                high: 112.0,
                low: 103.0,
                close: 108.0,
                volume: 300.0,
            },
            Bar {
                ts: 1120,
                open: 108.0,
                high: 115.0,
                low: 106.0,
                close: 113.0,
                volume: 400.0,
            },
        ];
        s.insert_bars("SBER@MISX", "m1", &bars).unwrap();

        let got = s.bars_since("SBER@MISX", "m1", 1060).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].ts, 1060);
        assert_eq!(got[1].ts, 1120);
    }

    #[test]
    fn insert_bars_upserts_on_conflict() {
        let s = Storage::in_memory().unwrap();
        let original = vec![Bar {
            ts: 1000,
            open: 100.0,
            high: 110.0,
            low: 95.0,
            close: 105.0,
            volume: 500.0,
        }];
        s.insert_bars("SBER@MISX", "m1", &original).unwrap();

        let revised = vec![Bar {
            ts: 1000,
            open: 100.0,
            high: 120.0,
            low: 95.0,
            close: 118.0,
            volume: 600.0,
        }];
        s.insert_bars("SBER@MISX", "m1", &revised).unwrap();

        let got = s.bars_since("SBER@MISX", "m1", 0).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].high, 120.0);
    }

    #[test]
    fn turnover_snapshot_roundtrip() {
        let s = Storage::in_memory().unwrap();
        let snap = TurnoverSnapshot {
            symbol: "GAZP@MISX".to_string(),
            ts: 2_000_000,
            turnover: 1_234_567.0,
            net_flow: 50_000.0,
            change: 0.023,
        };
        s.insert_turnover_snapshot(&snap).unwrap();

        let got = s.snapshots_since(2_000_000).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].symbol, "GAZP@MISX");
        assert!((got[0].turnover - 1_234_567.0).abs() < 1.0);
    }

    #[test]
    fn sector_map_roundtrip() {
        let s = Storage::in_memory().unwrap();
        s.upsert_sector_entry("SBER", "Финансы", false).unwrap();
        s.upsert_sector_entry("RU0009029540", "Финансы", true)
            .unwrap();
        s.upsert_sector_entry("GAZP", "Нефтегаз", false).unwrap();

        let pairs = s.load_sector_pairs().unwrap();
        assert_eq!(pairs.len(), 3);
        let tickers: Vec<_> = pairs
            .iter()
            .filter(|p| !p.2)
            .map(|p| p.0.as_str())
            .collect();
        assert!(tickers.contains(&"SBER"));
        assert!(tickers.contains(&"GAZP"));
    }

    #[test]
    fn snapshots_since_filters_by_time() {
        let s = Storage::in_memory().unwrap();
        for (i, ts) in [1000i64, 2000, 3000].iter().enumerate() {
            s.insert_turnover_snapshot(&TurnoverSnapshot {
                symbol: format!("SYM{i}@MISX"),
                ts: *ts,
                turnover: 100.0,
                net_flow: 0.0,
                change: 0.0,
            })
            .unwrap();
        }
        let got = s.snapshots_since(2000).unwrap();
        assert_eq!(got.len(), 2);
    }
}
