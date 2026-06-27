//! Адаптер Finam Trade API → доменные типы.
//!
//! Этот крейт изолирует всё «грязное»: gRPC-вызовы, авторизацию с refresh
//! токена, ограничение частоты запросов (≤200/мин на метод), переподключение
//! стримов (обрыв ~раз в 24 ч) и классификацию инструментов по секторам.
//! Наружу он отдаёт уже доменные типы из крейта `domain`.
//!
//! ## Статус: интерфейсы (Фаза 0)
//!
//! Сетевые реализации подключаются в фазе интеграции API; здесь определены
//! контракты (трейты/типы), на которые опираются `storage` и `app`.

pub mod classify;
pub mod rate;

pub use rate::RateLimiter;

use domain::{Bar, Instrument, Quote, Trade};

/// Тайм-фрейм бара. Тип живёт в `domain` (чистое доменное значение);
/// здесь он переэкспортирован, чтобы сетевой слой Finam и хранилище
/// пользовались одним и тем же типом.
pub use domain::TimeFrame;

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
