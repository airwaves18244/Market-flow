//! Состояние авторизации: учёт короткоживущего JWT и решение о refresh.
//!
//! Finam Trade API устроен двухуровнево: долгоживущий API-секрет (хранится в
//! ОС-keyring) обменивается через `AuthService` на короткоживущий JWT
//! access-token (порядка 15 минут). Запросы подписываются именно JWT, поэтому
//! его нужно обновлять заранее — до фактического истечения, с запасом
//! (skew), чтобы не словить отказ на границе.
//!
//! Здесь — чистая, без сети и кросс-платформенно тестируемая логика: что за
//! токен сейчас, годен ли он и пора ли его обновлять. Сам сетевой обмен
//! (`AuthService.Auth`) подключается в фазе интеграции API; моменты времени
//! подаются как [`Instant`], что делает поведение детерминированным в тестах.

use std::time::{Duration, Instant};

/// Запас перед истечением, при котором токен считается «пора обновить».
///
/// Покрывает дрейф часов и сетевую задержку самого обмена на новый токен.
pub const DEFAULT_REFRESH_SKEW: Duration = Duration::from_secs(60);

/// Состояние текущего JWT access-token и его срока годности.
///
/// Не хранит долгоживущий API-секрет (он живёт в keyring) — только эфемерный
/// токен и момент истечения.
#[derive(Debug, Clone, Default)]
pub struct TokenState {
    token: Option<String>,
    expires_at: Option<Instant>,
    /// Запас до истечения, при котором требуется упреждающий refresh.
    skew: Duration,
}

impl TokenState {
    /// Пустое состояние с запасом по умолчанию ([`DEFAULT_REFRESH_SKEW`]).
    pub fn new() -> Self {
        Self {
            token: None,
            expires_at: None,
            skew: DEFAULT_REFRESH_SKEW,
        }
    }

    /// Пустое состояние с заданным запасом перед истечением.
    pub fn with_skew(skew: Duration) -> Self {
        Self {
            token: None,
            expires_at: None,
            skew,
        }
    }

    /// Запомнить свежий токен, действительный `ttl` начиная с `now`.
    ///
    /// Пустой `token` трактуется как сброс — состояние становится «нет токена».
    pub fn set(&mut self, token: impl Into<String>, ttl: Duration, now: Instant) {
        let token = token.into();
        if token.is_empty() {
            self.clear();
            return;
        }
        self.expires_at = Some(now + ttl);
        self.token = Some(token);
    }

    /// Сбросить состояние (например, после ошибки авторизации).
    pub fn clear(&mut self) {
        self.token = None;
        self.expires_at = None;
    }

    /// Действующий токен на момент `now`, если он есть и ещё не истёк.
    ///
    /// «Истёк» здесь — буквально по сроку, без учёта запаса: токен может быть
    /// формально годен, но уже требовать обновления (см. [`needs_refresh`]).
    ///
    /// [`needs_refresh`]: Self::needs_refresh
    pub fn valid_token(&self, now: Instant) -> Option<&str> {
        match self.expires_at {
            Some(exp) if now < exp => self.token.as_deref(),
            _ => None,
        }
    }

    /// Нужно ли обновлять токен на момент `now`.
    ///
    /// `true`, если токена нет либо до истечения осталось не больше запаса
    /// (`now + skew >= expires_at`).
    pub fn needs_refresh(&self, now: Instant) -> bool {
        match self.expires_at {
            Some(exp) => now + self.skew >= exp,
            None => true,
        }
    }

    /// Сколько осталось до истечения на момент `now` (без учёта запаса).
    ///
    /// `None`, если токена нет; `Some(Duration::ZERO)`, если уже истёк.
    pub fn time_to_expiry(&self, now: Instant) -> Option<Duration> {
        self.expires_at
            .map(|exp| exp.saturating_duration_since(now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ttl(secs: u64) -> Duration {
        Duration::from_secs(secs)
    }

    #[test]
    fn fresh_state_needs_refresh_and_has_no_token() {
        let st = TokenState::new();
        let now = Instant::now();
        assert!(st.needs_refresh(now));
        assert_eq!(st.valid_token(now), None);
        assert_eq!(st.time_to_expiry(now), None);
    }

    #[test]
    fn set_token_is_valid_within_ttl() {
        let mut st = TokenState::with_skew(ttl(60));
        let t0 = Instant::now();
        st.set("jwt-abc", ttl(900), t0);

        // Сразу после выдачи токен годен и обновлять рано.
        assert_eq!(st.valid_token(t0), Some("jwt-abc"));
        assert!(!st.needs_refresh(t0));
        assert_eq!(st.time_to_expiry(t0), Some(ttl(900)));
    }

    #[test]
    fn needs_refresh_inside_skew_window() {
        let mut st = TokenState::with_skew(ttl(60));
        let t0 = Instant::now();
        st.set("jwt", ttl(900), t0);

        // За 61с до истечения — ещё рано.
        let before = t0 + ttl(900) - ttl(61);
        assert!(!st.needs_refresh(before));

        // Ровно на границе запаса (now + skew == expires_at) — уже пора.
        let edge = t0 + ttl(900) - ttl(60);
        assert!(st.needs_refresh(edge));
        // Но токен формально ещё годен.
        assert_eq!(st.valid_token(edge), Some("jwt"));
    }

    #[test]
    fn expired_token_is_not_valid() {
        let mut st = TokenState::with_skew(ttl(60));
        let t0 = Instant::now();
        st.set("jwt", ttl(900), t0);

        let after = t0 + ttl(901);
        assert_eq!(st.valid_token(after), None);
        assert!(st.needs_refresh(after));
        assert_eq!(st.time_to_expiry(after), Some(Duration::ZERO));
    }

    #[test]
    fn empty_token_clears_state() {
        let mut st = TokenState::new();
        let t0 = Instant::now();
        st.set("jwt", ttl(900), t0);
        st.set("", ttl(900), t0);
        assert_eq!(st.valid_token(t0), None);
        assert!(st.needs_refresh(t0));
    }

    #[test]
    fn clear_resets_after_auth_error() {
        let mut st = TokenState::new();
        let t0 = Instant::now();
        st.set("jwt", ttl(900), t0);
        st.clear();
        assert_eq!(st.valid_token(t0), None);
        assert!(st.needs_refresh(t0));
    }
}
