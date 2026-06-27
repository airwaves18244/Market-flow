//! Экспоненциальный backoff для повторов запросов.
//!
//! Пара к [`RateLimiter`](crate::RateLimiter) и [`TokenState`](crate::TokenState):
//! ограничитель решает, *можно* ли слать запрос, а backoff — *как долго ждать*
//! перед повтором после транзиентной ошибки (обрыв стрима, техокно, локальный
//! отказ rate-limit). Сами повторы и засыпание подключаются в фазе интеграции
//! API (нужен async-runtime); здесь — чистый, детерминированный расчёт задержек.
//!
//! Классификация ретраябельности ошибок — в [`DataError::is_retryable`].

use std::time::Duration;

use crate::DataError;

/// Политика экспоненциального backoff с верхним пределом и числом попыток.
#[derive(Debug, Clone, Copy)]
pub struct Backoff {
    /// Базовая задержка (для попытки №0).
    base: Duration,
    /// Множитель роста между попытками.
    factor: f64,
    /// Верхний предел одной задержки.
    max_delay: Duration,
    /// Максимум повторов (не считая исходной попытки).
    max_retries: u32,
}

impl Backoff {
    /// Политика с явными параметрами.
    ///
    /// # Panics
    /// Паникует при `factor < 1.0` — иначе задержки не растут и backoff теряет
    /// смысл.
    pub fn new(base: Duration, factor: f64, max_delay: Duration, max_retries: u32) -> Self {
        assert!(factor >= 1.0, "множитель backoff должен быть ≥ 1.0");
        Self {
            base,
            factor,
            max_delay,
            max_retries,
        }
    }

    /// Разумная политика по умолчанию: 500мс × 2, потолок 30с, до 5 повторов.
    pub fn finam_default() -> Self {
        Self::new(Duration::from_millis(500), 2.0, Duration::from_secs(30), 5)
    }

    /// Максимум повторов (не считая исходной попытки).
    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }

    /// Исчерпаны ли повторы: `attempt` — номер уже выполненной попытки (с 0).
    pub fn is_exhausted(&self, attempt: u32) -> bool {
        attempt >= self.max_retries
    }

    /// Задержка перед попыткой `attempt` (с 0): `base · factor^attempt`,
    /// ограниченная сверху `max_delay`.
    pub fn delay(&self, attempt: u32) -> Duration {
        let raw = self.base.as_secs_f64() * self.factor.powi(attempt as i32);
        // `raw` может стать `inf` при большом `attempt`; ограничиваем потолком
        // *до* конвертации в `Duration`, чтобы не словить панику на бесконечности.
        let capped = raw.min(self.max_delay.as_secs_f64()).max(0.0);
        Duration::from_secs_f64(capped)
    }

    /// Задержка с «полным джиттером»: равномерно в `[0, delay(attempt)]`.
    ///
    /// Источник случайности — снаружи: `rand_fraction` ∈ `[0, 1)` (зажимается в
    /// диапазон). Так расчёт остаётся чистым и детерминированно тестируемым, а
    /// размазывание повторов (анти-«стадо») — на стороне вызывающего.
    pub fn delay_with_jitter(&self, attempt: u32, rand_fraction: f64) -> Duration {
        let f = rand_fraction.clamp(0.0, 1.0);
        Duration::from_secs_f64(self.delay(attempt).as_secs_f64() * f)
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::finam_default()
    }
}

impl DataError {
    /// Транзиентна ли ошибка — имеет ли смысл повтор с backoff.
    ///
    /// Повторяемы сетевые сбои, техническое окно и локальный rate-limit;
    /// ошибки авторизации и прочие — нет (требуют re-auth или вмешательства).
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            DataError::Transport(_) | DataError::MaintenanceWindow | DataError::RateLimited(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_grows_exponentially_from_base() {
        let b = Backoff::new(Duration::from_millis(100), 2.0, Duration::from_secs(60), 5);
        assert_eq!(b.delay(0), Duration::from_millis(100));
        assert_eq!(b.delay(1), Duration::from_millis(200));
        assert_eq!(b.delay(2), Duration::from_millis(400));
        assert_eq!(b.delay(3), Duration::from_millis(800));
    }

    #[test]
    fn delay_is_capped_at_max() {
        let b = Backoff::new(Duration::from_secs(1), 2.0, Duration::from_secs(10), 20);
        // 1,2,4,8,10(cap),10,...
        assert_eq!(b.delay(3), Duration::from_secs(8));
        assert_eq!(b.delay(4), Duration::from_secs(10));
        assert_eq!(b.delay(5), Duration::from_secs(10));
        // Большой показатель не паникует (raw → inf), возвращается потолок.
        assert_eq!(b.delay(1000), Duration::from_secs(10));
    }

    #[test]
    fn jitter_scales_within_zero_and_full_delay() {
        let b = Backoff::new(Duration::from_secs(2), 2.0, Duration::from_secs(60), 5);
        let full = b.delay(2); // 8с
        assert_eq!(b.delay_with_jitter(2, 0.0), Duration::ZERO);
        assert_eq!(b.delay_with_jitter(2, 1.0), full);
        assert_eq!(b.delay_with_jitter(2, 0.5), full / 2);
        // Выход за диапазон зажимается.
        assert_eq!(b.delay_with_jitter(2, 2.0), full);
        assert_eq!(b.delay_with_jitter(2, -1.0), Duration::ZERO);
    }

    #[test]
    fn exhaustion_tracks_attempt_count() {
        let b = Backoff::new(Duration::from_millis(10), 2.0, Duration::from_secs(1), 3);
        assert!(!b.is_exhausted(0));
        assert!(!b.is_exhausted(2));
        assert!(b.is_exhausted(3));
        assert!(b.is_exhausted(4));
        assert_eq!(b.max_retries(), 3);
    }

    #[test]
    fn retryable_classification() {
        assert!(DataError::Transport("reset".into()).is_retryable());
        assert!(DataError::MaintenanceWindow.is_retryable());
        assert!(DataError::RateLimited("bars").is_retryable());

        assert!(!DataError::Auth("bad token".into()).is_retryable());
        assert!(!DataError::Other("boom".into()).is_retryable());
    }

    #[test]
    #[should_panic(expected = "множитель backoff должен быть ≥ 1.0")]
    fn factor_below_one_panics() {
        let _ = Backoff::new(Duration::from_millis(10), 0.5, Duration::from_secs(1), 3);
    }

    #[test]
    fn default_matches_finam_default() {
        let d = Backoff::default();
        assert_eq!(d.delay(0), Duration::from_millis(500));
        assert_eq!(d.max_retries(), 5);
    }
}
