//! Кодоген gRPC-стабов из vendored `.proto`.
//!
//! Активен только под фичей `grpc`. Использует `protoc` из `protoc-bin-vendored`
//! (не требует системного protoc), генерирует только клиентскую сторону
//! (терминал read-only — серверные стабы не нужны). Все пакеты собираются в один
//! модульный файл (`include_file`), чтобы prost корректно разложил вложенность
//! пакетов и кросс-пакетные ссылки (`grpc.tradeapi.v1.*`, `google.type.*`).

fn main() {
    #[cfg(feature = "grpc")]
    {
        // protoc из vendored-бинарника: кросс-платформенно, без системной установки.
        let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc недоступен");
        std::env::set_var("PROTOC", protoc);

        let protos = [
            "proto/grpc/tradeapi/v1/auth/auth_service.proto",
            "proto/grpc/tradeapi/v1/assets/assets_service.proto",
            "proto/grpc/tradeapi/v1/marketdata/marketdata_service.proto",
            "proto/grpc/tradeapi/v1/side.proto",
        ];
        let includes = ["proto"];

        tonic_build::configure()
            .build_server(false)
            .include_file("finam.rs")
            .compile_protos(&protos, &includes)
            .expect("не удалось сгенерировать gRPC-стабы из .proto");

        for p in protos {
            println!("cargo:rerun-if-changed={p}");
        }
    }
}
