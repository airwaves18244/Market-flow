//! Устойчивость (§ 0.3): техническое окно и экспоненциальный backoff.

use std::time::Duration;

/// Техническое окно Finam: 05:00–06:15 MSK (UTC+3, без перехода на летнее
/// время). В это время API недоступен. `unix_secs` — UNIX-секунды UTC.
pub fn is_maintenance_window(unix_secs: i64) -> bool {
    const MSK_OFFSET: i64 = 3 * 3600;
    const DAY: i64 = 86_400;
    let seconds_of_day = (unix_secs + MSK_OFFSET).rem_euclid(DAY);
    let start = 5 * 3600; // 05:00
    let end = 6 * 3600 + 15 * 60; // 06:15
    (start..end).contains(&seconds_of_day)
}

/// Задержка перед `attempt`-й попыткой переподключения: 1,2,4,…,32 c (с потолком).
pub fn backoff_delay(attempt: u32) -> Duration {
    let secs = 1u64 << attempt.min(5); // 2^0..2^5
    Duration::from_secs(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Хелпер: UNIX-время для часа:минуты по UTC в нулевые сутки эпохи.
    fn utc(h: i64, m: i64) -> i64 {
        h * 3600 + m * 60
    }

    #[test]
    fn maintenance_window_msk_bounds() {
        // 05:30 MSK == 02:30 UTC — внутри окна.
        assert!(is_maintenance_window(utc(2, 30)));
        // 05:00 MSK == 02:00 UTC — начало (включительно).
        assert!(is_maintenance_window(utc(2, 0)));
        // 06:20 MSK == 03:20 UTC — после окна.
        assert!(!is_maintenance_window(utc(3, 20)));
        // полночь UTC == 03:00 MSK — вне окна.
        assert!(!is_maintenance_window(0));
    }

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(backoff_delay(0), Duration::from_secs(1));
        assert_eq!(backoff_delay(3), Duration::from_secs(8));
        assert_eq!(backoff_delay(5), Duration::from_secs(32));
        assert_eq!(backoff_delay(99), Duration::from_secs(32)); // потолок
    }
}
