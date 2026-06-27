//! Ограничение частоты запросов (per-method).
//!
//! Finam Trade API лимитирует вызовы пометодно (порядка ~200 запросов в минуту
//! на метод). [`RateLimiter`] держит независимую «корзину токенов» на каждый
//! метод: запрос разрешён, если в корзине есть токен; токены доливаются со
//! временем до ёмкости.
//!
//! Реализация чистая (время — параметр `now`, UNIX-секунды), без таймеров и
//! внешних зависимостей, поэтому детерминированно тестируется. Это сознательная
//! замена внешнему `governor`: ядро остаётся кросс-платформенно собираемым без
//! асинхронной инфраструктуры, а ожидание/сон организует вызывающий слой.

use std::collections::BTreeMap;

/// Корзина токенов: ёмкость, текущий запас и скорость долива.
#[derive(Debug, Clone, Copy)]
struct Bucket {
    capacity: f64,
    tokens: f64,
    refill_per_sec: f64,
    last: i64,
}

impl Bucket {
    fn new(capacity: f64, refill_per_sec: f64, now: i64) -> Self {
        Self {
            capacity,
            tokens: capacity,
            refill_per_sec,
            last: now,
        }
    }

    /// Долить токены пропорционально прошедшему времени (не больше ёмкости).
    fn refill(&mut self, now: i64) {
        if now > self.last {
            let elapsed = (now - self.last) as f64;
            self.tokens = (self.tokens + elapsed * self.refill_per_sec).min(self.capacity);
            self.last = now;
        }
    }

    /// Попытаться списать один токен.
    fn try_acquire(&mut self, now: i64) -> bool {
        self.refill(now);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Пометодный ограничитель частоты на основе токен-бакета.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    buckets: BTreeMap<&'static str, Bucket>,
    capacity: f64,
    refill_per_sec: f64,
}

impl RateLimiter {
    /// Лимитер с общим лимитом `per_minute` на каждый метод (ёмкость = лимиту,
    /// долив = `per_minute/60` в секунду). `per_minute` зажимается в `>= 1`.
    pub fn per_minute(per_minute: u32) -> Self {
        let cap = per_minute.max(1) as f64;
        Self {
            buckets: BTreeMap::new(),
            capacity: cap,
            refill_per_sec: cap / 60.0,
        }
    }

    fn bucket(&mut self, method: &'static str, now: i64) -> &mut Bucket {
        let (cap, rate) = (self.capacity, self.refill_per_sec);
        self.buckets
            .entry(method)
            .or_insert_with(|| Bucket::new(cap, rate, now))
    }

    /// Разрешён ли сейчас вызов метода `method`. Списывает токен при успехе.
    pub fn try_acquire(&mut self, method: &'static str, now: i64) -> bool {
        self.bucket(method, now).try_acquire(now)
    }

    /// Доступный (целочисленный) остаток токенов метода на момент `now`.
    /// Полезно для диагностики/метрик; сам остаток не списывает.
    pub fn available(&mut self, method: &'static str, now: i64) -> u32 {
        let b = self.bucket(method, now);
        b.refill(now);
        b.tokens.floor().max(0.0) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_capacity_then_blocks() {
        let mut rl = RateLimiter::per_minute(200);
        // 200 вызовов в один и тот же момент проходят
        for _ in 0..200 {
            assert!(rl.try_acquire("Bars", 0));
        }
        // 201-й — отказ
        assert!(!rl.try_acquire("Bars", 0));
    }

    #[test]
    fn refills_over_time() {
        let mut rl = RateLimiter::per_minute(60); // 1 токен/сек
        for _ in 0..60 {
            assert!(rl.try_acquire("LastQuote", 0));
        }
        assert!(!rl.try_acquire("LastQuote", 0));
        // через 1 секунду долилось примерно 1 токен
        assert!(rl.try_acquire("LastQuote", 1));
        assert!(!rl.try_acquire("LastQuote", 1));
    }

    #[test]
    fn refill_caps_at_capacity() {
        let mut rl = RateLimiter::per_minute(120);
        // выпьем всё
        for _ in 0..120 {
            assert!(rl.try_acquire("Assets", 0));
        }
        // спустя час долив не превышает ёмкости
        assert_eq!(rl.available("Assets", 3_600), 120);
    }

    #[test]
    fn methods_have_independent_buckets() {
        let mut rl = RateLimiter::per_minute(1);
        assert!(rl.try_acquire("Bars", 0));
        assert!(!rl.try_acquire("Bars", 0));
        // другой метод — своя корзина
        assert!(rl.try_acquire("LatestTrades", 0));
    }
}
