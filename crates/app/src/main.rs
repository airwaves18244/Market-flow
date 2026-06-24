//! Точка входа десктопного терминала.
//!
//! ## Статус: каркас Phase 1
//!
//! Открывает DuckDB-хранилище, применяет схему и логирует готовность.
//! В Phase 3 здесь поднимается Tauri-приложение с IPC-командами,
//! live-push событиями и планировщиком батч-ингеста.

use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("market_terminal=info,warn")),
        )
        .init();

    tracing::info!("market terminal запускается (Phase 1)");
    tracing::info!(endpoint = finam_proto::ENDPOINT, "эндпоинт Finam Trade API");

    let db_path = PathBuf::from("market.duckdb");
    let _storage = storage::Storage::open(&db_path)?;
    tracing::info!(
        path = %db_path.display(),
        tables = storage::schema::ALL_DDL.len(),
        "DuckDB хранилище открыто, схема применена"
    );

    tracing::info!(
        classes = ?domain::AssetClass::ALL.map(|c| c.code()),
        "классы активов активны"
    );

    // TODO Phase 0 remaining: gRPC-клиент + auth (tonic, keyring, governor)
    // TODO Phase 1:           запустить планировщик ингеста
    // TODO Phase 3:           поднять Tauri-приложение

    tracing::info!("каркас завершён, выход");
    Ok(())
}
