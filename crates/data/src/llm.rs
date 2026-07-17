//! LLM-провайдер для живого ИИ-резюме (фича `llm`, задачи 10.4.1/10.4.3).
//!
//! Единый трейт [`LlmProvider`] и три HTTP-реализации поверх [`HttpClient`]:
//! [`OpenRouter`] (дефолтный провайдер, OpenAI-совместимая схема), [`OpenAi`]
//! (та же схема, другой хост) и [`Anthropic`] (Messages API). Маппинг
//! запрос/ответ каждого провайдера — чистые функции, тестируемые на фейковом
//! транспорте без сети (см. [`crate::http::HttpTransport`]).
//!
//! Ключ провайдера (`api_key`) передаётся вызывающей стороной (`app`, после
//! резолвинга через env/`.env`/keyring) и хранится только на время вызова —
//! ни один тип модуля не реализует `Debug` (чтобы секрет физически не мог
//! попасть в отладочный вывод), а текст ошибок ([`DataError`]) собирается из
//! статус-кода/тела ответа, не из значения ключа.

use std::future::Future;

use crate::http::{HttpClient, HttpTransport};
use crate::{DataError, Method};

/// Запрос к LLM: системный промпт (опционально), пользовательский промпт,
/// модель, лимит токенов ответа.
#[derive(Debug, Clone)]
pub struct LlmRequest {
    /// Системная инструкция (роль `system`). `None` — без системного промпта.
    pub system: Option<String>,
    /// Пользовательский промпт (собирается в `domain::keyactivity::prompt`).
    pub prompt: String,
    /// Модель провайдера (например, `anthropic/claude-sonnet-5` для
    /// OpenRouter или `claude-sonnet-5` для Anthropic напрямую).
    pub model: String,
    /// Верхняя граница токенов ответа.
    pub max_tokens: u32,
}

/// Провайдер LLM: один метод — сводка текста по запросу [`LlmRequest`].
///
/// Ошибка — сетевая/HTTP/парсинг ([`DataError`]); вызывающая сторона (`app`)
/// трактует любую ошибку как повод для локальной деградации, не паникует.
pub trait LlmProvider: Send + Sync {
    /// Выполнить запрос и вернуть текст ответа модели.
    fn summarize(&self, req: LlmRequest) -> impl Future<Output = Result<String, DataError>> + Send;
}

/// Собрать OpenAI-совместимое тело `chat/completions` (общее для OpenRouter и
/// OpenAI — обе схемы идентичны на уровне JSON).
fn openai_chat_request(req: &LlmRequest) -> serde_json::Value {
    let mut messages = Vec::with_capacity(2);
    if let Some(system) = &req.system {
        messages.push(serde_json::json!({"role": "system", "content": system}));
    }
    messages.push(serde_json::json!({"role": "user", "content": req.prompt}));
    serde_json::json!({
        "model": req.model,
        "max_tokens": req.max_tokens,
        "messages": messages,
    })
}

/// Разобрать ответ OpenAI-совместимой схемы: `choices[0].message.content`.
fn parse_openai_chat_response(value: &serde_json::Value) -> Result<String, DataError> {
    value["choices"][0]["message"]["content"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| {
            DataError::Other(
                "некорректный ответ провайдера: нет choices[0].message.content".to_owned(),
            )
        })
}

/// Собрать тело Anthropic Messages API (`system` — отдельное top-level поле,
/// не элемент `messages`, в отличие от OpenAI-схемы).
fn anthropic_request(req: &LlmRequest) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": req.model,
        "max_tokens": req.max_tokens,
        "messages": [{"role": "user", "content": req.prompt}],
    });
    if let Some(system) = &req.system {
        body["system"] = serde_json::Value::String(system.clone());
    }
    body
}

/// Разобрать ответ Anthropic Messages API: `content[0].text`.
fn parse_anthropic_response(value: &serde_json::Value) -> Result<String, DataError> {
    value["content"][0]["text"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| {
            DataError::Other("некорректный ответ провайдера: нет content[0].text".to_owned())
        })
}

/// OpenRouter — дефолтный провайдер (агрегатор моделей, OpenAI-совместимая
/// схема запрос/ответ).
pub struct OpenRouter<T: HttpTransport> {
    client: HttpClient<T>,
    api_key: String,
}

impl<T: HttpTransport> OpenRouter<T> {
    /// Эндпоинт OpenRouter (OpenAI-совместимый).
    pub const URL: &'static str = "https://openrouter.ai/api/v1/chat/completions";

    /// Провайдер поверх готового HTTP-клиента и ключа `api_key`.
    pub fn new(client: HttpClient<T>, api_key: impl Into<String>) -> Self {
        Self {
            client,
            api_key: api_key.into(),
        }
    }
}

impl<T: HttpTransport> LlmProvider for OpenRouter<T> {
    async fn summarize(&self, req: LlmRequest) -> Result<String, DataError> {
        let body = openai_chat_request(&req);
        let headers = vec![
            (
                "Authorization".to_owned(),
                format!("Bearer {}", self.api_key),
            ),
            ("Content-Type".to_owned(), "application/json".to_owned()),
        ];
        let value = self
            .client
            .post_json(Method::Llm, Self::URL, &headers, &body)
            .await?;
        parse_openai_chat_response(&value)
    }
}

/// OpenAI — прямое подключение (та же схема запрос/ответ, что у OpenRouter).
pub struct OpenAi<T: HttpTransport> {
    client: HttpClient<T>,
    api_key: String,
}

impl<T: HttpTransport> OpenAi<T> {
    /// Эндпоинт OpenAI `chat/completions`.
    pub const URL: &'static str = "https://api.openai.com/v1/chat/completions";

    /// Провайдер поверх готового HTTP-клиента и ключа `api_key`.
    pub fn new(client: HttpClient<T>, api_key: impl Into<String>) -> Self {
        Self {
            client,
            api_key: api_key.into(),
        }
    }
}

impl<T: HttpTransport> LlmProvider for OpenAi<T> {
    async fn summarize(&self, req: LlmRequest) -> Result<String, DataError> {
        let body = openai_chat_request(&req);
        let headers = vec![
            (
                "Authorization".to_owned(),
                format!("Bearer {}", self.api_key),
            ),
            ("Content-Type".to_owned(), "application/json".to_owned()),
        ];
        let value = self
            .client
            .post_json(Method::Llm, Self::URL, &headers, &body)
            .await?;
        parse_openai_chat_response(&value)
    }
}

/// Anthropic — прямое подключение к Messages API (заголовки `x-api-key` +
/// `anthropic-version`, отдельная схема тела запроса/ответа).
pub struct Anthropic<T: HttpTransport> {
    client: HttpClient<T>,
    api_key: String,
}

impl<T: HttpTransport> Anthropic<T> {
    /// Эндпоинт Anthropic Messages API.
    pub const URL: &'static str = "https://api.anthropic.com/v1/messages";
    /// Версия API (заголовок `anthropic-version`), фиксированная как в
    /// официальной документации Anthropic.
    pub const API_VERSION: &'static str = "2023-06-01";

    /// Провайдер поверх готового HTTP-клиента и ключа `api_key`.
    pub fn new(client: HttpClient<T>, api_key: impl Into<String>) -> Self {
        Self {
            client,
            api_key: api_key.into(),
        }
    }
}

impl<T: HttpTransport> LlmProvider for Anthropic<T> {
    async fn summarize(&self, req: LlmRequest) -> Result<String, DataError> {
        let body = anthropic_request(&req);
        let headers = vec![
            ("x-api-key".to_owned(), self.api_key.clone()),
            ("anthropic-version".to_owned(), Self::API_VERSION.to_owned()),
            ("Content-Type".to_owned(), "application/json".to_owned()),
        ];
        let value = self
            .client
            .post_json(Method::Llm, Self::URL, &headers, &body)
            .await?;
        parse_anthropic_response(&value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::HttpResponse;
    use crate::{Backoff, RateLimiter};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;
    use std::time::Duration;

    /// Один зафиксированный вызов фейкового транспорта.
    #[derive(Debug, Clone)]
    struct CapturedCall {
        url: String,
        headers: Vec<(String, String)>,
        body: Option<Vec<u8>>,
    }

    /// Фейковый транспорт: считает вызовы, запоминает аргументы, отдаёт
    /// заранее заданную программу результатов (без сети).
    struct FakeTransport {
        calls: AtomicU32,
        captured: Mutex<Vec<CapturedCall>>,
        program: Vec<Result<HttpResponse, DataError>>,
    }

    impl FakeTransport {
        fn always(resp: HttpResponse) -> Self {
            Self {
                calls: AtomicU32::new(0),
                captured: Mutex::new(Vec::new()),
                program: vec![Ok(resp)],
            }
        }

        fn err_then_ok(err: DataError, resp: HttpResponse) -> Self {
            Self {
                calls: AtomicU32::new(0),
                captured: Mutex::new(Vec::new()),
                program: vec![Err(err), Ok(resp)],
            }
        }

        fn calls(&self) -> u32 {
            self.calls.load(Ordering::SeqCst)
        }

        fn captured_calls(&self) -> Vec<CapturedCall> {
            self.captured.lock().expect("mutex отравлен").clone()
        }
    }

    impl HttpTransport for FakeTransport {
        async fn get(
            &self,
            _url: &str,
            _headers: &[(String, String)],
        ) -> Result<HttpResponse, DataError> {
            unreachable!("LLM-провайдеры используют только POST")
        }

        async fn post(
            &self,
            url: &str,
            headers: &[(String, String)],
            body: Vec<u8>,
        ) -> Result<HttpResponse, DataError> {
            self.captured
                .lock()
                .expect("mutex отравлен")
                .push(CapturedCall {
                    url: url.to_owned(),
                    headers: headers.to_vec(),
                    body: Some(body),
                });
            let i = self.calls.fetch_add(1, Ordering::SeqCst) as usize;
            self.program[i.min(self.program.len() - 1)].clone()
        }
    }

    fn ok_json(status: u16, body: &str) -> HttpResponse {
        HttpResponse {
            status,
            body: body.as_bytes().to_vec(),
        }
    }

    fn no_sleep_backoff(max_retries: u32) -> Backoff {
        Backoff::new(Duration::ZERO, 1.0, Duration::ZERO, max_retries)
    }

    fn client(transport: FakeTransport) -> HttpClient<FakeTransport> {
        HttpClient::with_policy(transport, RateLimiter::finam_default(), no_sleep_backoff(3))
    }

    fn req() -> LlmRequest {
        LlmRequest {
            system: Some("Ты — аналитик.".into()),
            prompt: "Резюмируй активность.".into(),
            model: "test-model".into(),
            max_tokens: 256,
        }
    }

    // ── OpenRouter ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn openrouter_maps_request_and_response() {
        let http = client(FakeTransport::always(ok_json(
            200,
            r#"{"choices":[{"message":{"content":"итог"}}]}"#,
        )));
        let provider = OpenRouter::new(http, "sk-or-secret");
        let text = provider.summarize(req()).await.unwrap();
        assert_eq!(text, "итог");

        let calls = provider.client.transport.captured_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].url, OpenRouter::<FakeTransport>::URL);
        assert!(calls[0]
            .headers
            .contains(&("Authorization".to_owned(), "Bearer sk-or-secret".to_owned())));

        let body: serde_json::Value =
            serde_json::from_slice(calls[0].body.as_ref().unwrap()).unwrap();
        assert_eq!(body["model"], "test-model");
        assert_eq!(body["max_tokens"], 256);
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "Резюмируй активность.");
    }

    #[tokio::test]
    async fn openrouter_without_system_omits_system_message() {
        let http = client(FakeTransport::always(ok_json(
            200,
            r#"{"choices":[{"message":{"content":"ok"}}]}"#,
        )));
        let provider = OpenRouter::new(http, "k");
        let mut r = req();
        r.system = None;
        provider.summarize(r).await.unwrap();

        let calls = provider.client.transport.captured_calls();
        let body: serde_json::Value =
            serde_json::from_slice(calls[0].body.as_ref().unwrap()).unwrap();
        assert_eq!(body["messages"].as_array().unwrap().len(), 1);
        assert_eq!(body["messages"][0]["role"], "user");
    }

    #[tokio::test]
    async fn openrouter_malformed_response_is_an_error() {
        let http = client(FakeTransport::always(ok_json(200, r#"{"unexpected":1}"#)));
        let provider = OpenRouter::new(http, "k");
        let err = provider.summarize(req()).await.unwrap_err();
        assert!(matches!(err, DataError::Other(_)));
    }

    #[tokio::test]
    async fn openrouter_retries_transient_error_then_succeeds() {
        let http = client(FakeTransport::err_then_ok(
            DataError::Transport("connection reset".into()),
            ok_json(200, r#"{"choices":[{"message":{"content":"ok"}}]}"#),
        ));
        let provider = OpenRouter::new(http, "k");
        let text = provider.summarize(req()).await.unwrap();
        assert_eq!(text, "ok");
        assert_eq!(provider.client.transport.calls(), 2);
    }

    // ── OpenAi ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn openai_maps_request_and_response() {
        let http = client(FakeTransport::always(ok_json(
            200,
            r#"{"choices":[{"message":{"content":"summary"}}]}"#,
        )));
        let provider = OpenAi::new(http, "sk-openai-secret");
        let text = provider.summarize(req()).await.unwrap();
        assert_eq!(text, "summary");

        let calls = provider.client.transport.captured_calls();
        assert_eq!(calls[0].url, OpenAi::<FakeTransport>::URL);
        assert!(calls[0].headers.contains(&(
            "Authorization".to_owned(),
            "Bearer sk-openai-secret".to_owned()
        )));
    }

    #[tokio::test]
    async fn openai_unauthorized_does_not_retry() {
        let http = client(FakeTransport::always(ok_json(401, "unauthorized")));
        let provider = OpenAi::new(http, "bad-key");
        let err = provider.summarize(req()).await.unwrap_err();
        assert!(matches!(err, DataError::Auth(_)));
        assert_eq!(provider.client.transport.calls(), 1);
    }

    // ── Anthropic ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn anthropic_maps_request_and_response() {
        let http = client(FakeTransport::always(ok_json(
            200,
            r#"{"content":[{"type":"text","text":"итог anthropic"}]}"#,
        )));
        let provider = Anthropic::new(http, "sk-ant-secret");
        let text = provider.summarize(req()).await.unwrap();
        assert_eq!(text, "итог anthropic");

        let calls = provider.client.transport.captured_calls();
        assert_eq!(calls[0].url, Anthropic::<FakeTransport>::URL);
        assert!(calls[0]
            .headers
            .contains(&("x-api-key".to_owned(), "sk-ant-secret".to_owned())));
        assert!(calls[0].headers.contains(&(
            "anthropic-version".to_owned(),
            Anthropic::<FakeTransport>::API_VERSION.to_owned()
        )));
        // У Anthropic Authorization не используется — секрет только в x-api-key.
        assert!(!calls[0].headers.iter().any(|(k, _)| k == "Authorization"));

        let body: serde_json::Value =
            serde_json::from_slice(calls[0].body.as_ref().unwrap()).unwrap();
        assert_eq!(body["model"], "test-model");
        assert_eq!(body["max_tokens"], 256);
        assert_eq!(body["system"], "Ты — аналитик.");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn anthropic_without_system_omits_system_field() {
        let http = client(FakeTransport::always(ok_json(
            200,
            r#"{"content":[{"type":"text","text":"ok"}]}"#,
        )));
        let provider = Anthropic::new(http, "k");
        let mut r = req();
        r.system = None;
        provider.summarize(r).await.unwrap();

        let calls = provider.client.transport.captured_calls();
        let body: serde_json::Value =
            serde_json::from_slice(calls[0].body.as_ref().unwrap()).unwrap();
        assert!(body.get("system").is_none());
    }

    #[tokio::test]
    async fn anthropic_malformed_response_is_an_error() {
        let http = client(FakeTransport::always(ok_json(200, r#"{"content":[]}"#)));
        let provider = Anthropic::new(http, "k");
        let err = provider.summarize(req()).await.unwrap_err();
        assert!(matches!(err, DataError::Other(_)));
    }

    // ── Секрет не попадает в отладочный вывод/ошибки ─────────────────────────

    #[tokio::test]
    async fn provider_error_does_not_leak_api_key() {
        let http = client(FakeTransport::always(ok_json(500, "boom")));
        let provider = Anthropic::new(http, "sk-ant-very-secret-value");
        let err = provider.summarize(req()).await.unwrap_err();
        let msg = err.to_string();
        assert!(!msg.contains("sk-ant-very-secret-value"));
    }
}
