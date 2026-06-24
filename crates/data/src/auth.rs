//! Состояние авторизации: кэш JWT и решение о его обновлении.
//!
//! Сетевой вызов `AuthService` живёт в [`crate::client`]; здесь — чистая логика
//! «пора ли обновлять токен», тестируемая с инъекцией времени.

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
