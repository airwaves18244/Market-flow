//! gRPC-стабы Finam Trade API.
//!
//! ## Статус: заглушка (Фаза 0)
//!
//! В фазе подключения API этот крейт будет генерировать клиентов и сообщения из
//! официальных `.proto` (репозиторий `FinamWeb/trade-api-docs`) с помощью
//! `tonic-build` в `build.rs`, а затем включать сгенерированный код, например:
//!
//! ```ignore
//! tonic::include_proto!("grpc.tradeapi.v1.marketdata");
//! tonic::include_proto!("grpc.tradeapi.v1.assets");
//! ```
//!
//! Эндпоинт: `https://trade-api.finam.ru:443` (HTTP/2, TLS через `rustls`).
//! Сервисы: `AuthService`, `AssetsService`, `MarketDataService`,
//! `AccountsService` (последний в v1 не используется — терминал read-only).

/// Базовый адрес gRPC-эндпоинта Finam Trade API.
pub const ENDPOINT: &str = "https://trade-api.finam.ru:443";
