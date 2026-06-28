//! gRPC-стабы Finam Trade API.
//!
//! Клиентские стабы генерируются из vendored `.proto` (каталог `proto/`,
//! санитизированные копии из `FinamWeb/finam-trade-api`) крейтом `tonic-build`
//! с `protoc` из `protoc-bin-vendored` — **за фичей `grpc`**. По умолчанию
//! (кросс-платформенный CI) фича выключена, и крейт остаётся лёгкой заглушкой
//! без зависимостей: тяжёлые `tonic`/`prost` и build-tooling не подтягиваются.
//!
//! Эндпоинт: `https://trade-api.finam.ru:443` (HTTP/2, TLS).
//! Сервисы: `AuthService` (готов), `AssetsService`, `MarketDataService`
//! (стабы добавляются в фазе интеграции рыночных данных).

/// Базовый адрес gRPC-эндпоинта Finam Trade API.
pub const ENDPOINT: &str = "https://trade-api.finam.ru:443";

/// Хост gRPC-эндпоинта (для проверки TLS-домена и метрик).
pub const HOST: &str = "trade-api.finam.ru";

/// Сгенерированные стабы `grpc.tradeapi.v1.auth` (`AuthService` и сообщения).
///
/// Доступен под фичей `grpc`. Содержит, в частности,
/// `auth_service_client::AuthServiceClient`, `AuthRequest`, `AuthResponse`.
#[cfg(feature = "grpc")]
pub mod auth {
    tonic::include_proto!("grpc.tradeapi.v1.auth");
}
