//! Генерация gRPC-стабов Finam Trade API из vendored `.proto`.
//!
//! Используем `protox` (чистый Rust) вместо системного `protoc`, поэтому сборка
//! не требует внешних бинарей. Генерируем только клиентов (терминал read-only —
//! серверная часть не нужна) для трёх сервисов: Auth, Assets, MarketData.
//! Их транзитивные импорты (`google/*`, `grpc-gateway`, `side.proto`) подтянутся
//! из дерева `proto/` автоматически.

use std::path::PathBuf;

fn main() {
    let proto_root = "proto";
    let services = [
        "proto/grpc/tradeapi/v1/auth/auth_service.proto",
        "proto/grpc/tradeapi/v1/assets/assets_service.proto",
        "proto/grpc/tradeapi/v1/marketdata/marketdata_service.proto",
    ];

    // protox компилирует протофайлы в FileDescriptorSet (с резолвом импортов).
    let file_descriptors =
        protox::compile(services, [proto_root]).expect("protox: компиляция .proto");

    // Один include-файл с вложенными модулями по пакетам — корректные
    // перекрёстные ссылки между пакетами (marketdata → grpc.tradeapi.v1.Side).
    let mut config = prost_build::Config::new();
    config.include_file("_protos.rs");

    tonic_prost_build::configure()
        .build_server(false)
        .build_client(true)
        .compile_fds_with_config(file_descriptors, config)
        .expect("tonic-prost-build: генерация клиентов");

    // Доступ к OUT_DIR подтверждает, что переменная задана (там лежит код).
    let _out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    println!("cargo:rerun-if-changed=proto");
    println!("cargo:rerun-if-changed=build.rs");
}
