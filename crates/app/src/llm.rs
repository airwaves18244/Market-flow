//! Живой ИИ-провайдер для резюме Key Activity (фаза 10.4, фича `llm`).
//!
//! Оркестрация поверх `data::llm`: выбор провайдера/модели/лимита токенов из
//! персистентных настроек ([`SettingsDto`]), резолвинг ключа провайдера
//! (env → `.env` → ОС-keyring, тот же порядок, что у секрета Finam в
//! [`crate::live::load_secret`]) и кэш на время сессии приложения ([`SummaryCache`]) —
//! повторный вызов с тем же входом (строки Key Activity + период + провайдер +
//! модель) не дёргает провайдера повторно.
//!
//! Любая ошибка — отсутствующий ключ, неизвестный код провайдера, сетевая
//! ошибка, тайм-аут, некорректный ответ — тихо превращается в `None`:
//! вызывающая сторона ([`crate::api::key_activity_summary_live`]) в этом
//! случае переключается на локальный текстовый свод
//! (`domain::keyactivity::prompt::fallback_summary`), как и раньше без фичи.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use std::time::Duration;

use data::llm::{Anthropic, LlmProvider, LlmRequest, OpenAi, OpenRouter};
use data::{HttpClient, ReqwestTransport};
use domain::keyactivity::{prompt, KeyActivityRow, Period};

use crate::dto::SettingsDto;

/// Тайм-аут на весь вызов провайдера (сверх тайм-аута отдельного HTTP-запроса
/// внутри `data::http::ReqwestTransport`) — защита от повисшего ретрай-цикла
/// при исчерпании бюджета `Backoff`.
const CALL_TIMEOUT: Duration = Duration::from_secs(45);

/// Верхняя граница символов промпта, передаваемого провайдеру (см.
/// `domain::keyactivity::prompt::build_prompt`).
const PROMPT_MAX_CHARS: usize = 8_000;

/// Известные коды провайдера (см. `SettingsDto::validate` — тот же список).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderKind {
    OpenRouter,
    OpenAi,
    Anthropic,
}

impl ProviderKind {
    fn from_code(code: &str) -> Option<Self> {
        match code {
            "openrouter" => Some(Self::OpenRouter),
            "openai" => Some(Self::OpenAi),
            "anthropic" => Some(Self::Anthropic),
            _ => None,
        }
    }

    /// Имя переменной окружения с ключом провайдера (см. `.env.example`).
    fn env_var(self) -> &'static str {
        match self {
            Self::OpenRouter => "OPENROUTER_API_KEY",
            Self::OpenAi => "OPENAI_API_KEY",
            Self::Anthropic => "ANTHROPIC_API_KEY",
        }
    }

    /// Имя записи в ОС-keyring (сервис — общий `KeyringSecretStore::DEFAULT_SERVICE`).
    #[cfg(feature = "keyring")]
    fn keyring_user(self) -> &'static str {
        match self {
            Self::OpenRouter => "llm-openrouter-api-key",
            Self::OpenAi => "llm-openai-api-key",
            Self::Anthropic => "llm-anthropic-api-key",
        }
    }
}

/// Достать ключ провайдера: переменная окружения → файл `.env` (поиск вверх по
/// дереву каталогов от текущего рабочего) → ОС-keyring (фича `keyring`).
/// `None` — ключ нигде не найден (не ошибка, вызывающая сторона деградирует
/// в локальный свод). Ключ нигде не логируется и не попадает в `Result::Err`.
fn resolve_key(kind: ProviderKind) -> Option<String> {
    let var = kind.env_var();
    if let Ok(v) = std::env::var(var) {
        let v = v.trim();
        if !v.is_empty() {
            return Some(v.to_owned());
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(v) = data::dotenv::find_dotenv_value(&cwd, 4, &[var]) {
            return Some(v);
        }
    }
    #[cfg(feature = "keyring")]
    {
        use data::{KeyringSecretStore, SecretStore};
        if let Ok(Some(v)) = KeyringSecretStore::with_target(
            KeyringSecretStore::DEFAULT_SERVICE,
            kind.keyring_user(),
        )
        .load()
        {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_owned());
            }
        }
    }
    None
}

/// Кэш готовых ИИ-резюме на время сессии приложения (не персистится на диск).
/// Ключ — хеш входа: строки Key Activity + период + провайдер + модель +
/// лимит токенов (см. [`cache_key`]). Одна запись хранит один готовый текст.
#[derive(Default)]
pub struct SummaryCache {
    entries: Mutex<HashMap<u64, String>>,
}

impl SummaryCache {
    /// Пустой кэш.
    pub fn new() -> Self {
        Self::default()
    }

    fn get(&self, key: u64) -> Option<String> {
        self.entries
            .lock()
            .expect("cache mutex отравлен")
            .get(&key)
            .cloned()
    }

    fn put(&self, key: u64, value: String) {
        self.entries
            .lock()
            .expect("cache mutex отравлен")
            .insert(key, value);
    }
}

/// Хеш входа для кэша: сериализованные строки Key Activity + период +
/// провайдер + модель + лимит токенов. Детерминирован при одинаковом входе —
/// повторный вызов с теми же аргументами возвращает тот же ключ.
fn cache_key(
    rows: &[KeyActivityRow],
    period: Period,
    provider: &str,
    model: &str,
    max_tokens: u32,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(rows)
        .unwrap_or_default()
        .hash(&mut hasher);
    period.label().hash(&mut hasher);
    provider.hash(&mut hasher);
    model.hash(&mut hasher);
    max_tokens.hash(&mut hasher);
    hasher.finish()
}

/// Вызвать выбранного в `settings` провайдера и вернуть текст резюме.
///
/// `None` возвращается, когда фича неприменима (неизвестный код провайдера
/// или ключ нигде не найден) или сам вызов не удался (сеть/тайм-аут/ошибка
/// формата ответа) — в обоих случаях вызывающая сторона переключается на
/// локальный текстовый свод. Кэш-хит не требует ключа: если резюме для этого
/// входа уже получено в этой сессии, повторный вызов провайдера не делается.
pub async fn summarize_key_activity(
    cache: &SummaryCache,
    settings: &SettingsDto,
    rows: &[KeyActivityRow],
    period: Period,
) -> Option<String> {
    let kind = ProviderKind::from_code(&settings.llm_provider)?;
    let model = settings.llm_model.clone();
    let max_tokens =
        u32::try_from(settings.llm_token_limit.clamp(1, i64::from(u32::MAX))).unwrap_or(u32::MAX);

    let key = cache_key(rows, period, &settings.llm_provider, &model, max_tokens);
    if let Some(cached) = cache.get(key) {
        return Some(cached);
    }

    let api_key = resolve_key(kind)?;
    let prompt_text = prompt::build_prompt(rows, period, PROMPT_MAX_CHARS);
    let req = LlmRequest {
        system: None,
        prompt: prompt_text,
        model,
        max_tokens,
    };

    let transport = ReqwestTransport::new().ok()?;
    let http = HttpClient::new(transport);

    let call = call_provider(kind, http, api_key, req);
    let result = match tokio::time::timeout(CALL_TIMEOUT, call).await {
        Ok(r) => r,
        Err(_) => Err(data::DataError::Transport(
            "тайм-аут вызова LLM-провайдера".to_owned(),
        )),
    };

    match result {
        Ok(text) => {
            cache.put(key, text.clone());
            Some(text)
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                provider = %settings.llm_provider,
                "LLM-провайдер недоступен, переключение на локальный свод"
            );
            None
        }
    }
}

/// Диспетчер: собрать нужного провайдера поверх готового HTTP-клиента и
/// вызвать его. Отдельная функция — чтобы конкретный тип провайдера не
/// протекал в сигнатуру [`summarize_key_activity`] (там нужен только `Result`).
async fn call_provider(
    kind: ProviderKind,
    http: HttpClient<ReqwestTransport>,
    api_key: String,
    req: LlmRequest,
) -> Result<String, data::DataError> {
    match kind {
        ProviderKind::OpenRouter => OpenRouter::new(http, api_key).summarize(req).await,
        ProviderKind::OpenAi => OpenAi::new(http, api_key).summarize(req).await,
        ProviderKind::Anthropic => Anthropic::new(http, api_key).summarize(req).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::http::{HttpResponse, HttpTransport};
    use domain::keyactivity::Metric;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn row(secid: &str) -> KeyActivityRow {
        KeyActivityRow {
            secid: secid.into(),
            rule_id: "r1".into(),
            rule_name: "Аномальный объём".into(),
            metric: Metric::VolumeZScore,
            value: 4.2,
            ts: 1000,
            importance: 3.0,
        }
    }

    fn settings_with(provider: &str, model: &str, token_limit: i64) -> SettingsDto {
        SettingsDto {
            llm_provider: provider.to_owned(),
            llm_model: model.to_owned(),
            llm_token_limit: token_limit,
            ..SettingsDto::default()
        }
    }

    // ── resolve_key / provider dispatch — через переменные окружения ────────

    /// Гвард переменной окружения: восстанавливает исходное значение при drop,
    /// чтобы тесты не гонялись друг за другом за общим процессным окружением.
    struct EnvGuard {
        key: &'static str,
        prev: Option<String>,
    }
    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let prev = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, prev }
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    #[test]
    fn unknown_provider_code_has_no_key_resolution() {
        assert!(ProviderKind::from_code("bogus").is_none());
    }

    #[test]
    fn resolve_key_reads_from_env_var() {
        let _guard = EnvGuard::set("ANTHROPIC_API_KEY", "sk-ant-test-value");
        let key = resolve_key(ProviderKind::Anthropic);
        assert_eq!(key.as_deref(), Some("sk-ant-test-value"));
    }

    #[test]
    fn resolve_key_ignores_empty_env_var() {
        let _guard = EnvGuard::set("OPENAI_API_KEY", "");
        // Пустая переменная окружения — как будто её нет (без .env/keyring
        // в тестовом окружении CI ключ не найдётся вовсе).
        assert_eq!(resolve_key(ProviderKind::OpenAi), None);
    }

    // ── cache_key — детерминизм и чувствительность к входу ───────────────────

    #[test]
    fn cache_key_is_deterministic_for_same_input() {
        let rows = vec![row("SBER")];
        let a = cache_key(&rows, Period::H1, "openrouter", "m1", 1000);
        let b = cache_key(&rows, Period::H1, "openrouter", "m1", 1000);
        assert_eq!(a, b);
    }

    #[test]
    fn cache_key_differs_on_model_or_provider_change() {
        let rows = vec![row("SBER")];
        let base = cache_key(&rows, Period::H1, "openrouter", "m1", 1000);
        assert_ne!(base, cache_key(&rows, Period::H1, "anthropic", "m1", 1000));
        assert_ne!(base, cache_key(&rows, Period::H1, "openrouter", "m2", 1000));
        assert_ne!(base, cache_key(&rows, Period::D1, "openrouter", "m1", 1000));
    }

    // ── summarize_key_activity: деградация без ключа/с неизвестным провайдером ─

    #[tokio::test]
    async fn summarize_returns_none_for_unknown_provider() {
        // Гарантированно нет такого ключа в окружении теста.
        let cache = SummaryCache::new();
        let settings = settings_with("bogus-provider", "m1", 1000);
        let rows = vec![row("SBER")];
        let out = summarize_key_activity(&cache, &settings, &rows, Period::H1).await;
        assert!(out.is_none());
    }

    #[tokio::test]
    async fn summarize_returns_none_without_key() {
        // На случай, если в окружении CI случайно задан ANTHROPIC_API_KEY —
        // явно очищаем его на время теста.
        let prev = std::env::var("ANTHROPIC_API_KEY").ok();
        std::env::remove_var("ANTHROPIC_API_KEY");

        let cache = SummaryCache::new();
        let settings = settings_with("anthropic", "m1", 1000);
        let rows = vec![row("SBER")];
        let out = summarize_key_activity(&cache, &settings, &rows, Period::H1).await;
        assert!(out.is_none());

        if let Some(v) = prev {
            std::env::set_var("ANTHROPIC_API_KEY", v);
        }
    }

    // ── Кэш-хит не требует ключа и не увеличивает счётчик вызовов провайдера ──
    //
    // `summarize_key_activity` строит собственный `ReqwestTransport` внутри —
    // фейковый транспорт сюда не подставить напрямую. Проверяем контракт кэша
    // через публичный API `SummaryCache` (put/get) с тем же `cache_key`,
    // который использует `summarize_key_activity`: если запись уже там, вызов
    // провайдера не нужен вовсе (early return до `resolve_key`).
    #[tokio::test]
    async fn cache_hit_skips_provider_and_key_resolution() {
        let cache = SummaryCache::new();
        let settings = settings_with("anthropic", "m1", 1000);
        let rows = vec![row("SBER")];
        let key = cache_key(&rows, Period::H1, "anthropic", "m1", 1000);
        cache.put(key, "закэшированный текст".to_owned());

        // Ключ провайдера точно отсутствует — если бы кэш не сработал,
        // summarize_key_activity вернула бы None (нет ключа), а не Some.
        let prev = std::env::var("ANTHROPIC_API_KEY").ok();
        std::env::remove_var("ANTHROPIC_API_KEY");

        let out = summarize_key_activity(&cache, &settings, &rows, Period::H1).await;
        assert_eq!(out.as_deref(), Some("закэшированный текст"));

        if let Some(v) = prev {
            std::env::set_var("ANTHROPIC_API_KEY", v);
        }
    }

    // ── Счётчик вызовов фейкового провайдера (через data::llm напрямую) ───────
    //
    // `call_provider`/`resolve_key` используют реальный `ReqwestTransport`,
    // поэтому end-to-end проверка "кэш не дёргает провайдера" — здесь, тем же
    // фейковым транспортом, что и в `data::llm::tests`, воспроизводя логику
    // `summarize_key_activity` (кэш-хит → ранний возврат, до вызова провайдера).
    struct CountingTransport {
        calls: std::sync::Arc<AtomicU32>,
        response: HttpResponse,
    }
    impl HttpTransport for CountingTransport {
        async fn get(
            &self,
            _url: &str,
            _headers: &[(String, String)],
        ) -> Result<HttpResponse, data::DataError> {
            unreachable!()
        }
        async fn post(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            _body: Vec<u8>,
        ) -> Result<HttpResponse, data::DataError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.response.clone())
        }
    }

    #[tokio::test]
    async fn repeated_call_with_cache_hit_does_not_call_provider_again() {
        let call_count = std::sync::Arc::new(AtomicU32::new(0));
        let transport = CountingTransport {
            calls: call_count.clone(),
            response: HttpResponse {
                status: 200,
                body: r#"{"content":[{"type":"text","text":"итог"}]}"#.as_bytes().to_vec(),
            },
        };
        let http = HttpClient::new(transport);
        let provider = Anthropic::new(http, "sk-ant-fake");

        let cache = SummaryCache::new();
        let rows = vec![row("SBER")];
        let key = cache_key(&rows, Period::H1, "anthropic", "m1", 1000);

        // Воспроизводим шаг `summarize_key_activity`: cache.get() → если
        // промах, зовём провайдера и кладём результат в кэш.
        async fn round(
            cache: &SummaryCache,
            provider: &Anthropic<CountingTransport>,
            key: u64,
        ) -> String {
            if let Some(cached) = cache.get(key) {
                return cached;
            }
            let text = provider
                .summarize(LlmRequest {
                    system: None,
                    prompt: "x".into(),
                    model: "m1".into(),
                    max_tokens: 1000,
                })
                .await
                .unwrap();
            cache.put(key, text.clone());
            text
        }

        let first = round(&cache, &provider, key).await;
        assert_eq!(first, "итог");
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Второй вызов с тем же входом — кэш-хит, к (фейковой) сети не идём.
        let second = round(&cache, &provider, key).await;
        assert_eq!(second, "итог");
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }
}
