# Vendored protobuf — Finam Trade API

Эти `.proto` скопированы без изменений из официального репозитория нового
gRPC API Finam:

- **Источник:** https://github.com/FinamWeb/finam-trade-api
- **Путь в источнике:** `proto/`
- **Зафиксированный коммит:** `3cec0896fa19936b164fb15561515f63e2e72df1`

## Что используется

Код генерируется в `build.rs` (через `protox` + `tonic-prost-build`) только для
клиентов трёх сервисов (терминал read-only):

- `grpc/tradeapi/v1/auth/auth_service.proto` — `AuthService`
- `grpc/tradeapi/v1/assets/assets_service.proto` — `AssetsService`
- `grpc/tradeapi/v1/marketdata/marketdata_service.proto` — `MarketDataService`

Их транзитивные импорты (`google/api`, `google/type`, `google/protobuf`,
`grpc/gateway/...`, `grpc/tradeapi/v1/side.proto`, `.../trade.proto`) включены в
дерево и резолвятся компилятором автоматически.

## Обновление

1. Скачать `proto/` нужного коммита из источника, заменить содержимое каталога.
2. Обновить коммит выше.
3. `cargo build -p finam-proto` — проверить генерацию.
