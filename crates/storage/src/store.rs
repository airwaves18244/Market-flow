//! Контракт хранилища: что умеет персистентный слой терминала.
//!
//! [`Store`] — единый интерфейс для записи (ингест) и чтения (запросы,
//! питающие аналитику из `domain`). У него две реализации:
//! - [`crate::mem::MemStore`] — в памяти, всегда доступна, целевая для тестов и
//!   CI на любой платформе;
//! - [`crate::duck::DuckStore`] — на нативном DuckDB (фича `duckdb`).
//!
//! Слой намеренно синхронный и не знает про gRPC/tokio: сетевой источник
//! (`data::MarketData`) и асинхронный планировщик живут выше, в `app`. Сюда
//! приходят уже доменные типы.

use domain::{Bar, Instrument, TimeFrame};
use serde::{Deserialize, Serialize};

use crate::StorageError;

/// Снимок агрегированного оборота инструмента на момент сканирования рынка
/// (строка таблицы `turnover_snapshots`). Из последовательности снимков
/// строятся тренды и перетоки во времени.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TurnoverSnapshot {
    /// Момент снимка, UNIX-секунды UTC.
    pub ts: i64,
    /// Накопленный оборот за день (денежный объём).
    pub turnover: f64,
    /// Чистый денежный поток (направленный оборот вверх − вниз).
    pub net_flow: f64,
    /// Дневное изменение в долях (`0.01` = +1%).
    pub change: f64,
}

/// Запись таблицы классификации секторов (`sector_map`).
///
/// Хранилище держит её в «сыром» виде (ключ + сектор + признак ISIN);
/// построение поисковой структуры `data::classify::SectorMap` — задача
/// адаптера `data`, чтобы хранилище не зависело от сетевого слоя.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectorEntry {
    /// Тикер или ISIN.
    pub key: String,
    /// Название сектора.
    pub sector: String,
    /// `true`, если ключ — ISIN (а не тикер).
    pub is_isin: bool,
}

/// Персистентный слой: ингест рыночных данных и аналитические запросы.
///
/// Записи идемпотентны по первичным ключам схемы (`INSERT OR REPLACE`):
/// повторный ингест того же бара/снимка не плодит дублей.
pub trait Store {
    /// Применить миграции (создать таблицы, зафиксировать версию схемы).
    /// Идемпотентна — безопасно вызывать при каждом старте.
    fn migrate(&mut self) -> Result<(), StorageError>;

    /// Текущая версия схемы (`None`, если БД ещё не мигрировали).
    fn schema_version(&self) -> Result<Option<i32>, StorageError>;

    /// Вставить/обновить инструменты справочника. Возвращает число строк.
    fn upsert_instruments(&mut self, items: &[Instrument]) -> Result<usize, StorageError>;

    /// Все инструменты справочника (порядок не гарантируется).
    fn instruments(&self) -> Result<Vec<Instrument>, StorageError>;

    /// Вставить бары инструмента в заданном тайм-фрейме. Возвращает число строк.
    fn insert_bars(
        &mut self,
        symbol: &str,
        tf: TimeFrame,
        bars: &[Bar],
    ) -> Result<usize, StorageError>;

    /// Бары инструмента в `[from_ts, to_ts]` (включительно), по возрастанию `ts`.
    fn bars(
        &self,
        symbol: &str,
        tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<Bar>, StorageError>;

    /// Время последнего сохранённого бара (для планирования бэкфилла).
    fn last_bar_ts(&self, symbol: &str, tf: TimeFrame) -> Result<Option<i64>, StorageError>;

    /// Вставить снимок оборота инструмента.
    fn insert_snapshot(
        &mut self,
        symbol: &str,
        snap: &TurnoverSnapshot,
    ) -> Result<(), StorageError>;

    /// Снимки оборота инструмента в `[from_ts, to_ts]`, по возрастанию `ts`.
    fn snapshots(
        &self,
        symbol: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<TurnoverSnapshot>, StorageError>;

    /// Заменить/дополнить таблицу классификации секторов. Возвращает число строк.
    fn upsert_sector_map(&mut self, entries: &[SectorEntry]) -> Result<usize, StorageError>;

    /// Все записи классификации секторов.
    fn sector_map(&self) -> Result<Vec<SectorEntry>, StorageError>;

    /// Все инструменты заданного класса активов.
    fn instruments_by_asset_class(
        &self,
        asset_class: &str,
    ) -> Result<Vec<Instrument>, StorageError> {
        let all = self.instruments()?;
        Ok(all
            .into_iter()
            .filter(|i| i.asset_class.code() == asset_class)
            .collect())
    }
}
