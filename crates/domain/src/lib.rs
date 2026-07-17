//! Доменная модель и аналитика рыночного терминала.
//!
//! Этот крейт — «ядро»: он содержит чистые типы и математику расчёта рыночных
//! метрик (оборот, денежные потоки, ширину рынка, секторную ротацию,
//! кросс-актив агрегаты). Он **не зависит** от gRPC (`data`), хранилища
//! (`storage`) и UI (`app`/`tauri`), поэтому собирается и тестируется
//! кросс-платформенно, в том числе в CI на Linux.
//!
//! Адаптеры переводят сырые ответы Finam Trade API в типы из [`model`],
//! затем вызывают функции из [`metrics`], а результат сериализуют во фронт.

pub mod algo;
pub mod backtest;
pub mod calendar;
pub mod delta;
pub mod history;
pub mod keyactivity;
pub mod metrics;
pub mod model;
pub mod options;
pub mod trading;

pub use model::{AssetClass, Bar, BookLevel, Instrument, OrderBook, Quote, Side, TimeFrame, Trade};
