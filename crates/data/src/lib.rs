//! Адаптер Finam Trade API → доменные типы.
//!
//! Этот крейт изолирует всё «грязное»: gRPC-вызовы, авторизацию с refresh
//! токена, ограничение частоты запросов (≤200/мин на метод), переподключение
//! стримов (обрыв ~раз в 24 ч) и классификацию инструментов по секторам.
//! Наружу он отдаёт уже доменные типы из крейта `domain`.
//!
//! ## Состав
//!
//! - [`MarketData`] — контракт источника рыночных данных (трейт);
//! - [`client::FinamClient`] — его gRPC-реализация (транспорт, auth+refresh,
//!   rate-limit), переводящая ответы API в доменные типы;
//! - [`convert`] — чистые преобразования protobuf → домен (приватный);
//! - [`auth`], [`ratelimit`], [`resilience`], [`secret`] — поддерживающая логика;
//! - [`classify`] — классификация инструментов по секторам.

pub mod auth;
pub mod classify;
pub mod client;
mod convert;
pub mod ratelimit;
pub mod resilience;
pub mod secret;

pub use client::FinamClient;

use domain::{Bar, Instrument, Quote, Trade};

/// Ошибки слоя данных.
#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("ошибка авторизации: {0}")]
    Auth(String),
    #[error("превышен лимит запросов по методу {0}")]
    RateLimited(&'static str),
    #[error("транспорт/сеть: {0}")]
    Transport(String),
    #[error("API недоступен (техническое окно 05:00–06:15 MSK)")]
    MaintenanceWindow,
    #[error("прочее: {0}")]
    Other(String),
}

/// Тайм-фрейм бара (соответствует `TimeFrame` в API Finam).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeFrame {
    M1,
    M5,
    M15,
    H1,
    D1,
}

/// Источник рыночных данных. Реальная реализация — gRPC-клиент Finam.
///
/// Методы асинхронные; реализация обязана уважать per-method rate-limit и
/// прозрачно обновлять токен авторизации.
pub trait MarketData {
    /// Список инструментов биржи (`AssetsService.Assets`).
    fn assets(
        &self,
        mic: &str,
    ) -> impl std::future::Future<Output = Result<Vec<Instrument>, DataError>> + Send;

    /// Исторические бары инструмента (`MarketDataService.Bars`).
    fn bars(
        &self,
        symbol: &str,
        tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
    ) -> impl std::future::Future<Output = Result<Vec<Bar>, DataError>> + Send;

    /// Последняя котировка (`MarketDataService.LastQuote`).
    fn last_quote(
        &self,
        symbol: &str,
    ) -> impl std::future::Future<Output = Result<Quote, DataError>> + Send;

    /// Последние сделки (`MarketDataService.LatestTrades`).
    fn latest_trades(
        &self,
        symbol: &str,
    ) -> impl std::future::Future<Output = Result<Vec<Trade>, DataError>> + Send;
}
