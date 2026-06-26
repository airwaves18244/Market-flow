//! Состояние приложения, разделяемое между IPC-командами.
//!
//! Оборачивает [`storage::Store`] за `Mutex`, чтобы команды Tauri (и фоновый
//! планировщик ингеста) безопасно обращались к хранилищу из разных потоков.
//! Бэкенд абстрактный: в тестах — `MemStore`, в продакшене — `DuckStore`.

use std::sync::Mutex;

use domain::TimeFrame;
use storage::{StorageError, Store};

use crate::api;
use crate::dto::{BarPoint, InstrumentDto, SectorEntryDto, SectorRow, TurnoverPoint};

/// Разделяемое состояние терминала.
pub struct AppState {
    store: Mutex<Box<dyn Store + Send>>,
}

impl AppState {
    /// Создать состояние поверх произвольного бэкенда хранилища.
    pub fn new(store: impl Store + Send + 'static) -> Self {
        Self {
            store: Mutex::new(Box::new(store)),
        }
    }

    /// Выполнить чтение под блокировкой. Отравленный мьютекс → ошибка БД.
    fn read<F, R>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&dyn Store) -> Result<R, StorageError>,
    {
        let guard = self
            .store
            .lock()
            .map_err(|_| StorageError::Db("state lock poisoned".into()))?;
        f(guard.as_ref())
    }

    pub fn instruments(&self) -> Result<Vec<InstrumentDto>, StorageError> {
        self.read(api::instruments)
    }

    pub fn bars(
        &self,
        symbol: &str,
        tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<BarPoint>, StorageError> {
        self.read(|s| api::bars(s, symbol, tf, from_ts, to_ts))
    }

    pub fn turnover_series(
        &self,
        symbol: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<TurnoverPoint>, StorageError> {
        self.read(|s| api::turnover_series(s, symbol, from_ts, to_ts))
    }

    pub fn sector_rollup(&self, from_ts: i64, to_ts: i64) -> Result<Vec<SectorRow>, StorageError> {
        self.read(|s| api::sector_rollup(s, from_ts, to_ts))
    }

    pub fn sector_map(&self) -> Result<Vec<SectorEntryDto>, StorageError> {
        self.read(api::sector_map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::MemStore;

    #[test]
    fn app_state_reads_through_to_store() {
        let mut store = MemStore::new();
        store.migrate().unwrap();
        let state = AppState::new(store);
        // пустое хранилище читается без паники и блокировок
        assert!(state.instruments().unwrap().is_empty());
        assert!(state.sector_rollup(0, i64::MAX).unwrap().is_empty());
    }
}
