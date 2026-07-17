//! gRPC-стабы Finam Trade API.
//!
//! Клиентские стабы генерируются из vendored `.proto` (каталог `proto/`,
//! санитизированные копии из `FinamWeb/finam-trade-api`) крейтом `tonic-build`
//! с `protoc` из `protoc-bin-vendored` — **за фичей `grpc`**. По умолчанию
//! (кросс-платформенный CI) фича выключена, и крейт остаётся лёгкой заглушкой
//! без зависимостей: тяжёлые `tonic`/`prost` и build-tooling не подтягиваются.
//!
//! Эндпоинт: `https://tradeapi.finam.ru:443` (HTTP/2, TLS). Прежний хост
//! `trade-api.finam.ru` отвечает `301 → tradeapi.finam.ru`, а gRPC-клиенты
//! редиректы не следуют (живой смоук T14, 2026-07-17).
//! Сервисы: `AuthService`, `AssetsService`, `MarketDataService` (read-only).

/// Базовый адрес gRPC-эндпоинта Finam Trade API.
pub const ENDPOINT: &str = "https://tradeapi.finam.ru:443";

/// Хост gRPC-эндпоинта (для проверки TLS-домена и метрик).
pub const HOST: &str = "tradeapi.finam.ru";

/// Весь сгенерированный код (один модульный файл с вложенностью пакетов).
///
/// Доступен под фичей `grpc`. Содержит пакеты `grpc.tradeapi.v1.*` и
/// `google.type.*`; `google.protobuf.*` отображены на `prost_types`.
#[cfg(feature = "grpc")]
#[allow(clippy::all, clippy::pedantic, clippy::nursery, rustdoc::all)]
#[rustfmt::skip]
pub mod pb {
    include!(concat!(env!("OUT_DIR"), "/finam.rs"));
}

/// `grpc.tradeapi.v1.auth` — `AuthService` и сообщения.
///
/// Содержит `auth_service_client::AuthServiceClient`, `AuthRequest`,
/// `AuthResponse`, `TokenDetailsRequest`/`TokenDetailsResponse`.
#[cfg(feature = "grpc")]
pub use pb::grpc::tradeapi::v1::auth;

/// `grpc.tradeapi.v1.assets` — `AssetsService` и сообщения.
#[cfg(feature = "grpc")]
pub use pb::grpc::tradeapi::v1::assets;

/// `grpc.tradeapi.v1.marketdata` — `MarketDataService` и сообщения.
#[cfg(feature = "grpc")]
pub use pb::grpc::tradeapi::v1::marketdata;
