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
//! - доступ к `tradeapi.finam.ru:443` (в Claude Code on the web — добавить хост
//!   в network egress allowlist окружения);
//! - валидный `FINAM_API_SECRET` (или секрет в keyring).

use std::borrow::Borrow;
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
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                // R-9: тримим — случайный пробел/перевод строки в keyring иначе
                // уехал бы в заголовок авторизации и сломал бы обмен токена.
                return Ok(trimmed.to_owned());
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
    let secret = secret.trim();
    if secret.is_empty() {
        return Err("FINAM_API_SECRET пуст".into());
    }
    // R-9: сохраняем уже тримленным, чтобы `load_secret` отдавал секрет без
    // паразитных пробелов независимо от того, откуда он потом читается.
    KeyringSecretStore::new().store(secret)?;
    println!("Секрет сохранён в ОС-keyring (market-terminal/finam-api-secret).");
    Ok(())
}

/// Боевой цикл headless-режима: строит собственный `MemStore`/`AppState` и
/// наполняет его боевыми данными Finam.
///
/// `mic` — биржа для справочника (например, `MISX`). Бесконечный цикл ингеста
/// завершается только при остановке процесса. Общая с GUI-путём логика (auth →
/// справочник → планировщик поверх переданного стора) вынесена в
/// [`run_ingest_into`]: здесь остаётся лишь создание собственного состояния.
///
/// В GUI-сборке (`tauri`+`live`) headless-вход не используется — боевой ингест
/// поднимает Tauri-setup через [`run_ingest_into`], поэтому там глушим dead_code.
#[cfg_attr(feature = "tauri", allow(dead_code))]
pub async fn run(mic: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut store = MemStore::new();
    store.migrate()?;
    let state = Arc::new(AppState::new(store));
    // CLI-вход пока не даёт способа отменить извне (нет обработчика сигнала) —
    // цикл крутится до убийства процесса, как и раньше. Флаг заведён ради
    // единообразия с GUI-путём и ALGOPACK-аналогом ([`run_algo`]); подключение
    // к Ctrl+C/Tauri-lifecycle — отдельная задача интеграции.
    run_ingest_into(state, mic, CancelFlag::new()).await
}

/// Общее ядро боевого запуска Finam: авторизация по секрету → справочник →
/// планировщик ингеста баров **поверх переданного стора** (`state`).
///
/// В отличие от старого [`run`], собственный стор здесь НЕ создаётся: держатель
/// состояния `S` передаёт вызывающая сторона. Это ключ к GAP-1/GAP-2 — GUI-путь
/// (Tauri-setup) отдаёт сюда `&AppState`, которым владеет окно, и вкладка
/// «Обзор» видит те же инструменты/бары; headless-путь отдаёт `Arc<AppState>`.
/// Оба варианта удовлетворяют `Borrow<AppState>`.
pub async fn run_ingest_into<S>(
    state: S,
    mic: &str,
    cancel: CancelFlag,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: Borrow<AppState>,
{
    let secret = load_secret()?;
    let auth = AuthManager::new(GrpcAuthTransport::new(), MemSecretStore::with_secret(secret));
    let md = FinamMarketData::connect(auth)?;
    run_source_into(md, state, mic, cancel).await
}

/// Часть [`run_ingest_into`] без построения боевого источника: справочник →
/// запись инструментов в переданный стор → планировщик поверх того же стора.
/// Источник абстрактный (`MarketData`), поэтому логика детерминированно
/// тестируется на фейке (см. тесты модуля).
async fn run_source_into<M, S>(
    source: M,
    state: S,
    mic: &str,
    cancel: CancelFlag,
) -> Result<(), Box<dyn std::error::Error>>
where
    M: MarketData,
    S: Borrow<AppState>,
{
    let symbols = register_instruments(&source, state.borrow(), mic).await?;
    tracing::info!(
        symbols = symbols.len(),
        "live: запуск планировщика ингеста в переданный стор"
    );
    let svc = IngestService::new(source, state, symbols, IngestConfig::default());
    svc.run(cancel).await;
    Ok(())
}

/// Запросить справочник биржи `mic` и записать инструменты в переданный стор.
/// Возвращает вотчлист символов для планировщика. Пустой справочник — ошибка
/// (боевая биржа всегда что-то отдаёт; пусто → неверный MIC/сбой авторизации).
async fn register_instruments<M: MarketData>(
    source: &M,
    state: &AppState,
    mic: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    tracing::info!(
        mic,
        endpoint = finam_proto::ENDPOINT,
        "live: запрос справочника"
    );
    let instruments = source.assets(mic).await?;
    tracing::info!(count = instruments.len(), "live: справочник получен");
    if instruments.is_empty() {
        return Err(format!("по бирже {mic} не получено инструментов").into());
    }
    state.ingest_instruments(&instruments)?;
    Ok(instruments.iter().map(|i| i.symbol.clone()).collect())
}

// ── Фаза 10.6.4 — ингест ALGOPACK (фича `moex`) ───────────────────────────────
//
// Резолвер токена ([`crate::algo_ingest::load_algo_token`]) и общее ядро запуска
// ([`crate::algo_ingest::run_algo_ingest_into`]) переехали в `algo_ingest`
// (модуль под фичей `moex`), чтобы боевой ALGOPACK-ингест из GUI мог собираться
// без фичи `live` (R-8/GAP-6). Здесь остаётся лишь headless-обёртка `run_algo`
// поверх собственного `Arc<AppState>` — симметрично [`run`].

/// Боевой цикл ингеста ALGOPACK поверх собственного `Arc<AppState>` (headless).
/// Тонкая обёртка над [`crate::algo_ingest::run_algo_ingest_into`]: символы
/// задаёт вызывающая сторона (обычно watchlist из настроек паспорта MOEX ALGO,
/// 10.8.1). В GUI-сборке ALGOPACK-ингест поднимает Tauri-setup напрямую через
/// то же ядро, поэтому здесь глушим dead_code.
#[cfg(feature = "moex")]
#[allow(dead_code)]
pub async fn run_algo(
    state: Arc<AppState>,
    symbols: Vec<String>,
    config: crate::algo_ingest::AlgoIngestConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // См. комментарий в `run` — CLI-вход пока не подключает отмену к внешнему
    // сигналу, но ядро уже умеет её принимать.
    crate::algo_ingest::run_algo_ingest_into(state, symbols, config, CancelFlag::new()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::DataError;
    use domain::{AssetClass, Bar, Instrument, Quote, TimeFrame, Trade};
    use storage::MemStore;

    /// Фейковый источник рыночных данных: отдаёт два инструмента справочника и
    /// по два бара на символ. Повторяет подход фейков `ingest`/`data` — без сети
    /// и реальных задержек, чтобы проверить наполнение переданного стора.
    struct FakeMarketData;

    impl FakeMarketData {
        fn instrument(symbol: &str, ticker: &str) -> Instrument {
            Instrument {
                symbol: symbol.to_owned(),
                ticker: ticker.to_owned(),
                name: ticker.to_owned(),
                asset_class: AssetClass::Equity,
                sector: None,
                lot_size: 1,
                isin: None,
            }
        }
    }

    impl MarketData for FakeMarketData {
        async fn assets(&self, _mic: &str) -> Result<Vec<Instrument>, DataError> {
            Ok(vec![
                Self::instrument("SBER@MISX", "SBER"),
                Self::instrument("LKOH@MISX", "LKOH"),
            ])
        }

        async fn bars(
            &self,
            _symbol: &str,
            _tf: TimeFrame,
            _from_ts: i64,
            to_ts: i64,
        ) -> Result<Vec<Bar>, DataError> {
            let mk = |ts: i64, o: f64, c: f64| Bar {
                ts,
                open: o,
                high: o.max(c),
                low: o.min(c),
                close: c,
                volume: 1_000.0,
            };
            Ok(vec![mk(to_ts - 1, 100.0, 101.0), mk(to_ts, 101.0, 103.0)])
        }

        async fn last_quote(&self, _symbol: &str) -> Result<Quote, DataError> {
            Err(DataError::Other("не используется".into()))
        }

        async fn latest_trades(&self, _symbol: &str) -> Result<Vec<Trade>, DataError> {
            Ok(Vec::new())
        }
    }

    fn migrated_state() -> Arc<AppState> {
        let mut store = MemStore::new();
        store.migrate().unwrap();
        Arc::new(AppState::new(store))
    }

    /// GAP-1/GAP-2: `register_instruments` пишет справочник в ПЕРЕДАННЫЙ стор
    /// (тот же, из которого читает команда `instruments`), а планировщик поверх
    /// того же стора наполняет его барами. Это ровно тот путь, что связывает
    /// боевой ингест с окном Tauri в [`run_source_into`].
    #[tokio::test]
    async fn register_and_ingest_fill_passed_store() {
        let state = migrated_state();

        // 1) Справочник → инструменты оказались в переданном сторе.
        let symbols = register_instruments(&FakeMarketData, state.as_ref(), "MISX")
            .await
            .unwrap();
        assert_eq!(symbols.len(), 2);
        let instruments = state.instruments().unwrap();
        assert_eq!(instruments.len(), 2);
        assert!(instruments.iter().any(|i| i.symbol == "SBER@MISX"));

        // 2) Один такт планировщика поверх того же стора → бары того же окна.
        let mut svc = IngestService::new(
            FakeMarketData,
            Arc::clone(&state),
            symbols,
            IngestConfig::default(),
        );
        svc.tick(1_000_000).await.unwrap();
        let bars = state
            .bars("SBER@MISX", TimeFrame::D1, 0, i64::MAX)
            .unwrap();
        assert_eq!(bars.len(), 2, "бары должны попасть в переданный стор");
    }

    /// Тот же путь работает и через `&AppState` (держатель GUI-пути): планировщик
    /// принимает заимствованное состояние (`Borrow<AppState>`), не создавая
    /// собственного стора.
    #[tokio::test]
    async fn ingest_service_accepts_borrowed_state() {
        let mut store = MemStore::new();
        store.migrate().unwrap();
        let state = AppState::new(store);

        let symbols = register_instruments(&FakeMarketData, &state, "MISX")
            .await
            .unwrap();
        // S = &AppState — тот же тип держателя, что отдаёт Tauri-setup.
        let mut svc = IngestService::new(
            FakeMarketData,
            &state,
            symbols,
            IngestConfig::default(),
        );
        svc.tick(1_000_000).await.unwrap();
        assert_eq!(
            state
                .bars("LKOH@MISX", TimeFrame::D1, 0, i64::MAX)
                .unwrap()
                .len(),
            2
        );
    }

    /// Пустой справочник (неверный MIC/сбой) → понятная ошибка, стор не тронут.
    #[tokio::test]
    async fn empty_catalog_is_error() {
        struct EmptySource;
        impl MarketData for EmptySource {
            async fn assets(&self, _mic: &str) -> Result<Vec<Instrument>, DataError> {
                Ok(Vec::new())
            }
            async fn bars(
                &self,
                _symbol: &str,
                _tf: TimeFrame,
                _from_ts: i64,
                _to_ts: i64,
            ) -> Result<Vec<Bar>, DataError> {
                Ok(Vec::new())
            }
            async fn last_quote(&self, _symbol: &str) -> Result<Quote, DataError> {
                Err(DataError::Other("не используется".into()))
            }
            async fn latest_trades(&self, _symbol: &str) -> Result<Vec<Trade>, DataError> {
                Ok(Vec::new())
            }
        }
        let state = migrated_state();
        let err = register_instruments(&EmptySource, state.as_ref(), "MISX")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("не получено инструментов"));
        assert!(state.instruments().unwrap().is_empty());
    }
}
