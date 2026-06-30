//! Состояние приложения, разделяемое между IPC-командами.
//!
//! Оборачивает [`storage::Store`] за `Mutex`, чтобы команды Tauri (и фоновый
//! планировщик ингеста) безопасно обращались к хранилищу из разных потоков.
//! Бэкенд абстрактный: в тестах — `MemStore`, в продакшене — `DuckStore`.

use std::sync::Mutex;

#[cfg(feature = "ingest")]
use domain::Bar;
use domain::TimeFrame;
#[cfg(feature = "ingest")]
use storage::ingest::snapshot_from_bars;
use storage::{StorageError, Store};

use domain::backtest::StrategyParams;

use crate::api;
use crate::dto::{
    AccountDto, AlertEventDto, AlertRuleInput, BacktestConfigInput, BacktestReportDto, BarPoint,
    BondIssuerDto, BreadthDto, CrossAssetSummaryDto, FlowEdgeDto, FootprintBarDto, FutureGroupDto,
    InstrumentDto, OrderDto, OrderInput, PositionDto, RobotConfigInput, RobotSignalDto,
    RrgSectorDto, SectorEntryDto, SectorRow, StrategyDescriptorDto, SubmitResultDto, TopMoverDto,
    TurnoverByClassPoint, TurnoverPoint, YieldCurvePoint,
};
use crate::trade::TradeSession;

/// Разделяемое состояние терминала.
pub struct AppState {
    store: Mutex<Box<dyn Store + Send>>,
    /// Сессия симулированной торговли (paper trading).
    trade: TradeSession,
}

// В headless-live режиме IPC-read-методы (обработчики команд) не вызываются —
// их потребляет Tauri-UI и тесты. Глушим dead_code только для этой комбинации.
#[cfg_attr(feature = "live", allow(dead_code))]
impl AppState {
    /// Создать состояние поверх произвольного бэкенда хранилища.
    pub fn new(store: impl Store + Send + 'static) -> Self {
        Self {
            store: Mutex::new(Box::new(store)),
            trade: TradeSession::new(),
        }
    }

    /// Доступ к сессии торговли (для live-эмиттеров `on_trade`/`on_book`).
    pub fn trade_session(&self) -> &TradeSession {
        &self.trade
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

    /// Выполнить запись под блокировкой. Отравленный мьютекс → ошибка БД.
    #[cfg(feature = "ingest")]
    fn write<F, R>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&mut dyn Store) -> Result<R, StorageError>,
    {
        let mut guard = self
            .store
            .lock()
            .map_err(|_| StorageError::Db("state lock poisoned".into()))?;
        f(guard.as_mut())
    }

    /// Записать бары инструмента и построить снимок оборота на `snapshot_ts`.
    ///
    /// Точка входа планировщика ингеста ([`crate::ingest`]). Идемпотентно по
    /// ключам схемы (повторный ингест не плодит дублей). Пустая серия — no-op;
    /// снимок пишется только для непустой серии.
    #[cfg(feature = "ingest")]
    pub fn ingest_bars(
        &self,
        symbol: &str,
        tf: TimeFrame,
        bars: &[Bar],
        snapshot_ts: i64,
    ) -> Result<(), StorageError> {
        if bars.is_empty() {
            return Ok(());
        }
        self.write(|s| {
            s.insert_bars(symbol, tf, bars)?;
            if let Some(snap) = snapshot_from_bars(bars, snapshot_ts) {
                s.insert_snapshot(symbol, &snap)?;
            }
            Ok(())
        })
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

    /// Прогон правил алёртов по сохранённым барам (replay-проверка правил).
    pub fn alerts_scan(
        &self,
        rules: &[AlertRuleInput],
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<AlertEventDto>, StorageError> {
        self.read(|s| api::alerts_scan(s, rules, from_ts, to_ts))
    }

    // ── V2 / Бэктестер ──────────────────────────────────────────────────────

    /// Каталог встроенных стратегий бэктестера (не зависит от хранилища).
    pub fn list_strategies(&self) -> Vec<StrategyDescriptorDto> {
        api::list_strategies()
    }

    /// Прогон бэктеста стратегии по сохранённым барам инструмента.
    #[allow(clippy::too_many_arguments)]
    pub fn run_backtest(
        &self,
        symbol: &str,
        tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
        strategy_id: &str,
        params: &StrategyParams,
        config: &BacktestConfigInput,
    ) -> Result<BacktestReportDto, StorageError> {
        self.read(|s| api::run_backtest(s, symbol, tf, from_ts, to_ts, strategy_id, params, config))
    }

    // ── V2 / Delta ──────────────────────────────────────────────────────────

    /// Footprint/дельта инструмента по сохранённым тиковым сделкам.
    pub fn delta_footprint(
        &self,
        symbol: &str,
        tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
        tick_size: f64,
    ) -> Result<Vec<FootprintBarDto>, StorageError> {
        self.read(|s| api::delta_footprint(s, symbol, tf, from_ts, to_ts, tick_size))
    }

    /// Прогон детектирующих роботов по сохранённой ленте инструмента.
    pub fn robot_scan(
        &self,
        symbol: &str,
        from_ts: i64,
        to_ts: i64,
        config: &RobotConfigInput,
    ) -> Result<Vec<RobotSignalDto>, StorageError> {
        self.read(|s| api::robot_scan(s, symbol, from_ts, to_ts, config))
    }

    // ── V2 / Trade (симулятор исполнения) ────────────────────────────────────

    /// Поставить заявку в симулятор. `Err` — причина отклонения/ошибки ввода.
    pub fn submit_order(&self, input: &OrderInput) -> Result<SubmitResultDto, String> {
        self.trade.submit(input)
    }

    /// Отменить активную заявку.
    pub fn cancel_order(&self, id: u64) -> Result<OrderDto, String> {
        self.trade.cancel(id)
    }

    /// Активные заявки (блоттер).
    pub fn order_blotter(&self) -> Vec<OrderDto> {
        self.trade.orders()
    }

    /// Открытые позиции.
    pub fn positions(&self) -> Vec<PositionDto> {
        self.trade.positions()
    }

    /// Состояние счёта.
    pub fn account(&self) -> AccountDto {
        self.trade.account()
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
