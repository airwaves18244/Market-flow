//! Авторизация: кэш JWT, решение об обновлении и менеджер refresh.
//!
//! [`TokenCache`] — чистая логика «пора ли обновлять токен» (тестируется с
//! инъекцией времени). [`AuthManager`] добавляет сетевой вызов `AuthService` и
//! отдаёт актуальный токен; он разделяется (`Arc`) между унарными и стрим-
//! методами клиента.

use tokio::sync::RwLock;
use tonic::transport::Channel;

use finam_proto::{auth, AuthServiceClient};

use crate::ratelimit::Limiter;
use crate::{map_status, now_unix, DataError};

/// Запас по времени до истечения JWT, при котором инициируется refresh.
const REFRESH_MARGIN_SECS: i64 = 60;

/// Менеджер авторизации: хранит секрет, кэширует JWT и обновляет его.
pub struct AuthManager {
    client: AuthServiceClient<Channel>,
    secret: String,
    source_app_id: String,
    cache: RwLock<TokenCache>,
    limiter: Limiter,
}

impl AuthManager {
    /// Создать менеджер поверх клиента `AuthService`.
    pub fn new(
        client: AuthServiceClient<Channel>,
        secret: impl Into<String>,
        source_app_id: impl Into<String>,
    ) -> Self {
        Self {
            client,
            secret: secret.into(),
            source_app_id: source_app_id.into(),
            cache: RwLock::new(TokenCache::new()),
            limiter: Limiter::per_minute(200),
        }
    }

    /// Обменять секрет на свежий JWT и узнать срок его действия.
    pub async fn refresh(&self) -> Result<(), DataError> {
        self.limiter.acquire().await;
        let mut client = self.client.clone();
        let token = client
            .auth(auth::AuthRequest {
                secret: self.secret.clone(),
                source_app_id: self.source_app_id.clone(),
            })
            .await
            .map_err(map_status)?
            .into_inner()
            .token;

        let details = client
            .token_details(auth::TokenDetailsRequest {
                token: token.clone(),
            })
            .await
            .map_err(map_status)?
            .into_inner();
        let expires_at = details
            .expires_at
            .map(|t| t.seconds)
            .unwrap_or_else(|| now_unix() + 600);

        tracing::debug!(expires_at, "обновлён JWT Finam");
        self.cache.write().await.set(token, expires_at);
        Ok(())
    }

    /// Вернуть актуальный токен, обновив его при необходимости.
    pub async fn token(&self) -> Result<String, DataError> {
        if self
            .cache
            .read()
            .await
            .needs_refresh(now_unix(), REFRESH_MARGIN_SECS)
        {
            self.refresh().await?;
        }
        self.cache
            .read()
            .await
            .token()
            .map(str::to_string)
            .ok_or_else(|| DataError::Auth("нет токена после refresh".into()))
    }
}

/// Кэш JWT-токена с моментом истечения (UNIX-секунды UTC).
#[derive(Debug, Default, Clone)]
pub struct TokenCache {
    jwt: Option<String>,
    expires_at: i64,
}

impl TokenCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Сохранить новый токен и его время истечения.
    pub fn set(&mut self, jwt: String, expires_at: i64) {
        self.jwt = Some(jwt);
        self.expires_at = expires_at;
    }

    /// Текущий токен, если есть.
    pub fn token(&self) -> Option<&str> {
        self.jwt.as_deref()
    }

    /// Нужно ли обновить токен на момент `now`. Истиной считается отсутствие
    /// токена или приближение к истечению ближе чем на `margin_secs`.
    pub fn needs_refresh(&self, now: i64, margin_secs: i64) -> bool {
        match self.jwt {
            None => true,
            Some(_) => now + margin_secs >= self.expires_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_cache_always_needs_refresh() {
        let c = TokenCache::new();
        assert!(c.needs_refresh(0, 60));
        assert!(c.token().is_none());
    }

    #[test]
    fn fresh_token_does_not_need_refresh() {
        let mut c = TokenCache::new();
        c.set("jwt".into(), 1000);
        assert!(!c.needs_refresh(800, 60)); // 800+60=860 < 1000
        assert_eq!(c.token(), Some("jwt"));
    }

    #[test]
    fn near_expiry_within_margin_needs_refresh() {
        let mut c = TokenCache::new();
        c.set("jwt".into(), 1000);
        assert!(c.needs_refresh(950, 60)); // 950+60=1010 >= 1000
        assert!(c.needs_refresh(1000, 0)); // ровно на истечении
    }
}
