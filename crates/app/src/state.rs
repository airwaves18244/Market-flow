//! Состояние приложения, разделяемое между IPC-командами.
//!
//! Оборачивает [`storage::Store`] за `Mutex`, чтобы команды Tauri (и фоновый
//! планировщик ингеста) безопасно обращались к хранилищу из разных потоков.
//! Бэкенд абстрактный: в тестах — `MemStore`, в продакшене — `DuckStore`.

use std::sync::Mutex;

use domain::TimeFrame;
use storage::{StorageError, Store};

use crate::api;
use crate::dto::{
    BarPoint, BondIssuerDto, BreadthDto, CrossAssetSummaryDto, FlowEdgeDto, FutureGroupDto,
    InstrumentDto, OrderBookDto, ReplayStateDto, RrgSectorDto, SectorEntryDto, SectorRow,
    TimeAndSalesDto, TopMoverDto, TriggeredAlertDto, TurnoverByClassPoint, TurnoverPoint,
    YieldCurvePoint,
};

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

    pub fn breadth_data(&self, from_ts: i64, to_ts: i64) -> Result<BreadthDto, StorageError> {
        self.read(|s| api::breadth_data(s, from_ts, to_ts))
    }

    pub fn top_movers(
        &self,
        from_ts: i64,
        to_ts: i64,
        limit: Option<usize>,
    ) -> Result<Vec<TopMoverDto>, StorageError> {
        self.read(|s| api::top_movers(s, from_ts, to_ts, limit))
    }

    pub fn rrg_sectors(&self, from_ts: i64, to_ts: i64) -> Result<Vec<RrgSectorDto>, StorageError> {
        self.read(|s| api::rrg_sectors(s, from_ts, to_ts))
    }

    pub fn futures_rollup(
        &self,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<FutureGroupDto>, StorageError> {
        self.read(|s| api::futures_rollup(s, from_ts, to_ts))
    }

    pub fn bonds_rollup(
        &self,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<BondIssuerDto>, StorageError> {
        self.read(|s| api::bonds_rollup(s, from_ts, to_ts))
    }

    /// Кривая доходности не зависит от хранилища (статический scaffold),
    /// поэтому блокировка стора не нужна.
    pub fn yield_curve(&self) -> Result<Vec<YieldCurvePoint>, StorageError> {
        api::yield_curve()
    }

    pub fn cross_asset_summary(
        &self,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<CrossAssetSummaryDto, StorageError> {
        self.read(|s| api::cross_asset_summary(s, from_ts, to_ts))
    }

    pub fn turnover_timeline(
        &self,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<TurnoverByClassPoint>, StorageError> {
        self.read(|s| api::turnover_timeline(s, from_ts, to_ts))
    }

    pub fn flow_sankey(&self, from_ts: i64, to_ts: i64) -> Result<Vec<FlowEdgeDto>, StorageError> {
        self.read(|s| api::flow_sankey(s, from_ts, to_ts))
    }

    // ── Фаза 7 — live-функции ─────────────────────────────────────────────

    pub fn order_book(&self, symbol: &str, depth: usize) -> Result<OrderBookDto, StorageError> {
        self.read(|s| api::order_book(s, symbol, depth))
    }

    pub fn time_and_sales(
        &self,
        symbol: &str,
        limit: usize,
    ) -> Result<TimeAndSalesDto, StorageError> {
        self.read(|s| api::time_and_sales(s, symbol, limit))
    }

    pub fn active_alerts(&self) -> Result<Vec<TriggeredAlertDto>, StorageError> {
        self.read(api::active_alerts)
    }

    pub fn replay_state(
        &self,
        symbol: &str,
        played: usize,
    ) -> Result<ReplayStateDto, StorageError> {
        self.read(|s| api::replay_state(s, symbol, played))
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
