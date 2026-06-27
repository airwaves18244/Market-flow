//! Ограничитель частоты запросов (per-method rate-limit).
//!
//! Finam Trade API лимитирует каждый метод примерно на 200 запросов в минуту.
//! Здесь — чистый, без внешних зависимостей и кросс-платформенно тестируемый
//! ограничитель: скользящее окно по каждому методу отдельно.
//!
//! Логика детерминированно тестируется через [`RateLimiter::try_acquire_at`]
//! (моменты времени подаются явно), а боевой путь [`RateLimiter::try_acquire`]
//! берёт текущий монотонный [`Instant`].
//!
//! Ключ метода — `&'static str`, что согласуется с [`DataError::RateLimited`]
//! и исключает аллокации на горячем пути.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::DataError;

/// Лимит Finam Trade API по умолчанию: запросов на метод за окно.
pub const DEFAULT_LIMIT_PER_MINUTE: u32 = 200;

/// Скользящее окно учёта вызовов по каждому методу.
///
/// Один экземпляр обслуживает все методы клиента: учёт ведётся раздельно по
/// ключу метода, поэтому насыщение одного метода не блокирует другие.
#[derive(Debug)]
pub struct RateLimiter {
    limit: u32,
    window: Duration,
    /// Метки времени успешных вызовов в пределах окна, по методу.
    calls: Mutex<HashMap<&'static str, Vec<Instant>>>,
}

impl RateLimiter {
    /// Ограничитель с заданным лимитом на произвольное окно.
    ///
    /// # Panics
    /// Паникует, если `limit == 0` или `window` нулевой — такой ограничитель
    /// отверг бы любой запрос и почти наверняка означает ошибку конфигурации.
    pub fn new(limit: u32, window: Duration) -> Self {
        assert!(limit > 0, "лимит должен быть положительным");
        assert!(!window.is_zero(), "окно должно быть ненулевым");
        Self {
            limit,
            window,
            calls: Mutex::new(HashMap::new()),
        }
    }

    /// Ограничитель «N запросов в минуту» (окно — 60 секунд).
    pub fn per_minute(limit: u32) -> Self {
        Self::new(limit, Duration::from_secs(60))
    }

    /// Ограничитель под лимит Finam по умолчанию (200 запросов/мин на метод).
    pub fn finam_default() -> Self {
        Self::per_minute(DEFAULT_LIMIT_PER_MINUTE)
    }

    /// Попытаться занять слот для `method` «сейчас».
    ///
    /// Возвращает [`DataError::RateLimited`], если в текущем окне уже
    /// исчерпан лимит по этому методу.
    pub fn try_acquire(&self, method: &'static str) -> Result<(), DataError> {
        self.try_acquire_at(method, Instant::now())
    }

    /// Версия [`try_acquire`](Self::try_acquire) с явным моментом времени —
    /// для детерминированных тестов и симуляций.
    pub fn try_acquire_at(&self, method: &'static str, now: Instant) -> Result<(), DataError> {
        let mut calls = self.calls.lock().expect("rate-limiter mutex отравлен");
        let slots = calls.entry(method).or_default();
        prune(slots, now, self.window);
        if slots.len() as u32 >= self.limit {
            return Err(DataError::RateLimited(method));
        }
        slots.push(now);
        Ok(())
    }

    /// Сколько ещё вызовов `method` допускается прямо сейчас (в текущем окне).
    pub fn remaining(&self, method: &'static str) -> u32 {
        self.remaining_at(method, Instant::now())
    }

    /// Версия [`remaining`](Self::remaining) с явным моментом времени.
    pub fn remaining_at(&self, method: &'static str, now: Instant) -> u32 {
        let mut calls = self.calls.lock().expect("rate-limiter mutex отравлен");
        let slots = calls.entry(method).or_default();
        prune(slots, now, self.window);
        self.limit.saturating_sub(slots.len() as u32)
    }

    /// Через сколько освободится слот для `method`, если он сейчас исчерпан.
    ///
    /// Возвращает `None`, если слот доступен немедленно, иначе — задержку до
    /// истечения самой старой метки в окне.
    pub fn retry_after_at(&self, method: &'static str, now: Instant) -> Option<Duration> {
        let mut calls = self.calls.lock().expect("rate-limiter mutex отравлен");
        let slots = calls.entry(method).or_default();
        prune(slots, now, self.window);
        if (slots.len() as u32) < self.limit {
            return None;
        }
        // Самая старая метка истечёт через (window - (now - oldest)).
        slots.first().map(|&oldest| {
            self.window
                .saturating_sub(now.saturating_duration_since(oldest))
        })
    }
}

/// Убрать из окна метки старше `window` относительно `now`.
fn prune(slots: &mut Vec<Instant>, now: Instant, window: Duration) {
    // Оставляем только метки строго новее границы окна. Если `now` ближе к
    // старту монотонных часов, чем ширина окна (`checked_sub` → `None`), все
    // метки заведомо в пределах окна — ничего не выбрасываем.
    if let Some(cutoff) = now.checked_sub(window) {
        slots.retain(|&t| t > cutoff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_limit_then_rejects() {
        let rl = RateLimiter::per_minute(3);
        let t0 = Instant::now();
        assert!(rl.try_acquire_at("bars", t0).is_ok());
        assert!(rl.try_acquire_at("bars", t0).is_ok());
        assert!(rl.try_acquire_at("bars", t0).is_ok());
        assert!(matches!(
            rl.try_acquire_at("bars", t0),
            Err(DataError::RateLimited("bars"))
        ));
    }

    #[test]
    fn methods_are_counted_independently() {
        let rl = RateLimiter::per_minute(1);
        let t0 = Instant::now();
        assert!(rl.try_acquire_at("bars", t0).is_ok());
        // Лимит "bars" исчерпан, но "assets" не затронут.
        assert!(rl.try_acquire_at("bars", t0).is_err());
        assert!(rl.try_acquire_at("assets", t0).is_ok());
    }

    #[test]
    fn slot_frees_after_window_slides() {
        let rl = RateLimiter::new(2, Duration::from_secs(60));
        let t0 = Instant::now();
        assert!(rl.try_acquire_at("bars", t0).is_ok());
        assert!(rl.try_acquire_at("bars", t0).is_ok());
        assert!(rl.try_acquire_at("bars", t0).is_err());

        // Чуть раньше границы окна старые метки ещё в счёте — слот занят.
        let just_before = t0 + Duration::from_secs(60) - Duration::from_millis(1);
        assert!(rl.try_acquire_at("bars", just_before).is_err());

        // Ровно на границе окна (граница строгая) самая старая метка истекает —
        // слот освобождается.
        let edge = t0 + Duration::from_secs(60);
        assert!(rl.try_acquire_at("bars", edge).is_ok());
    }

    #[test]
    fn remaining_tracks_usage_and_recovery() {
        let rl = RateLimiter::per_minute(3);
        let t0 = Instant::now();
        assert_eq!(rl.remaining_at("bars", t0), 3);
        rl.try_acquire_at("bars", t0).unwrap();
        rl.try_acquire_at("bars", t0).unwrap();
        assert_eq!(rl.remaining_at("bars", t0), 1);

        // После сдвига окна вместимость восстанавливается.
        let later = t0 + Duration::from_secs(61);
        assert_eq!(rl.remaining_at("bars", later), 3);
    }

    #[test]
    fn retry_after_reports_delay_until_oldest_expires() {
        let rl = RateLimiter::new(1, Duration::from_secs(60));
        let t0 = Instant::now();
        // Слот свободен → задержки нет.
        assert_eq!(rl.retry_after_at("bars", t0), None);

        rl.try_acquire_at("bars", t0).unwrap();
        // Через 20с после занятия ждать осталось ~40с.
        let t1 = t0 + Duration::from_secs(20);
        assert_eq!(rl.retry_after_at("bars", t1), Some(Duration::from_secs(40)));
    }

    #[test]
    fn unused_method_has_full_capacity() {
        let rl = RateLimiter::finam_default();
        assert_eq!(rl.remaining("last_quote"), DEFAULT_LIMIT_PER_MINUTE);
    }

    #[test]
    #[should_panic(expected = "лимит должен быть положительным")]
    fn zero_limit_panics() {
        let _ = RateLimiter::new(0, Duration::from_secs(60));
    }

    #[test]
    #[should_panic(expected = "окно должно быть ненулевым")]
    fn zero_window_panics() {
        let _ = RateLimiter::new(10, Duration::ZERO);
    }
}
