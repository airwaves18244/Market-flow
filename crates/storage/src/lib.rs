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
//! - [`history`] — планирование инкрементальной дозагрузки истории (фаза 11.2);
//! - [`datadir`] — резолвер директории данных (файл БД, Parquet-экспорты).

pub mod backfill;
pub mod datadir;
#[cfg(feature = "duckdb")]
pub mod duck;
pub mod history;
pub mod ingest;
pub mod mem;
pub mod migrate;
pub mod schema;
pub mod store;

pub use datadir::{default_data_dir, default_db_path, resolve_data_dir_with, DATA_DIR_ENV};
pub use history::{covered_ranges, plan_from_catalog, plan_history_fetch};
pub use mem::MemStore;
pub use store::{HistoryDatasetRecord, SectorEntry, Store, TurnoverSnapshot};

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
