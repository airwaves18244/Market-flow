//! Боевой запуск: live-подключение к Finam Trade API (фича `live`).
//!
//! Связывает реальный gRPC-источник (`data::FinamMarketData`) с хранилищем и
//! планировщиком ингеста ([`crate::ingest`]): авторизуется по секрету, тянет
//! справочник инструментов, наполняет `Store` и крутит цикл опроса баров.
//!
//! Секрет берётся из переменной окружения `FINAM_API_SECRET`, а при сборке с
//! фичей `keyring` — из ОС-keyring. Он **не** попадает в репозиторий и логи.
//!
//! Требования к окружению:
//! - доступ к `trade-api.finam.ru:443` (в Claude Code on the web — добавить хост
//!   в network egress allowlist окружения);
//! - валидный `FINAM_API_SECRET` (или секрет в keyring).

use std::sync::Arc;

use data::{AuthManager, FinamMarketData, GrpcAuthTransport, MarketData, MemSecretStore};
use storage::{MemStore, Store};

use crate::ingest::{IngestConfig, IngestService};
use crate::state::AppState;

/// Достать API-секрет: сперва `FINAM_API_SECRET`, затем ОС-keyring (фича
/// `keyring`). Возвращает понятную ошибку, если секрет нигде не задан.
pub fn load_secret() -> Result<String, String> {
    if let Ok(s) = std::env::var("FINAM_API_SECRET") {
        if !s.trim().is_empty() {
            return Ok(s);
        }
    }
    #[cfg(feature = "keyring")]
    {
        use data::{KeyringSecretStore, SecretStore};
        if let Ok(Some(s)) = KeyringSecretStore::new().load() {
            if !s.trim().is_empty() {
                return Ok(s);
            }
        }
    }
    Err(
        "API-секрет не задан: установите переменную окружения FINAM_API_SECRET \
         (или сохраните секрет в ОС-keyring при сборке с фичей `keyring`)"
            .to_owned(),
    )
}

/// Сохранить секрет из `FINAM_API_SECRET` в ОС-keyring (одноразовая настройка).
#[cfg(feature = "keyring")]
pub fn store_secret_from_env() -> Result<(), Box<dyn std::error::Error>> {
    use data::{KeyringSecretStore, SecretStore};
    let secret = std::env::var("FINAM_API_SECRET")
        .map_err(|_| "FINAM_API_SECRET не задан — нечего сохранять в keyring")?;
    if secret.trim().is_empty() {
        return Err("FINAM_API_SECRET пуст".into());
    }
    KeyringSecretStore::new().store(&secret)?;
    println!("Секрет сохранён в ОС-keyring (market-terminal/finam-api-secret).");
    Ok(())
}

/// Боевой цикл: авторизация → справочник → ингест баров вотчлиста.
///
/// `mic` — биржа для справочника (например, `MISX`). Бесконечный цикл ингеста
/// завершается только при остановке процесса.
pub async fn run(mic: &str) -> Result<(), Box<dyn std::error::Error>> {
    let secret = load_secret()?;
    let auth = AuthManager::new(
        GrpcAuthTransport::new(),
        MemSecretStore::with_secret(secret),
    );
    let md = FinamMarketData::connect(auth)?;

    tracing::info!(
        mic,
        endpoint = finam_proto::ENDPOINT,
        "live: запрос справочника"
    );
    let instruments = md.assets(mic).await?;
    tracing::info!(count = instruments.len(), "live: справочник получен");
    if instruments.is_empty() {
        return Err(format!("по бирже {mic} не получено инструментов").into());
    }

    let mut store = MemStore::new();
    store.migrate()?;
    store.upsert_instruments(&instruments)?;
    let state = Arc::new(AppState::new(store));

    let symbols: Vec<String> = instruments.iter().map(|i| i.symbol.clone()).collect();
    tracing::info!(symbols = symbols.len(), "live: запуск планировщика ингеста");
    let svc = IngestService::new(md, Arc::clone(&state), symbols, IngestConfig::default());
    svc.run().await;
    Ok(())
}
