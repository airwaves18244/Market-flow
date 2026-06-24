//! Ограничение частоты запросов (§ 0.3): ≤200 запросов/мин на метод.
//!
//! Тонкая обёртка над `governor`. Реальные вызовы ждут окна через
//! [`Limiter::acquire`]; неблокирующая [`Limiter::try_acquire`] удобна в тестах.

use std::num::NonZeroU32;

use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};

/// Лимит запросов в минуту по одному методу.
pub struct Limiter(DefaultDirectRateLimiter);

impl Limiter {
    /// Лимитер на `max` запросов в минуту (минимум 1).
    pub fn per_minute(max: u32) -> Self {
        let quota = Quota::per_minute(NonZeroU32::new(max.max(1)).expect("max >= 1"));
        Self(RateLimiter::direct(quota))
    }

    /// Дождаться доступного окна (асинхронно).
    pub async fn acquire(&self) {
        self.0.until_ready().await;
    }

    /// Попытаться занять окно без ожидания. `true` — разрешено сейчас.
    pub fn try_acquire(&self) -> bool {
        self.0.check().is_ok()
    }
}

/// Лимитеры на методы рыночных данных (auth-лимитер живёт в `AuthManager`).
pub struct Limiters {
    pub assets: Limiter,
    pub bars: Limiter,
    pub quote: Limiter,
    pub trades: Limiter,
}

impl Default for Limiters {
    fn default() -> Self {
        /// Лимит Finam Trade API на метод.
        const PER_METHOD: u32 = 200;
        Self {
            assets: Limiter::per_minute(PER_METHOD),
            bars: Limiter::per_minute(PER_METHOD),
            quote: Limiter::per_minute(PER_METHOD),
            trades: Limiter::per_minute(PER_METHOD),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_acquire_allows_burst_then_blocks() {
        let l = Limiter::per_minute(2); // burst = 2
        assert!(l.try_acquire());
        assert!(l.try_acquire());
        assert!(!l.try_acquire()); // третий — за пределами окна
    }

    #[test]
    fn zero_is_clamped_to_one() {
        let l = Limiter::per_minute(0);
        assert!(l.try_acquire());
        assert!(!l.try_acquire());
    }
}
