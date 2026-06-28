//! Кодоген gRPC-стабов из vendored `.proto`.
//!
//! Активен только под фичей `grpc`. Использует `protoc` из `protoc-bin-vendored`
//! (не требует системного protoc), генерирует только клиентскую сторону
//! (терминал read-only — серверные стабы не нужны).

fn main() {
    #[cfg(feature = "grpc")]
    {
        // protoc из vendored-бинарника: кросс-платформенно, без системной установки.
        let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc недоступен");
        std::env::set_var("PROTOC", protoc);

        let protos = ["proto/grpc/tradeapi/v1/auth/auth_service.proto"];
        let includes = ["proto"];

        tonic_build::configure()
            .build_server(false)
            .compile_protos(&protos, &includes)
            .expect("не удалось сгенерировать gRPC-стабы из .proto");

        for p in protos {
            println!("cargo:rerun-if-changed={p}");
        }
    }
}
