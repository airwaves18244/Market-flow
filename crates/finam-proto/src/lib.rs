//! gRPC-стабы Finam Trade API (генерируются из vendored `.proto` в `build.rs`).
//!
//! Источник контрактов — репозиторий `FinamWeb/finam-trade-api` (новый gRPC API),
//! коммит зафиксирован в `proto/VENDOR.md`. Генерируются только **клиенты** трёх
//! сервисов (терминал read-only):
//!
//! - `AuthService` — авторизация и обновление JWT;
//! - `AssetsService` — справочники инструментов/бирж;
//! - `MarketDataService` — бары, котировки, сделки, стаканы (+ стримы).
//!
//! Эндпоинт: [`ENDPOINT`] (HTTP/2, TLS). Все сгенерированные типы доступны через
//! модуль [`pb`].

/// Сгенерированный код всех пакетов (`grpc.tradeapi.v1.*`, `google.*`, …).
/// Вложенность модулей повторяет protobuf-пакеты.
pub mod pb {
    #![allow(
        clippy::all,
        rustdoc::all,
        unreachable_pub,
        missing_docs,
        non_snake_case
    )]
    include!(concat!(env!("OUT_DIR"), "/_protos.rs"));
}

/// Базовый адрес gRPC-эндпоинта Finam Trade API (HTTP/2, TLS через `rustls`).
pub const ENDPOINT: &str = "https://trade-api.finam.ru:443";

// Удобные ре-экспорты клиентов, используемых слоем `data`.
pub use pb::grpc::tradeapi::v1::assets::assets_service_client::AssetsServiceClient;
pub use pb::grpc::tradeapi::v1::auth::auth_service_client::AuthServiceClient;
pub use pb::grpc::tradeapi::v1::marketdata::market_data_service_client::MarketDataServiceClient;

/// Пространство имён сервиса инструментов.
pub use pb::grpc::tradeapi::v1::assets;
/// Пространство имён сервиса авторизации.
pub use pb::grpc::tradeapi::v1::auth;
/// Пространство имён сервиса рыночных данных.
pub use pb::grpc::tradeapi::v1::marketdata;
/// Сторона сделки (общий тип пакета `grpc.tradeapi.v1`).
pub use pb::grpc::tradeapi::v1::Side;

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::transport::Channel;

    /// Тип-левел проверка: клиенты трёх сервисов сгенерированы и доступны.
    /// Падает на этапе компиляции, если кодоген сломался или изменились пути.
    #[test]
    fn clients_are_generated() {
        let _auth: fn() -> Option<AuthServiceClient<Channel>> = || None;
        let _assets: fn() -> Option<AssetsServiceClient<Channel>> = || None;
        let _md: fn() -> Option<MarketDataServiceClient<Channel>> = || None;
    }

    /// Несколько ключевых сообщений запросов существуют в сгенерированном коде.
    #[test]
    fn key_request_messages_exist() {
        let _ = std::mem::size_of::<auth::AuthRequest>();
        let _ = std::mem::size_of::<assets::AssetsRequest>();
        let _ = std::mem::size_of::<marketdata::BarsRequest>();
    }
}
