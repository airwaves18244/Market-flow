//! Локальное аналитическое хранилище.
//!
//! Накапливает котировки/обороты во времени, чтобы считать тренды и перетоки
//! денег (а не только мгновенный снимок). Целевой движок — встраиваемый
//! **DuckDB** (колоночный OLAP, читает/пишет Parquet), идеально подходящий для
//! запросов вида «оборот по секторам по дням».
//!
//! ## Слои (Фаза 1)
//!
//! - [`schema`] / [`migrate`] — DDL («источник правды») и идемпотентные миграции;
//! - [`store::Store`] — контракт записи/чтения, реализуемый бэкендами;
//! - [`mem::MemStore`] — реализация в памяти (кросс-платформенно тестируемая);
//! - [`duck::DuckStore`] — нативный DuckDB за фичей `duckdb`;
//! - [`ingest`] — запись данных (бары/снимки/инструменты, классификация);
//! - [`backfill`] — планирование дозагрузки исторических баров;
//! - [`config`] — резолвинг директории данных (файловая БД истории, фаза 11).

pub mod backfill;
pub mod config;
#[cfg(feature = "duckdb")]
pub mod duck;
pub mod ingest;
pub mod mem;
pub mod migrate;
pub mod schema;
pub mod store;

pub use mem::MemStore;
pub use store::{SectorEntry, Store, TurnoverSnapshot};

#[cfg(feature = "duckdb")]
pub use duck::DuckStore;

/// Ошибки хранилища.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("ошибка БД: {0}")]
    Db(String),
    #[error("миграция не применена: {0}")]
    Migration(String),
}
