//! Сетевой gRPC-слой Finam Trade API (фича `grpc`).
//!
//! Здесь собирается «грязный» сетевой обмен поверх сгенерированных стабов
//! ([`finam_proto`]) и чистых примитивов крейта: [`TokenState`] решает, когда
//! обновлять JWT, [`RateLimiter`] держит лимит метода `Auth`, [`Backoff`] задаёт
//! паузы при транзиентных сбоях, [`SecretStore`] отдаёт долгоживущий секрет.
//!
//! Оркестрация ([`AuthManager`]) отделена от транспорта ([`AuthTransport`]):
//! боевой транспорт — gRPC ([`GrpcAuthTransport`]), а логика «переиспользовать
//! токен / обновить / повторить при сбое» тестируется на фейковом транспорте без
//! сети. Сам обмен `AuthService.Auth` интеграционно проверяется при наличии
//! реального секрета (в CI выключено).

use std::future::Future;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::backoff::jitter_fraction;
use crate::{Backoff, DataError, Method, RateLimiter, SecretStore, TokenState};

/// Свежевыданный access-token и его время жизни.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthToken {
    /// JWT access-token.
    pub token: String,
    /// Сколько токен действителен с момента выдачи.
    pub ttl: Duration,
}

/// Транспорт обмена секрета на токен. Абстрагирует сеть, чтобы оркестрацию
/// [`AuthManager`] можно было тестировать детерминированно.
pub trait AuthTransport: Send + Sync {
    /// Обменять долгоживущий `secret` на короткоживущий [`AuthToken`].
    fn authenticate(
        &self,
        secret: &str,
    ) -> impl Future<Output = Result<AuthToken, DataError>> + Send;
}

/// Менеджер авторизации: выдаёт действующий JWT, обновляя его при необходимости.
///
/// Потокобезопасен: текущий [`TokenState`] под синхронным мьютексом, который
/// **не** удерживается через `await`. Обмен секрета на токен сериализуется
/// отдельным асинхронным `refresh_lock` (single-flight, R-7): при протухшем
/// токене N параллельных вызовов не устраивают залп из N обменов `Auth` —
/// обмен делает только первый взявший лок, остальные после ожидания
/// перечитывают [`TokenState`] и переиспользуют уже свежий токен.
pub struct AuthManager<T: AuthTransport, S: SecretStore> {
    transport: T,
    secret: S,
    state: Mutex<TokenState>,
    /// Сериализует секцию обмена токена (single-flight). Асинхронный мьютекс —
    /// его можно держать через `await` самого обмена, в отличие от `state`.
    refresh_lock: tokio::sync::Mutex<()>,
    limiter: RateLimiter,
    backoff: Backoff,
}

impl<T: AuthTransport, S: SecretStore> AuthManager<T, S> {
    /// Менеджер с разумными умолчаниями (лимит Finam, backoff по умолчанию,
    /// стандартный запас на упреждающий refresh в [`TokenState`]).
    pub fn new(transport: T, secret: S) -> Self {
        Self::with_policy(
            transport,
            secret,
            TokenState::new(),
            RateLimiter::finam_default(),
            Backoff::finam_default(),
        )
    }

    /// Менеджер с явными политиками (для тестов и тонкой настройки).
    pub fn with_policy(
        transport: T,
        secret: S,
        state: TokenState,
        limiter: RateLimiter,
        backoff: Backoff,
    ) -> Self {
        Self {
            transport,
            secret,
            state: Mutex::new(state),
            refresh_lock: tokio::sync::Mutex::new(()),
            limiter,
            backoff,
        }
    }

    /// Действующий access-token: переиспользует текущий, пока он не подлежит
    /// упреждающему обновлению, иначе обновляет через транспорт.
    pub async fn access_token(&self) -> Result<String, DataError> {
        let now = Instant::now();
        {
            let st = self.state.lock().expect("token-state mutex отравлен");
            if !st.needs_refresh(now) {
                if let Some(tok) = st.valid_token(now) {
                    return Ok(tok.to_owned());
                }
            }
        }
        self.refresh().await
    }

    /// Обновить токен под single-flight (R-7): взять `refresh_lock`, затем ещё
    /// раз проверить [`TokenState`] — пока ждали лок, обмен мог уже сделать
    /// другой вызывающий, и тогда переиспользуем его свежий токен без нового
    /// обмена `Auth`. Реальный обмен ([`do_refresh`]) выполняет лишь тот, кто
    /// увидел кэш всё ещё протухшим.
    ///
    /// После [`invalidate`] кэш пуст, поэтому повторная проверка ничего не
    /// вернёт и обмен гарантированно произойдёт — это опора [`force_refresh`].
    ///
    /// [`do_refresh`]: Self::do_refresh
    /// [`invalidate`]: Self::invalidate
    /// [`force_refresh`]: Self::force_refresh
    pub async fn refresh(&self) -> Result<String, DataError> {
        let _guard = self.refresh_lock.lock().await;
        // Повторная проверка под локом: возможно, обмен уже сделал сосед.
        {
            let now = Instant::now();
            let st = self.state.lock().expect("token-state mutex отравлен");
            if !st.needs_refresh(now) {
                if let Some(tok) = st.valid_token(now) {
                    return Ok(tok.to_owned());
                }
            }
        }
        self.do_refresh().await
    }

    /// Собственно обмен секрета на токен (внутри single-flight [`refresh`]):
    /// уважает лимит метода `Auth`, повторяет транзиентные сбои с [`Backoff`],
    /// на невосстановимой ошибке сбрасывает кэш токена.
    ///
    /// [`refresh`]: Self::refresh
    async fn do_refresh(&self) -> Result<String, DataError> {
        let secret = self
            .secret
            .load()?
            .ok_or_else(|| DataError::Auth("секрет API не задан".to_owned()))?;

        let mut attempt = 0u32;
        loop {
            // Лимит метода Auth — раздельный per-method счётчик.
            if let Err(e) = self.limiter.try_acquire(Method::Auth) {
                if self.backoff.is_exhausted(attempt) {
                    return Err(e);
                }
                self.sleep_for(attempt).await;
                attempt += 1;
                continue;
            }

            match self.transport.authenticate(&secret).await {
                Ok(AuthToken { token, ttl }) => {
                    let mut st = self.state.lock().expect("token-state mutex отравлен");
                    st.set(token.clone(), ttl, Instant::now());
                    return Ok(token);
                }
                Err(e) if e.is_retryable() && !self.backoff.is_exhausted(attempt) => {
                    self.sleep_for(attempt).await;
                    attempt += 1;
                }
                Err(e) => {
                    // Невосстановимая ошибка (auth/прочее) — сбрасываем токен.
                    self.state
                        .lock()
                        .expect("token-state mutex отравлен")
                        .clear();
                    return Err(e);
                }
            }
        }
    }

    /// Сбросить закэшированный токен: следующий [`access_token`] сходит за новым.
    ///
    /// Нужен, когда сервер ответил `UNAUTHENTICATED` до истечения skew-окна
    /// (например, токен инвалидирован на стороне Finam): формально «годный» по
    /// сроку токен из [`TokenState`] больше нельзя переиспользовать.
    ///
    /// [`access_token`]: Self::access_token
    pub fn invalidate(&self) {
        self.state
            .lock()
            .expect("token-state mutex отравлен")
            .clear();
    }

    /// Форсированно получить новый токен, минуя кэш: сброс [`TokenState`] +
    /// один обмен через транспорт (по образцу [`refresh`]).
    ///
    /// Вызывается после отказа авторизации на боевом вызове — ровно один раз на
    /// вызов (защита от бесконечного цикла re-auth — на стороне вызывающего).
    ///
    /// [`refresh`]: Self::refresh
    pub async fn force_refresh(&self) -> Result<String, DataError> {
        // Сбрасываем кэш до обмена: даже если refresh упадёт, протухший токен
        // не отдастся как «действующий» из-под конкурентного access_token.
        self.invalidate();
        self.refresh().await
    }

    /// Снимок: есть ли сейчас действующий (не требующий refresh) токен.
    pub fn has_fresh_token(&self) -> bool {
        let now = Instant::now();
        let st = self.state.lock().expect("token-state mutex отравлен");
        !st.needs_refresh(now) && st.valid_token(now).is_some()
    }

    async fn sleep_for(&self, attempt: u32) {
        let delay = self.backoff.delay_with_jitter(attempt, jitter_fraction());
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
    }
}

/// Время жизни токена из меток `created_at`/`expires_at` (в секундах эпохи).
///
/// Чистый помощник для [`GrpcAuthTransport`]: TTL = `expires_at − created_at`,
/// не отрицателен. Если `expires_at <= created_at`, вернёт [`Duration::ZERO`]
/// (токен считается просроченным сразу — вызовет немедленный refresh).
pub fn ttl_from_epoch_secs(created_secs: i64, expires_secs: i64) -> Duration {
    let secs = expires_secs.saturating_sub(created_secs).max(0);
    Duration::from_secs(secs as u64)
}

/// Боевой gRPC-транспорт: обмен по `AuthService.Auth` + уточнение TTL через
/// `AuthService.TokenDetails`.
pub struct GrpcAuthTransport {
    endpoint: String,
    source_app_id: String,
}

impl GrpcAuthTransport {
    /// Транспорт к стандартному эндпоинту Finam ([`finam_proto::ENDPOINT`]).
    pub fn new() -> Self {
        Self {
            endpoint: finam_proto::ENDPOINT.to_owned(),
            source_app_id: String::new(),
        }
    }

    /// Транспорт к произвольному эндпоинту (для стенда/прокси).
    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            source_app_id: String::new(),
        }
    }

    /// Указать `source_app_id`, которым подписываются auth-запросы.
    pub fn with_source_app_id(mut self, id: impl Into<String>) -> Self {
        self.source_app_id = id.into();
        self
    }
}

impl Default for GrpcAuthTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthTransport for GrpcAuthTransport {
    async fn authenticate(&self, secret: &str) -> Result<AuthToken, DataError> {
        use finam_proto::auth::auth_service_client::AuthServiceClient;
        use finam_proto::auth::{AuthRequest, TokenDetailsRequest};

        let channel = crate::market::build_endpoint(&self.endpoint)?
            .connect()
            .await
            .map_err(|e| DataError::Transport(format!("подключение: {e}")))?;

        let mut client = AuthServiceClient::new(channel);

        let token = client
            .auth(AuthRequest {
                secret: secret.to_owned(),
                source_app_id: self.source_app_id.clone(),
            })
            .await
            .map_err(status_to_error)?
            .into_inner()
            .token;

        // TTL берём из деталей токена (сроки выдачи/истечения).
        let details = client
            .token_details(TokenDetailsRequest {
                token: token.clone(),
            })
            .await
            .map_err(status_to_error)?
            .into_inner();

        let created = details.created_at.map(|t| t.seconds).unwrap_or(0);
        let expires = details.expires_at.map(|t| t.seconds).unwrap_or(0);
        let ttl = ttl_from_epoch_secs(created, expires);

        Ok(AuthToken { token, ttl })
    }
}

/// Маппинг `tonic::Status` в [`DataError`] с учётом ретраябельности.
fn status_to_error(status: tonic::Status) -> DataError {
    use tonic::Code;
    match status.code() {
        Code::Unauthenticated | Code::PermissionDenied => {
            DataError::Auth(status.message().to_owned())
        }
        // Транзиентные коды → транспортная (ретраябельная) ошибка.
        Code::Unavailable | Code::DeadlineExceeded | Code::Aborted | Code::ResourceExhausted => {
            DataError::Transport(format!("{}: {}", status.code(), status.message()))
        }
        other => DataError::Other(format!("{}: {}", other, status.message())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemSecretStore;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    /// Фейковый транспорт: считает вызовы и отдаёт заранее заданную программу
    /// результатов (последний повторяется, когда программа исчерпана).
    struct FakeTransport {
        calls: AtomicU32,
        program: Vec<Result<AuthToken, DataError>>,
    }

    impl FakeTransport {
        fn new(program: Vec<Result<AuthToken, DataError>>) -> Self {
            Self {
                calls: AtomicU32::new(0),
                program,
            }
        }
        fn always_ok(token: &str, ttl: Duration) -> Self {
            Self::new(vec![Ok(AuthToken {
                token: token.to_owned(),
                ttl,
            })])
        }
        fn calls(&self) -> u32 {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl AuthTransport for FakeTransport {
        async fn authenticate(&self, _secret: &str) -> Result<AuthToken, DataError> {
            let i = self.calls.fetch_add(1, Ordering::SeqCst) as usize;
            self.program[i.min(self.program.len() - 1)].clone()
        }
    }

    /// Backoff без задержек — чтобы тесты не спали.
    fn no_sleep_backoff(max_retries: u32) -> Backoff {
        Backoff::new(Duration::ZERO, 1.0, Duration::ZERO, max_retries)
    }

    fn manager(
        transport: FakeTransport,
        secret: Option<&str>,
    ) -> AuthManager<FakeTransport, MemSecretStore> {
        let store = match secret {
            Some(s) => MemSecretStore::with_secret(s),
            None => MemSecretStore::new(),
        };
        AuthManager::with_policy(
            transport,
            store,
            TokenState::new(),
            RateLimiter::finam_default(),
            no_sleep_backoff(5),
        )
    }

    #[tokio::test]
    async fn fetches_then_reuses_valid_token() {
        let m = manager(
            FakeTransport::always_ok("jwt-1", Duration::from_secs(900)),
            Some("api-secret"),
        );
        assert_eq!(m.access_token().await.unwrap(), "jwt-1");
        // Второй вызов берёт кэш — транспорт не дёргается повторно.
        assert_eq!(m.access_token().await.unwrap(), "jwt-1");
        assert_eq!(m.transport.calls(), 1);
        assert!(m.has_fresh_token());
    }

    #[tokio::test]
    async fn refresh_when_ttl_within_skew() {
        // TTL меньше запаса refresh (60с) → токен сразу «пора обновить».
        let m = manager(
            FakeTransport::always_ok("jwt-short", Duration::from_secs(10)),
            Some("api-secret"),
        );
        assert_eq!(m.access_token().await.unwrap(), "jwt-short");
        // Каждый запрос видит needs_refresh=true и обновляет.
        m.access_token().await.unwrap();
        assert_eq!(m.transport.calls(), 2);
        assert!(!m.has_fresh_token());
    }

    #[tokio::test]
    async fn missing_secret_is_auth_error() {
        let m = manager(
            FakeTransport::always_ok("x", Duration::from_secs(900)),
            None,
        );
        let err = m.access_token().await.unwrap_err();
        assert!(matches!(err, DataError::Auth(_)));
        assert_eq!(m.transport.calls(), 0);
    }

    #[tokio::test]
    async fn retries_transient_then_succeeds() {
        let m = manager(
            FakeTransport::new(vec![
                Err(DataError::Transport("reset".into())),
                Err(DataError::MaintenanceWindow),
                Ok(AuthToken {
                    token: "jwt-ok".into(),
                    ttl: Duration::from_secs(900),
                }),
            ]),
            Some("api-secret"),
        );
        assert_eq!(m.access_token().await.unwrap(), "jwt-ok");
        assert_eq!(m.transport.calls(), 3);
    }

    #[tokio::test]
    async fn does_not_retry_auth_error() {
        let m = manager(
            FakeTransport::new(vec![
                Err(DataError::Auth("bad secret".into())),
                Ok(AuthToken {
                    token: "never".into(),
                    ttl: Duration::from_secs(900),
                }),
            ]),
            Some("api-secret"),
        );
        let err = m.access_token().await.unwrap_err();
        assert!(matches!(err, DataError::Auth(_)));
        // Невосстановимая ошибка → один вызов, без повторов.
        assert_eq!(m.transport.calls(), 1);
    }

    #[tokio::test]
    async fn gives_up_after_exhausting_retries() {
        let transport = FakeTransport::new(vec![Err(DataError::Transport("down".into()))]);
        let m = AuthManager::with_policy(
            transport,
            MemSecretStore::with_secret("api-secret"),
            TokenState::new(),
            RateLimiter::finam_default(),
            no_sleep_backoff(2), // 1 исходная + 2 повтора = 3 попытки
        );
        let err = m.access_token().await.unwrap_err();
        assert!(matches!(err, DataError::Transport(_)));
        assert_eq!(m.transport.calls(), 3);
    }

    #[tokio::test]
    async fn concurrent_callers_share_manager() {
        let m = Arc::new(manager(
            FakeTransport::always_ok("jwt-shared", Duration::from_secs(900)),
            Some("api-secret"),
        ));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let m = Arc::clone(&m);
            handles.push(tokio::spawn(async move { m.access_token().await }));
        }
        for h in handles {
            assert_eq!(h.await.unwrap().unwrap(), "jwt-shared");
        }
    }

    /// R-7: при протухшем токене N параллельных `access_token` не должны
    /// устраивать залп из N обменов `Auth` — single-flight сводит их к одному.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn refresh_is_single_flight_under_concurrency() {
        // Транспорт с задержкой: пока первый взявший `refresh_lock` делает обмен,
        // остальные девять успевают увидеть протухший кэш и встать в очередь на
        // лок; после обмена они перечитают уже свежий токен и обмен не повторят.
        struct SlowTransport {
            calls: Arc<AtomicU32>,
        }
        impl AuthTransport for SlowTransport {
            async fn authenticate(&self, _secret: &str) -> Result<AuthToken, DataError> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(50)).await;
                Ok(AuthToken {
                    token: "jwt-shared".to_owned(),
                    ttl: Duration::from_secs(900),
                })
            }
        }

        let calls = Arc::new(AtomicU32::new(0));
        let m = Arc::new(AuthManager::with_policy(
            SlowTransport {
                calls: Arc::clone(&calls),
            },
            MemSecretStore::with_secret("api-secret"),
            TokenState::new(), // токена ещё нет → все вызовы увидят needs_refresh
            RateLimiter::finam_default(),
            no_sleep_backoff(5),
        ));

        let mut handles = Vec::new();
        for _ in 0..10 {
            let m = Arc::clone(&m);
            handles.push(tokio::spawn(async move { m.access_token().await }));
        }
        for h in handles {
            assert_eq!(h.await.unwrap().unwrap(), "jwt-shared");
        }
        // Ровно один обмен Auth на десять конкурентных вызовов.
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn ttl_from_epoch_is_difference_and_nonnegative() {
        assert_eq!(ttl_from_epoch_secs(1_000, 1_900), Duration::from_secs(900));
        // Истёкший/перевёрнутый диапазон → ноль (немедленный refresh).
        assert_eq!(ttl_from_epoch_secs(2_000, 1_000), Duration::ZERO);
        assert_eq!(ttl_from_epoch_secs(5, 5), Duration::ZERO);
    }
}
