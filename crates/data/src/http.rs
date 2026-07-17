//! HTTP/JSON-транспорт (фича `http`).
//!
//! Тонкая обёртка поверх REST/JSON API (MOEX ALGOPACK, LLM-провайдеры) —
//! в отличие от gRPC-слоя ([`crate::grpc`]) здесь нет единого протобаф-контракта,
//! только «условный» HTTP `GET` с заголовками и JSON-телом ответа.
//!
//! Как и в [`crate::grpc`], оркестрация ([`HttpClient`]) отделена от транспорта
//! ([`HttpTransport`]): боевой транспорт — [`ReqwestTransport`] поверх `reqwest`
//! (TLS — rustls, сжатие — gzip), а логика «держать per-method лимит, повторять
//! транзиентные сбои с backoff, маппить статус в [`DataError`]» тестируется на
//! фейковом транспорте без сети.
//!
//! Секреты (значение заголовка `Authorization`) передаются вызывающей стороной
//! на каждый запрос и нигде здесь не сохраняются и не логируются — ни один тип
//! модуля не хранит заголовки дольше одного вызова.

use std::future::Future;
use std::time::{Duration, Instant};

use crate::{Backoff, DataError, Method, RateLimiter};

/// Тайм-аут HTTP-запроса по умолчанию.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Ответ транспорта: статус-код и «сырое» тело.
///
/// Тело не разбирается на уровне транспорта — JSON парсится в [`HttpClient`],
/// потому что тело нужно прочитать и у не-2xx ответов (для сообщения об
/// ошибке), а не только у успешных.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP статус-код ответа.
    pub status: u16,
    /// Тело ответа как есть (обычно UTF-8 JSON).
    pub body: Vec<u8>,
}

/// Транспорт одного HTTP `GET`/`POST`-запроса. Абстрагирует сеть, чтобы
/// оркестрацию [`HttpClient`] (лимиты, ретраи, маппинг ошибок) можно было
/// тестировать детерминированно, без реального HTTP.
///
/// Возвращает `Err` только при сбое самого обмена (сеть, DNS, тайм-аут,
/// некорректные заголовки) — любой полученный HTTP-ответ, включая 4xx/5xx,
/// это `Ok(HttpResponse { .. })`; классификацию статуса делает [`HttpClient`].
pub trait HttpTransport: Send + Sync {
    /// Выполнить `GET url` с заголовками `headers` (пары «имя», «значение»).
    fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
    ) -> impl Future<Output = Result<HttpResponse, DataError>> + Send;

    /// Выполнить `POST url` с заголовками `headers` и телом `body` (сырые байты
    /// — JSON-тело сериализует вызывающая сторона в [`HttpClient::post_json`]).
    fn post(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: Vec<u8>,
    ) -> impl Future<Output = Result<HttpResponse, DataError>> + Send;
}

/// HTTP/JSON-клиент: держит per-method rate-limit и повторяет транзиентные
/// сбои с [`Backoff`], поверх произвольного [`HttpTransport`].
pub struct HttpClient<T: HttpTransport> {
    // `pub(crate)`, а не приватное: тестам смежных модулей (`crate::llm`) нужен
    // прямой доступ к фейковому транспорту (счётчик вызовов, захваченные
    // запросы) без искусственного публичного геттера в боевом API.
    pub(crate) transport: T,
    limiter: RateLimiter,
    backoff: Backoff,
}

impl<T: HttpTransport> HttpClient<T> {
    /// Клиент с разумными умолчаниями (лимит и backoff Finam по умолчанию).
    pub fn new(transport: T) -> Self {
        Self::with_policy(
            transport,
            RateLimiter::finam_default(),
            Backoff::finam_default(),
        )
    }

    /// Клиент с явными политиками (для тестов и тонкой настройки).
    pub fn with_policy(transport: T, limiter: RateLimiter, backoff: Backoff) -> Self {
        Self {
            transport,
            limiter,
            backoff,
        }
    }

    /// `GET url` с заголовками `headers`, разобранный как JSON.
    ///
    /// `method` — ключ per-method лимита (см. [`Method`]) и метка ошибки
    /// [`DataError::RateLimited`]. Транзиентные сбои (429, 5xx, сетевые/тайм-аут)
    /// повторяются с экспоненциальным backoff; прочие 4xx возвращаются сразу.
    pub async fn get_json(
        &self,
        method: Method,
        url: &str,
        headers: &[(String, String)],
    ) -> Result<serde_json::Value, DataError> {
        let mut attempt = 0u32;
        loop {
            // Лимит метода — раздельный per-method счётчик.
            if let Err(e) = self.limiter.try_acquire(method) {
                if self.backoff.is_exhausted(attempt) {
                    return Err(e);
                }
                self.sleep_for(attempt).await;
                attempt += 1;
                continue;
            }

            let outcome = match self.transport.get(url, headers).await {
                Ok(resp) => response_to_result(method, resp),
                Err(e) => Err(e),
            };

            match outcome {
                Ok(value) => return Ok(value),
                Err(e) if e.is_retryable() && !self.backoff.is_exhausted(attempt) => {
                    self.sleep_for(attempt).await;
                    attempt += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// `POST url` с заголовками `headers` и JSON-телом `body`, ответ разобран
    /// как JSON. Та же политика лимитов/ретраев/маппинга ошибок, что и у
    /// [`HttpClient::get_json`] (нужна LLM-провайдерам: их API — POST с
    /// JSON-телом запроса).
    pub async fn post_json(
        &self,
        method: Method,
        url: &str,
        headers: &[(String, String)],
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, DataError> {
        let body_bytes = serde_json::to_vec(body)
            .map_err(|e| DataError::Other(format!("не удалось сериализовать JSON-тело: {e}")))?;

        let mut attempt = 0u32;
        loop {
            if let Err(e) = self.limiter.try_acquire(method) {
                if self.backoff.is_exhausted(attempt) {
                    return Err(e);
                }
                self.sleep_for(attempt).await;
                attempt += 1;
                continue;
            }

            let outcome = match self.transport.post(url, headers, body_bytes.clone()).await {
                Ok(resp) => response_to_result(method, resp),
                Err(e) => Err(e),
            };

            match outcome {
                Ok(value) => return Ok(value),
                Err(e) if e.is_retryable() && !self.backoff.is_exhausted(attempt) => {
                    self.sleep_for(attempt).await;
                    attempt += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn sleep_for(&self, attempt: u32) {
        let delay = self.backoff.delay_with_jitter(attempt, jitter_fraction());
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
    }
}

/// Доля джиттера в `[0, 1)` из субнаносекунд монотонных часов. Размазывает
/// повторы без внешнего `rand`; качество ГПСЧ здесь не критично.
fn jitter_fraction() -> f64 {
    let nanos = Instant::now().elapsed().subsec_nanos();
    f64::from(nanos % 1_000) / 1_000.0
}

/// Маппинг HTTP-ответа в [`DataError`] с учётом ретраябельности
/// (см. [`DataError::is_retryable`]).
///
/// - `2xx` — тело разбирается как JSON;
/// - `429` — [`DataError::RateLimited`] (ретраябельно);
/// - `401`/`403` — [`DataError::Auth`] (не ретраябельно, нужен re-auth);
/// - `5xx` — [`DataError::Transport`] (ретраябельно, серверная сторона);
/// - прочие `4xx` и неожиданные коды — [`DataError::Other`] (не ретраябельно).
fn response_to_result(method: Method, resp: HttpResponse) -> Result<serde_json::Value, DataError> {
    match resp.status {
        200..=299 => serde_json::from_slice(&resp.body)
            .map_err(|e| DataError::Other(format!("не удалось разобрать JSON-ответ: {e}"))),
        429 => Err(DataError::RateLimited(method.as_str())),
        401 | 403 => Err(DataError::Auth(format!(
            "HTTP {}: доступ отклонён ({})",
            resp.status, method
        ))),
        500..=599 => Err(DataError::Transport(format!(
            "HTTP {}: серверная ошибка ({})",
            resp.status, method
        ))),
        other => Err(DataError::Other(format!(
            "HTTP {other}: неожиданный статус ({method})"
        ))),
    }
}

/// Боевой транспорт поверх `reqwest` (rustls, gzip).
///
/// Один `reqwest::Client` переиспользуется между вызовами (внутри — пул
/// соединений). Тайм-аут запроса конфигурируется, по умолчанию —
/// [`DEFAULT_TIMEOUT`].
///
/// `Clone` — дешёвый (`reqwest::Client` внутри держит `Arc` на пул
/// соединений/конфигурацию TLS), поэтому один построенный транспорт можно
/// закэшировать на время сессии приложения и раздавать клоны вызывающим
/// сторонам вместо пересборки клиента на каждый вызов (см. `app::llm`).
#[derive(Clone)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    /// Транспорт с тайм-аутом по умолчанию ([`DEFAULT_TIMEOUT`]).
    pub fn new() -> Result<Self, DataError> {
        Self::with_timeout(DEFAULT_TIMEOUT)
    }

    /// Транспорт с явным тайм-аутом запроса.
    pub fn with_timeout(timeout: Duration) -> Result<Self, DataError> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| DataError::Other(format!("не удалось создать HTTP-клиент: {e}")))?;
        Ok(Self { client })
    }
}

impl Default for ReqwestTransport {
    /// # Panics
    /// Паникует, если `reqwest::Client` не удалось собрать (практически не
    /// происходит с настройками по умолчанию — TLS-бэкенд собирается статически).
    fn default() -> Self {
        Self::new().expect("сборка HTTP-клиента по умолчанию не должна падать")
    }
}

impl HttpTransport for ReqwestTransport {
    async fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
    ) -> Result<HttpResponse, DataError> {
        let header_map = build_header_map(headers)?;

        let resp = self
            .client
            .get(url)
            .headers(header_map)
            .send()
            .await
            .map_err(|e| DataError::Transport(describe_request_error(&e)))?;

        read_response(resp).await
    }

    async fn post(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: Vec<u8>,
    ) -> Result<HttpResponse, DataError> {
        let header_map = build_header_map(headers)?;

        let resp = self
            .client
            .post(url)
            .headers(header_map)
            .body(body)
            .send()
            .await
            .map_err(|e| DataError::Transport(describe_request_error(&e)))?;

        read_response(resp).await
    }
}

/// Собрать `reqwest::HeaderMap` из пар «имя, значение» (общая часть `get`/`post`).
fn build_header_map(headers: &[(String, String)]) -> Result<reqwest::header::HeaderMap, DataError> {
    let mut header_map = reqwest::header::HeaderMap::new();
    for (name, value) in headers {
        let header_name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| DataError::Other(format!("неверное имя заголовка: {name}")))?;
        let header_value = reqwest::header::HeaderValue::from_str(value)
            .map_err(|_| DataError::Other(format!("неверное значение заголовка {name}")))?;
        header_map.insert(header_name, header_value);
    }
    Ok(header_map)
}

/// Прочитать статус и тело ответа (общая часть `get`/`post`).
async fn read_response(resp: reqwest::Response) -> Result<HttpResponse, DataError> {
    let status = resp.status().as_u16();
    let body = resp
        .bytes()
        .await
        .map_err(|e| DataError::Transport(format!("чтение тела ответа: {e}")))?
        .to_vec();
    Ok(HttpResponse { status, body })
}

/// Короткое, без секретов, описание сетевой ошибки `reqwest` (тайм-аут/сеть).
///
/// `reqwest::Error` не включает заголовки запроса в `Display`, но мы явно
/// формируем сообщение сами, чтобы не зависеть от деталей чужой реализации.
fn describe_request_error(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        format!("тайм-аут запроса: {e}")
    } else if e.is_connect() {
        format!("ошибка соединения: {e}")
    } else {
        format!("сетевая ошибка: {e}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;

    /// Один зафиксированный вызов фейкового транспорта (для проверки, что
    /// URL/заголовки/тело действительно доходят из `HttpClient` до транспорта).
    #[derive(Debug, Clone)]
    struct CapturedCall {
        url: String,
        headers: Vec<(String, String)>,
        /// `None` — вызов был `GET`; `Some(body)` — `POST` с телом `body`.
        body: Option<Vec<u8>>,
    }

    /// Фейковый транспорт: считает вызовы, запоминает их аргументы и отдаёт
    /// заранее заданную программу результатов (последний повторяется, когда
    /// программа исчерпана).
    struct FakeTransport {
        calls: AtomicU32,
        captured: Mutex<Vec<CapturedCall>>,
        program: Vec<Result<HttpResponse, DataError>>,
    }

    impl FakeTransport {
        fn new(program: Vec<Result<HttpResponse, DataError>>) -> Self {
            Self {
                calls: AtomicU32::new(0),
                captured: Mutex::new(Vec::new()),
                program,
            }
        }

        fn always(resp: HttpResponse) -> Self {
            Self::new(vec![Ok(resp)])
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
            url: &str,
            headers: &[(String, String)],
        ) -> Result<HttpResponse, DataError> {
            self.captured
                .lock()
                .expect("mutex отравлен")
                .push(CapturedCall {
                    url: url.to_owned(),
                    headers: headers.to_vec(),
                    body: None,
                });
            let i = self.calls.fetch_add(1, Ordering::SeqCst) as usize;
            self.program[i.min(self.program.len() - 1)].clone()
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

    /// Backoff без задержек — чтобы тесты не спали.
    fn no_sleep_backoff(max_retries: u32) -> Backoff {
        Backoff::new(Duration::ZERO, 1.0, Duration::ZERO, max_retries)
    }

    fn client(transport: FakeTransport, max_retries: u32) -> HttpClient<FakeTransport> {
        HttpClient::with_policy(
            transport,
            RateLimiter::finam_default(),
            no_sleep_backoff(max_retries),
        )
    }

    #[tokio::test]
    async fn request_carries_url_and_headers_to_transport() {
        let c = client(FakeTransport::always(ok_json(200, r#"{"ok":true}"#)), 3);
        let headers = vec![("Authorization".to_owned(), "Bearer secret-token".to_owned())];
        let value = c
            .get_json(
                Method::MoexTradestats,
                "https://apim.moex.com/iss/datashop/algopack/eq/tradestats.json",
                &headers,
            )
            .await
            .unwrap();
        assert_eq!(value["ok"], serde_json::Value::Bool(true));

        let calls = c.transport.captured_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].url,
            "https://apim.moex.com/iss/datashop/algopack/eq/tradestats.json"
        );
        assert_eq!(
            calls[0].headers,
            vec![("Authorization".to_owned(), "Bearer secret-token".to_owned())]
        );
    }

    #[tokio::test]
    async fn server_error_retries_then_succeeds() {
        let c = client(
            FakeTransport::new(vec![
                Ok(ok_json(503, "service unavailable")),
                Ok(ok_json(502, "bad gateway")),
                Ok(ok_json(200, r#"{"data":[1,2,3]}"#)),
            ]),
            5,
        );
        let value = c
            .get_json(Method::MoexFutoi, "https://example.invalid/futoi", &[])
            .await
            .unwrap();
        assert_eq!(value["data"], serde_json::json!([1, 2, 3]));
        assert_eq!(c.transport.calls(), 3);
    }

    #[tokio::test]
    async fn rate_limited_retries_then_succeeds() {
        let c = client(
            FakeTransport::new(vec![Ok(ok_json(429, "too many")), Ok(ok_json(200, "{}"))]),
            3,
        );
        let value = c
            .get_json(Method::MoexHi2, "https://example.invalid/hi2", &[])
            .await
            .unwrap();
        assert_eq!(value, serde_json::json!({}));
        assert_eq!(c.transport.calls(), 2);
    }

    #[tokio::test]
    async fn transient_network_error_retries_then_succeeds() {
        let c = client(
            FakeTransport::new(vec![
                Err(DataError::Transport("connection reset".into())),
                Ok(ok_json(200, "{}")),
            ]),
            3,
        );
        c.get_json(Method::MoexObstats, "https://example.invalid/obstats", &[])
            .await
            .unwrap();
        assert_eq!(c.transport.calls(), 2);
    }

    #[tokio::test]
    async fn not_found_does_not_retry() {
        let c = client(FakeTransport::always(ok_json(404, "not found")), 5);
        let err = c
            .get_json(Method::MoexCandles, "https://example.invalid/candles", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::Other(_)));
        assert!(!err.is_retryable());
        // Один вызов — без повторов на неретраябельной ошибке.
        assert_eq!(c.transport.calls(), 1);
    }

    #[tokio::test]
    async fn unauthorized_maps_to_auth_error_without_retry() {
        let c = client(FakeTransport::always(ok_json(401, "unauthorized")), 5);
        let err = c
            .get_json(Method::Llm, "https://example.invalid/llm", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::Auth(_)));
        assert!(!err.is_retryable());
        assert_eq!(c.transport.calls(), 1);

        let c = client(FakeTransport::always(ok_json(403, "forbidden")), 5);
        let err = c
            .get_json(Method::Llm, "https://example.invalid/llm", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::Auth(_)));
    }

    #[tokio::test]
    async fn gives_up_after_exhausting_retries() {
        let c = client(FakeTransport::always(ok_json(500, "boom")), 2); // 1 исходная + 2 повтора
        let err = c
            .get_json(
                Method::MoexOrderstats,
                "https://example.invalid/orderstats",
                &[],
            )
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::Transport(_)));
        assert_eq!(c.transport.calls(), 3);
    }

    #[tokio::test]
    async fn malformed_json_is_not_retryable() {
        let c = client(FakeTransport::always(ok_json(200, "not json")), 5);
        let err = c
            .get_json(Method::MoexOptions, "https://example.invalid/options", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::Other(_)));
        assert!(!err.is_retryable());
        assert_eq!(c.transport.calls(), 1);
    }

    #[tokio::test]
    async fn respects_per_method_rate_limit() {
        // Лимит в 1 запрос на метод: второй независимый вызов того же метода
        // должен упереться в лимит и (без доступных повторов) вернуть ошибку,
        // не дёргая транспорт.
        let limiter = RateLimiter::per_minute(1);
        let backoff = no_sleep_backoff(0);
        let transport = FakeTransport::always(ok_json(200, "{}"));
        let c = HttpClient::with_policy(transport, limiter, backoff);

        c.get_json(Method::MoexTradestats, "https://example.invalid/a", &[])
            .await
            .unwrap();
        assert_eq!(c.transport.calls(), 1);

        let err = c
            .get_json(Method::MoexTradestats, "https://example.invalid/b", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::RateLimited("moex_tradestats")));
        // Транспорт не вызывался второй раз — отказ произошёл на лимитере.
        assert_eq!(c.transport.calls(), 1);

        // Другой метод не затронут общим лимитом.
        c.get_json(Method::MoexFutoi, "https://example.invalid/c", &[])
            .await
            .unwrap();
        assert_eq!(c.transport.calls(), 2);
    }

    #[test]
    fn debug_output_of_response_does_not_expose_request_headers() {
        // `HttpResponse` не содержит заголовков запроса вовсе — секрет
        // Authorization физически не может попасть в его `Debug`-вывод.
        let resp = ok_json(200, "{}");
        let debug = format!("{resp:?}");
        assert!(!debug.contains("Bearer"));
        assert!(!debug.contains("Authorization"));
    }

    #[tokio::test]
    async fn post_json_carries_url_headers_and_body_to_transport() {
        let c = client(FakeTransport::always(ok_json(200, r#"{"ok":true}"#)), 3);
        let headers = vec![("Authorization".to_owned(), "Bearer secret-token".to_owned())];
        let body = serde_json::json!({"model": "x", "messages": []});
        let value = c
            .post_json(Method::Llm, "https://example.invalid/llm", &headers, &body)
            .await
            .unwrap();
        assert_eq!(value["ok"], serde_json::Value::Bool(true));

        let calls = c.transport.captured_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].url, "https://example.invalid/llm");
        assert_eq!(
            calls[0].headers,
            vec![("Authorization".to_owned(), "Bearer secret-token".to_owned())]
        );
        let sent_body: serde_json::Value =
            serde_json::from_slice(calls[0].body.as_ref().unwrap()).unwrap();
        assert_eq!(sent_body, body);
    }

    #[tokio::test]
    async fn post_json_retries_transient_errors_then_succeeds() {
        let c = client(
            FakeTransport::new(vec![
                Ok(ok_json(503, "unavailable")),
                Ok(ok_json(200, r#"{"data":1}"#)),
            ]),
            5,
        );
        let value = c
            .post_json(
                Method::Llm,
                "https://example.invalid/llm",
                &[],
                &serde_json::json!({}),
            )
            .await
            .unwrap();
        assert_eq!(value["data"], serde_json::json!(1));
        assert_eq!(c.transport.calls(), 2);
    }

    #[tokio::test]
    async fn post_json_unauthorized_does_not_retry() {
        let c = client(FakeTransport::always(ok_json(401, "unauthorized")), 5);
        let err = c
            .post_json(
                Method::Llm,
                "https://example.invalid/llm",
                &[],
                &serde_json::json!({}),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::Auth(_)));
        assert_eq!(c.transport.calls(), 1);
    }
}
