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

use crate::cancel::CancelFlag;
use crate::ingest::{IngestConfig, IngestService};
use crate::state::AppState;

/// Достать API-секрет: сперва переменная окружения `FINAM_API_SECRET`, затем
/// файл `.env` (рядом с рабочим каталогом или выше; ключи `FINAM_API_SECRET`/
/// `FINAM_SECRET`, без учёта регистра), затем ОС-keyring (фича `keyring`).
/// Возвращает понятную ошибку, если секрет нигде не задан.
pub fn load_secret() -> Result<String, String> {
    if let Ok(s) = std::env::var(data::SECRET_ENV_VAR) {
        if !s.trim().is_empty() {
            return Ok(s.trim().to_owned());
        }
    }
    // Файл `.env` (в `.gitignore`): ищем начиная с текущего каталога вверх.
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(s) = data::find_dotenv_secret(&cwd, 4) {
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
        "API-секрет не задан: установите переменную окружения FINAM_API_SECRET, \
         положите его в файл .env (FINAM_API_SECRET=… или FINAM_SECRET=…) \
         или сохраните в ОС-keyring (сборка с фичей `keyring`)"
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
    // CLI-вход пока не даёт способа отменить извне (нет обработчика сигнала) —
    // цикл крутится до убийства процесса, как и раньше. Флаг заведён здесь,
    // чтобы `IngestService::run` был единообразно отменяем со своим ALGOPACK-
    // аналогом ([`run_algo`]); подключение к Ctrl+C/Tauri-lifecycle — отдельная
    // задача интеграции.
    svc.run(CancelFlag::new()).await;
    Ok(())
}

// ── Фаза 10.6.4 — ингест ALGOPACK (фича `moex`) ───────────────────────────────
//
// В отличие от `run` (Finam bars, вызывается из `main()` по CLI-аргументу),
// у `run_algo` пока нет собственного CLI-входа — вотчлист/рынок ALGOPACK
// приходят из настроек паспорта MOEX ALGO (10.8.1), UI для которых уже есть,
// а связка «настройки → запуск планировщика» — задача следующей интеграции.
// Публичное API уже готово и протестировано (`AlgoIngestService` на фейке);
// глушим dead_code, как у `ingest`/`replay`.

/// Переменная окружения с токеном MOEX ALGOPACK (`Authorization: Bearer`).
#[cfg(feature = "moex")]
#[allow(dead_code)]
pub const ALGO_TOKEN_ENV_VAR: &str = "MOEX_ALGOPACK_TOKEN";

/// Достать токен ALGOPACK: переменная окружения [`ALGO_TOKEN_ENV_VAR`], затем
/// файл `.env` (тот же поиск вверх по дереву каталогов, что и у
/// [`load_secret`]; ключи `MOEX_ALGOPACK_TOKEN`/`ALGOPACK_TOKEN`, без учёта
/// регистра). Отдельный секрет от Finam API — разные хосты/авторизация
/// (10.0.1: `Authorization: Bearer` на `apim.moex.com`, без общего с Finam
/// gRPC-секретом смысла).
#[cfg(feature = "moex")]
#[allow(dead_code)]
pub fn load_algo_token() -> Result<String, String> {
    if let Ok(s) = std::env::var(ALGO_TOKEN_ENV_VAR) {
        if !s.trim().is_empty() {
            return Ok(s.trim().to_owned());
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let keys = ["MOEX_ALGOPACK_TOKEN", "ALGOPACK_TOKEN"];
        if let Some(s) = data::dotenv::find_dotenv_value(&cwd, 4, &keys) {
            return Ok(s);
        }
    }
    Err(
        "токен ALGOPACK не задан: установите переменную окружения MOEX_ALGOPACK_TOKEN \
         или положите его в файл .env (MOEX_ALGOPACK_TOKEN=…)"
            .to_owned(),
    )
}

/// Боевой цикл ингеста ALGOPACK: авторизация по токену → планировщик по
/// вотчлисту `symbols` на рынке `config.market`. Отдельный вход от
/// [`run`] — не требует Finam-секрета/справочника (символы задаёт вызывающая
/// сторона, обычно watchlist из настроек паспорта MOEX ALGO, 10.8.1).
#[cfg(feature = "moex")]
#[allow(dead_code)]
pub async fn run_algo(
    state: Arc<AppState>,
    symbols: Vec<String>,
    config: crate::algo_ingest::AlgoIngestConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let token = load_algo_token()?;
    let transport = data::ReqwestTransport::new()?;
    let client = data::MoexAlgo::new(transport, token);

    tracing::info!(
        market = config.market.code(),
        symbols = symbols.len(),
        "live: запуск планировщика ингеста ALGOPACK"
    );
    let svc = crate::algo_ingest::AlgoIngestService::new(client, state, symbols, config);
    // См. комментарий в `run` — CLI-вход пока не подключает отмену к
    // внешнему сигналу, но `AlgoIngestService::run` уже умеет её принимать.
    svc.run(CancelFlag::new()).await;
    Ok(())
}
