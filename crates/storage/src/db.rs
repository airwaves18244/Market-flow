//! Соединение с DuckDB и инициализация схемы.
//!
//! [`Db`] оборачивает [`duckdb::Connection`], применяя миграции при открытии,
//! так что после `open*` база гарантированно на актуальной версии схемы.

use std::path::Path;

use duckdb::Connection;

use crate::{ingest, migrate, query, Result};

/// Открытое аналитическое хранилище.
pub struct Db {
    conn: Connection,
}

impl Db {
    /// Открыть (или создать) файловую БД по пути и применить миграции.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    /// Открыть БД в памяти (для тестов и эфемерных сессий) и применить миграции.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Self> {
        migrate::apply(&conn)?;
        Ok(Self { conn })
    }

    /// Текущая версия схемы (число применённых миграций по версиям).
    pub fn schema_version(&self) -> Result<u32> {
        migrate::current_version(&self.conn)
    }

    /// Низкоуровневый доступ к соединению (для запросов вне готовых хелперов).
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    // --- Ингест (§ 1.3, § 1.5) -------------------------------------------

    /// Upsert справочника инструментов.
    pub fn upsert_instruments(&self, instruments: &[domain::Instrument]) -> Result<usize> {
        ingest::upsert_instruments(&self.conn, instruments)
    }

    /// Батч-вставка баров одного инструмента и тайм-фрейма.
    pub fn insert_bars(
        &self,
        symbol: &str,
        tf: crate::TimeFrame,
        bars: &[domain::Bar],
    ) -> Result<usize> {
        ingest::insert_bars(&self.conn, symbol, tf, bars)
    }

    /// Батч-вставка снимков оборота.
    pub fn insert_turnover_snapshots(&self, snapshots: &[ingest::SymbolSnapshot]) -> Result<usize> {
        ingest::insert_turnover_snapshots(&self.conn, snapshots)
    }

    /// Загрузить пары «ключ → сектор» в таблицу классификации.
    pub fn upsert_sector_map<'a>(
        &self,
        pairs: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<usize> {
        ingest::upsert_sector_map(&self.conn, pairs)
    }

    /// Проставить `instruments.sector` из таблицы классификации
    /// (приоритет соответствия по тикеру над ISIN). Возвращает число строк.
    pub fn apply_sectors_to_instruments(&self) -> Result<usize> {
        ingest::apply_sectors_to_instruments(&self.conn)
    }

    // --- Запросы (§ 1.7) --------------------------------------------------

    /// Оборот и нетто-поток по секторам за период `[from_ts, to_ts]`.
    pub fn turnover_by_sector(
        &self,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<query::SectorTurnover>> {
        query::turnover_by_sector(&self.conn, from_ts, to_ts)
    }

    /// Топ-движения за период (по модулю изменения), не более `limit`.
    pub fn top_movers(&self, from_ts: i64, to_ts: i64, limit: usize) -> Result<Vec<query::Mover>> {
        query::top_movers(&self.conn, from_ts, to_ts, limit)
    }

    /// Временной ряд нетто-потока инструмента за период.
    pub fn net_flow_series(
        &self,
        symbol: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<query::FlowPoint>> {
        query::net_flow_series(&self.conn, symbol, from_ts, to_ts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory_applies_schema() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.schema_version().unwrap(), 1);
    }

    #[test]
    fn migrations_are_idempotent_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("market.duckdb");
        {
            let db = Db::open(&path).unwrap();
            assert_eq!(db.schema_version().unwrap(), 1);
        }
        // Повторное открытие не накатывает миграции заново и не падает.
        let db = Db::open(&path).unwrap();
        assert_eq!(db.schema_version().unwrap(), 1);
    }
}
