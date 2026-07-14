//! Клиент MOEX ALGOPACK (фича `moex`, поверх фичи `http`).
//!
//! Реализует контракт `SPEC_0-12.md` `10.1`/`10.9.2`: транспорт поверх
//! [`crate::http::HttpClient`] с Bearer-заголовком ([`client`]), чистый
//! парсер ISS JSON (`columns`+`data` → строки, мягкий маппинг полей —
//! [`parse`]) и трейт-контракт источника данных для оркестрации в других
//! слоях ([`source`]).
//!
//! Подмодули:
//! - [`parse`] — чистый разбор ISS JSON (без сети), тестируется на офлайн-
//!   фикстурах (`crates/data/tests/fixtures/moex/`);
//! - [`client`] — [`MoexAlgo`]: методы `tradestats`/`orderstats`/`obstats`/
//!   `hi2`/`futoi`/`candles`, пагинация курсором, Bearer-авторизация;
//! - [`source`] — [`AlgoSource`] + [`FakeAlgoSource`] для тестов оркестрации.
//!
//! Контракт API (база, авторизация, датасеты) — `SPEC_0-12.md` `10.0`; форма
//! параметров запроса и пагинации помечена `(unverified)` — см.
//! `crates/data/tests/fixtures/moex/README.md`.

pub mod client;
pub mod parse;
pub mod source;

pub use client::{DateRange, Market, MoexAlgo, DEFAULT_BASE_URL};
pub use parse::{IssCandle, IssCursor, IssTable, RowView};
pub use source::{AlgoSource, FakeAlgoSource};
