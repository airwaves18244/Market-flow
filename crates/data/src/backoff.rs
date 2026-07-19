//! Экспоненциальный backoff для повторов запросов.
//!
//! Пара к [`RateLimiter`](crate::RateLimiter) и [`TokenState`](crate::TokenState):
//! ограничитель решает, *можно* ли слать запрос, а backoff — *как долго ждать*
//! перед повтором после транзиентной ошибки (обрыв стрима, техокно, локальный
//! отказ rate-limit). Сами повторы и засыпание подключаются в фазе интеграции
//! API (нужен async-runtime); здесь — чистый, детерминированный расчёт задержек.
//!
//! Классификация ретраябельности ошибок — в [`DataError::is_retryable`].

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

/// Доля джиттера в `[0, 1)` для «полного джиттера» ([`Backoff::delay_with_jitter`]).
///
/// Единый источник размазывания повторов для всего транспортного слоя (`grpc`/
/// `http`/`market`): раньше каждый модуль держал свою копию на
/// `Instant::now().elapsed().subsec_nanos()`, но у свежесозданного `Instant`
/// прошедшее время околонулевое (десятки нс) — джиттер вырождался в ~0 и
/// backoff фактически не размазывался (R-5). Берём младшие наносекунды
/// **системных** часов ([`SystemTime`]) и перемешиваем атомарным счётчиком,
/// чтобы две выборки в пределах одной наносекунды всё же различались. Качество
/// ГПСЧ здесь не критично — нужен лишь ненулевой разброс, чтобы одновременные
/// повторы разных задач не били залпом.
pub fn jitter_fraction() -> f64 {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let salt = COUNTER.fetch_add(1, Ordering::Relaxed);
    // Кнутов мультипликативный хеш (2654435761 — 2^32/φ) разносит соседние
    // значения счётчика по всему диапазону, а не сдвигает на единицу.
    let mixed = nanos.wrapping_add(salt.wrapping_mul(2_654_435_761));
    f64::from(mixed % 1_000_000) / 1_000_000.0
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
    fn jitter_fraction_is_not_degenerate() {
        // Старый джиттер на `Instant::now().elapsed()` давал десятки нс и почти
        // всегда ~0. Системные наносекунды дают полноценный разброс: максимум из
        // сотни выборок гарантированно заметно больше нуля, а все значения — в
        // полуинтервале `[0, 1)`.
        let samples: Vec<f64> = (0..100).map(|_| jitter_fraction()).collect();
        let max = samples.iter().copied().fold(0.0_f64, f64::max);
        assert!(max > 0.3, "джиттер вырожден около нуля: max={max}");
        assert!(
            samples.iter().all(|&f| (0.0..1.0).contains(&f)),
            "джиттер вышел за [0, 1): {samples:?}"
        );
    }

    #[test]
    fn default_matches_finam_default() {
        let d = Backoff::default();
        assert_eq!(d.delay(0), Duration::from_millis(500));
        assert_eq!(d.max_retries(), 5);
    }
}
