//! Движок хранилища поверх встраиваемого DuckDB.
//!
//! [`Store`] открывает соединение (файл на диске или in-memory для тестов),
//! накатывает миграции схемы и предоставляет ингест-writer'ы и аналитические
//! запросы. Время везде — UNIX-секунды UTC (`i64`), как в [`domain`].

use std::path::Path;

use domain::{AssetClass, Bar, Instrument};
use duckdb::{params, Connection};

use crate::schema::{self, Migration};
use crate::StorageError;

impl From<duckdb::Error> for StorageError {
    fn from(e: duckdb::Error) -> Self {
        StorageError::Db(e.to_string())
    }
}

/// Снимок накопленного дневного оборота инструмента на момент сканирования.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurnoverSnapshot {
    pub ts: i64,
    pub turnover: f64,
    pub net_flow: f64,
    pub change: f64,
}

/// Локальное аналитическое хранилище (DuckDB).
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Открыть/создать хранилище в файле и накатить миграции.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let conn = Connection::open(path)?;
        Self::from_conn(conn)
    }

    /// In-memory хранилище (для тестов и эфемерных расчётов).
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        Self::from_conn(conn)
    }

    fn from_conn(conn: Connection) -> Result<Self, StorageError> {
        let store = Store { conn };
        store.migrate()?;
        Ok(store)
    }

    /// Накатить недостающие миграции (идемпотентно).
    fn migrate(&self) -> Result<(), StorageError> {
        self.conn.execute_batch(schema::DDL_SCHEMA_MIGRATIONS)?;
        let current = self.schema_version()?;
        for m in schema::MIGRATIONS {
            if m.version <= current {
                continue;
            }
            self.apply_migration(m)?;
        }
        Ok(())
    }

    fn apply_migration(&self, m: &Migration) -> Result<(), StorageError> {
        for stmt in m.statements {
            self.conn.execute_batch(stmt).map_err(|e| {
                StorageError::Migration(format!("v{}: {e}", m.version))
            })?;
        }
        self.conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?, ?)",
            params![m.version, now_unix()],
        )?;
        Ok(())
    }

    /// Текущая (максимальная применённая) версия схемы; `0` — пустая БД.
    pub fn schema_version(&self) -> Result<i64, StorageError> {
        let v: Option<i64> = self.conn.query_row(
            "SELECT max(version) FROM schema_migrations",
            [],
            |r| r.get(0),
        )?;
        Ok(v.unwrap_or(0))
    }

    // ── Ингест ──────────────────────────────────────────────────────────

    /// Вставить/обновить пачку инструментов (в одной транзакции).
    pub fn upsert_instruments(&self, items: &[Instrument]) -> Result<usize, StorageError> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO instruments
                 (symbol, ticker, name, asset_class, sector, lot_size, isin, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )?;
            let now = now_unix();
            for it in items {
                stmt.execute(params![
                    it.symbol,
                    it.ticker,
                    it.name,
                    it.asset_class.code(),
                    it.sector,
                    it.lot_size,
                    it.isin,
                    now,
                ])?;
            }
        }
        tx.commit()?;
        Ok(items.len())
    }

    /// Вставить/обновить пачку баров одного инструмента и тайм-фрейма.
    pub fn insert_bars(
        &self,
        symbol: &str,
        timeframe: &str,
        bars: &[Bar],
    ) -> Result<usize, StorageError> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO bars
                 (symbol, timeframe, ts, open, high, low, close, volume)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )?;
            for b in bars {
                stmt.execute(params![
                    symbol, timeframe, b.ts, b.open, b.high, b.low, b.close, b.volume
                ])?;
            }
        }
        tx.commit()?;
        Ok(bars.len())
    }

    /// Записать снимок накопленного дневного оборота инструмента.
    pub fn insert_turnover_snapshot(
        &self,
        symbol: &str,
        snap: TurnoverSnapshot,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO turnover_snapshots
             (symbol, ts, turnover, net_flow, change) VALUES (?, ?, ?, ?, ?)",
            params![symbol, snap.ts, snap.turnover, snap.net_flow, snap.change],
        )?;
        Ok(())
    }

    /// Полностью заменить таблицу классификации секторов.
    /// `pairs` — `(ключ, сектор, is_isin)`, ключ — тикер (в верхнем регистре) или ISIN.
    pub fn replace_sector_map(
        &self,
        pairs: &[(String, String, bool)],
    ) -> Result<usize, StorageError> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute_batch("DELETE FROM sector_map")?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO sector_map (key, sector, is_isin) VALUES (?, ?, ?)",
            )?;
            for (key, sector, is_isin) in pairs {
                stmt.execute(params![key, sector, is_isin])?;
            }
        }
        tx.commit()?;
        Ok(pairs.len())
    }

    /// Проставить сектор инструментам из таблицы классификации.
    /// Приоритет у совпадения по тикеру, затем — по ISIN. Возвращает число
    /// обновлённых строк.
    pub fn apply_sectors_to_instruments(&self) -> Result<usize, StorageError> {
        // По тикеру (key хранится в верхнем регистре).
        let by_ticker = self.conn.execute(
            "UPDATE instruments SET sector = sm.sector
             FROM sector_map sm
             WHERE sm.is_isin = false AND sm.key = upper(instruments.ticker)",
            [],
        )?;
        // По ISIN — только там, где сектор ещё не проставлен.
        let by_isin = self.conn.execute(
            "UPDATE instruments SET sector = sm.sector
             FROM sector_map sm
             WHERE sm.is_isin = true AND sm.key = instruments.isin
               AND instruments.sector IS NULL",
            [],
        )?;
        Ok(by_ticker + by_isin)
    }

    // ── Запросы ─────────────────────────────────────────────────────────

    /// Бары инструмента в полуинтервале `[from_ts, to_ts)`, по возрастанию времени.
    pub fn bars(
        &self,
        symbol: &str,
        timeframe: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<Bar>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT ts, open, high, low, close, volume FROM bars
             WHERE symbol = ? AND timeframe = ? AND ts >= ? AND ts < ?
             ORDER BY ts",
        )?;
        let rows = stmt.query_map(params![symbol, timeframe, from_ts, to_ts], |r| {
            Ok(Bar {
                ts: r.get(0)?,
                open: r.get(1)?,
                high: r.get(2)?,
                low: r.get(3)?,
                close: r.get(4)?,
                volume: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Инструменты заданного класса активов (по возрастанию тикера).
    pub fn instruments_by_class(
        &self,
        class: AssetClass,
    ) -> Result<Vec<Instrument>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT symbol, ticker, name, asset_class, sector, lot_size, isin
             FROM instruments WHERE asset_class = ? ORDER BY ticker",
        )?;
        let rows = stmt.query_map(params![class.code()], |r| {
            let class_code: String = r.get(3)?;
            Ok(Instrument {
                symbol: r.get(0)?,
                ticker: r.get(1)?,
                name: r.get(2)?,
                asset_class: parse_class(&class_code),
                sector: r.get(4)?,
                lot_size: r.get::<_, i64>(5)? as u32,
                isin: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Последний (по времени) снимок оборота инструмента.
    pub fn latest_snapshot(
        &self,
        symbol: &str,
    ) -> Result<Option<TurnoverSnapshot>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT ts, turnover, net_flow, change FROM turnover_snapshots
             WHERE symbol = ? ORDER BY ts DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![symbol], |r| {
            Ok(TurnoverSnapshot {
                ts: r.get(0)?,
                turnover: r.get(1)?,
                net_flow: r.get(2)?,
                change: r.get(3)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// Суммарный оборот по секторам по *последнему снимку каждого инструмента*
    /// на момент `>= since_ts`. Возвращает пары `(сектор, оборот)` по убыванию.
    pub fn turnover_by_sector(
        &self,
        since_ts: i64,
    ) -> Result<Vec<(String, f64)>, StorageError> {
        let mut stmt = self.conn.prepare(
            "WITH latest AS (
                 SELECT symbol, turnover,
                        row_number() OVER (PARTITION BY symbol ORDER BY ts DESC) AS rn
                 FROM turnover_snapshots WHERE ts >= ?
             )
             SELECT coalesce(i.sector, 'Без сектора') AS sector, sum(l.turnover) AS turnover
             FROM latest l JOIN instruments i ON i.symbol = l.symbol
             WHERE l.rn = 1
             GROUP BY 1 ORDER BY 2 DESC",
        )?;
        let rows = stmt.query_map(params![since_ts], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

fn parse_class(code: &str) -> AssetClass {
    match code {
        "future" => AssetClass::Future,
        "bond" => AssetClass::Bond,
        _ => AssetClass::Equity,
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inst(symbol: &str, ticker: &str, class: AssetClass, isin: Option<&str>) -> Instrument {
        Instrument {
            symbol: symbol.into(),
            ticker: ticker.into(),
            name: ticker.into(),
            asset_class: class,
            sector: None,
            lot_size: 1,
            isin: isin.map(Into::into),
        }
    }

    fn bar(ts: i64, close: f64, volume: f64) -> Bar {
        Bar { ts, open: close, high: close, low: close, close, volume }
    }

    #[test]
    fn migrations_bring_schema_to_target() {
        let s = Store::open_in_memory().unwrap();
        assert_eq!(s.schema_version().unwrap(), schema::target_version());
        // Повторный накат идемпотентен.
        s.migrate().unwrap();
        assert_eq!(s.schema_version().unwrap(), schema::target_version());
    }

    #[test]
    fn instruments_roundtrip_by_class() {
        let s = Store::open_in_memory().unwrap();
        s.upsert_instruments(&[
            inst("SBER@MISX", "SBER", AssetClass::Equity, Some("RU0009029540")),
            inst("SiM5@RTSX", "SiM5", AssetClass::Future, None),
        ])
        .unwrap();
        let eq = s.instruments_by_class(AssetClass::Equity).unwrap();
        assert_eq!(eq.len(), 1);
        assert_eq!(eq[0].ticker, "SBER");
        // upsert перезаписывает, а не дублирует.
        s.upsert_instruments(&[inst("SBER@MISX", "SBER", AssetClass::Equity, None)])
            .unwrap();
        assert_eq!(s.instruments_by_class(AssetClass::Equity).unwrap().len(), 1);
    }

    #[test]
    fn bars_roundtrip_is_ordered_and_half_open() {
        let s = Store::open_in_memory().unwrap();
        s.insert_bars("SBER@MISX", "d1", &[bar(30, 3.0, 1.0), bar(10, 1.0, 1.0), bar(20, 2.0, 1.0)])
            .unwrap();
        let got = s.bars("SBER@MISX", "d1", 10, 30).unwrap();
        // [10, 30): 30 исключён.
        assert_eq!(got.iter().map(|b| b.ts).collect::<Vec<_>>(), vec![10, 20]);
    }

    #[test]
    fn latest_snapshot_returns_newest() {
        let s = Store::open_in_memory().unwrap();
        s.insert_turnover_snapshot("SBER@MISX", TurnoverSnapshot { ts: 100, turnover: 1.0, net_flow: 0.5, change: 0.01 }).unwrap();
        s.insert_turnover_snapshot("SBER@MISX", TurnoverSnapshot { ts: 200, turnover: 9.0, net_flow: 1.5, change: 0.02 }).unwrap();
        let snap = s.latest_snapshot("SBER@MISX").unwrap().unwrap();
        assert_eq!(snap.ts, 200);
        assert_eq!(snap.turnover, 9.0);
        assert!(s.latest_snapshot("NOPE").unwrap().is_none());
    }

    #[test]
    fn sector_map_applies_by_ticker_then_isin() {
        let s = Store::open_in_memory().unwrap();
        s.upsert_instruments(&[
            inst("SBER@MISX", "SBER", AssetClass::Equity, Some("RU0009029540")),
            inst("GAZP@MISX", "GAZP", AssetClass::Equity, Some("RU0007661625")),
        ])
        .unwrap();
        s.replace_sector_map(&[
            ("SBER".into(), "Финансы".into(), false),
            ("RU0007661625".into(), "Нефтегаз".into(), true),
        ])
        .unwrap();
        let updated = s.apply_sectors_to_instruments().unwrap();
        assert_eq!(updated, 2);
        let eq = s.instruments_by_class(AssetClass::Equity).unwrap();
        let sber = eq.iter().find(|i| i.ticker == "SBER").unwrap();
        let gazp = eq.iter().find(|i| i.ticker == "GAZP").unwrap();
        assert_eq!(sber.sector.as_deref(), Some("Финансы")); // по тикеру
        assert_eq!(gazp.sector.as_deref(), Some("Нефтегаз")); // по ISIN
    }

    #[test]
    fn turnover_by_sector_uses_latest_per_symbol() {
        let s = Store::open_in_memory().unwrap();
        let mut sber = inst("SBER@MISX", "SBER", AssetClass::Equity, None);
        sber.sector = Some("Финансы".into());
        let mut gazp = inst("GAZP@MISX", "GAZP", AssetClass::Equity, None);
        gazp.sector = Some("Нефтегаз".into());
        let mut lkoh = inst("LKOH@MISX", "LKOH", AssetClass::Equity, None);
        lkoh.sector = Some("Нефтегаз".into());
        s.upsert_instruments(&[sber, gazp, lkoh]).unwrap();

        // По два снимка на инструмент — учитываться должен последний.
        for (sym, ts, t) in [
            ("SBER@MISX", 100, 1.0), ("SBER@MISX", 200, 10.0),
            ("GAZP@MISX", 150, 5.0), ("GAZP@MISX", 250, 7.0),
            ("LKOH@MISX", 250, 3.0),
        ] {
            s.insert_turnover_snapshot(sym, TurnoverSnapshot { ts, turnover: t, net_flow: 0.0, change: 0.0 }).unwrap();
        }
        let by_sector = s.turnover_by_sector(0).unwrap();
        // Нефтегаз: 7 + 3 = 10; Финансы: 10. Сортировка по убыванию, ties — любой порядок.
        let map: std::collections::HashMap<_, _> = by_sector.into_iter().collect();
        assert_eq!(map["Нефтегаз"], 10.0);
        assert_eq!(map["Финансы"], 10.0);
    }
}
