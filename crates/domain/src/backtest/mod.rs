//! Бэктестер стратегий (в духе MultiCharts): проигрывает исторические бары
//! через стратегию и выдаёт кривую капитала, список сделок и метрики.
//!
//! Слои:
//! - [`strategy`] — контракт `Strategy`, контекст бара и сигнал (целевая позиция);
//! - [`engine`] — детерминированный прогон с учётом позиции/комиссии/слиппеджа;
//! - [`report`] — сделки, кривая капитала и метрики эффективности;
//! - [`library`] — встроенные стратегии («известные сценарии») + фабрика по id.
//!
//! Всё чисто и тестируемо без сети/диска — как и остальной `domain`.

pub mod engine;
pub mod library;
pub mod report;
pub mod strategy;

pub use engine::{run_backtest, BacktestConfig, FillTiming};
pub use library::{descriptors, strategy_from_id, ParamSpec, StrategyDescriptor};
pub use report::{BacktestReport, PerfMetrics, SimTrade};
pub use strategy::{BarContext, Signal, Strategy, StrategyParams};
