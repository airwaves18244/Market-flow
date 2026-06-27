//! Авторизация и обновление токена доступа.
//!
//! Finam Trade API v1 авторизует по API-секрету: вызов `AuthService.Auth`
//! отдаёт короткоживущий JWT (≈15 минут). «Refresh» — это повторная авторизация
//! тем же секретом до истечения срока. [`TokenManager`] кэширует текущий JWT и
//! его срок годности, обновляя его заранее (с запасом `skew`), и не зависит от
//! транспорта: саму сетевую авторизацию передают замыканием.
//!
//! Логика чистая (время — параметр `now`, UNIX-секунды), поэтому полностью
//! тестируется без сети.

use std::future::Future;

use crate::DataError;

/// Сколько секунд до истечения JWT считать его «пора обновить» (запас на сетевую
/// задержку и рассинхрон часов).
pub const DEFAULT_SKEW_SECS: i64 = 30;

/// Менеджер токена доступа: кэш JWT + срок годности + политика обновления.
#[derive(Debug, Clone, Default)]
pub struct TokenManager {
    token: Option<String>,
    /// Момент истечения JWT, UNIX-секунды UTC.
    expires_at: i64,
    /// Запас перед истечением, при котором инициируем обновление.
    skew: i64,
}

impl TokenManager {
    /// Создать менеджер с запасом обновления [`DEFAULT_SKEW_SECS`].
    pub fn new() -> Self {
        Self::with_skew(DEFAULT_SKEW_SECS)
    }

    /// Создать менеджер с заданным запасом обновления (секунды, `>= 0`).
    pub fn with_skew(skew_secs: i64) -> Self {
        Self {
            token: None,
            expires_at: 0,
            skew: skew_secs.max(0),
        }
    }

    /// Сохранить свежий JWT: `ttl_secs` — срок его жизни от `now`.
    pub fn store(&mut self, token: impl Into<String>, now: i64, ttl_secs: i64) {
        self.token = Some(token.into());
        self.expires_at = now.saturating_add(ttl_secs);
    }

    /// Нужно ли обновлять токен на момент `now` (нет токена или близко к концу).
    pub fn needs_refresh(&self, now: i64) -> bool {
        match self.token {
            None => true,
            Some(_) => now.saturating_add(self.skew) >= self.expires_at,
        }
    }

    /// Текущий действительный токен на момент `now`, иначе `None`.
    pub fn current(&self, now: i64) -> Option<&str> {
        if self.needs_refresh(now) {
            None
        } else {
            self.token.as_deref()
        }
    }

    /// Вернуть действительный токен, при необходимости обновив его через
    /// `refresh`. Замыкание возвращает пару `(jwt, ttl_secs)`.
    ///
    /// Так слой авторизации остаётся независимым от транспорта: реальная
    /// реализация дёргает gRPC `AuthService.Auth`, тест — отдаёт фиктивный JWT.
    pub async fn valid_token<F, Fut>(&mut self, now: i64, refresh: F) -> Result<String, DataError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(String, i64), DataError>>,
    {
        if self.needs_refresh(now) {
            let (token, ttl) = refresh().await?;
            if ttl <= 0 {
                return Err(DataError::Auth("получен токен с нулевым сроком".into()));
            }
            self.store(token, now, ttl);
        }
        // На этом шаге токен гарантированно есть.
        self.token
            .clone()
            .ok_or_else(|| DataError::Auth("токен отсутствует после обновления".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Минимальный исполнитель фьючерсов без внешних зависимостей: гоняет
    /// `poll` до готовности (наши фьючерсы готовы сразу, без реального I/O).
    fn block_on<F: Future>(fut: F) -> F::Output {
        use std::pin::pin;
        use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

        fn noop(_: *const ()) {}
        fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);

        let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
        let mut cx = Context::from_waker(&waker);
        let mut fut = pin!(fut);
        loop {
            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    #[test]
    fn fresh_manager_needs_refresh() {
        let tm = TokenManager::new();
        assert!(tm.needs_refresh(0));
        assert_eq!(tm.current(0), None);
    }

    #[test]
    fn stored_token_is_valid_until_skew_window() {
        let mut tm = TokenManager::with_skew(30);
        tm.store("jwt", 1_000, 900); // истекает в 1900
        assert!(!tm.needs_refresh(1_000));
        assert_eq!(tm.current(1_500), Some("jwt"));
        // за 30с до истечения — пора обновлять
        assert!(tm.needs_refresh(1_870));
        assert_eq!(tm.current(1_870), None);
    }

    #[test]
    fn valid_token_refreshes_when_needed() {
        let mut tm = TokenManager::with_skew(10);
        let mut calls = 0;
        // первый вызов — обновление
        let t1 = block_on(tm.valid_token(0, || {
            calls += 1;
            async { Ok(("jwt-1".to_string(), 600)) }
        }))
        .unwrap();
        assert_eq!(t1, "jwt-1");
        assert_eq!(calls, 1);

        // в пределах срока — без обновления (замыкание не вызывается)
        let t2 = block_on(tm.valid_token(100, || {
            calls += 1;
            async { Ok(("jwt-2".to_string(), 600)) }
        }))
        .unwrap();
        assert_eq!(t2, "jwt-1");
        assert_eq!(calls, 1);

        // у края срока — снова обновление
        let t3 = block_on(tm.valid_token(595, || {
            calls += 1;
            async { Ok(("jwt-3".to_string(), 600)) }
        }))
        .unwrap();
        assert_eq!(t3, "jwt-3");
        assert_eq!(calls, 2);
    }

    #[test]
    fn rejects_zero_ttl_token() {
        let mut tm = TokenManager::new();
        let err = block_on(tm.valid_token(0, || async { Ok(("jwt".to_string(), 0)) }));
        assert!(err.is_err());
    }

    #[test]
    fn propagates_refresh_error() {
        let mut tm = TokenManager::new();
        let err = block_on(tm.valid_token(0, || async { Err(DataError::Auth("сеть".into())) }));
        assert!(matches!(err, Err(DataError::Auth(_))));
    }
}
