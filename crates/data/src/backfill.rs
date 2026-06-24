//! Планировщик исторического бэкфилла и батч-поллинга.
//!
//! Finam Trade API отдаёт бары порциями и ограничивает частоту вызовов
//! (~200 запросов/мин на метод). Чтобы выкачать длинную историю, диапазон
//! времени режется на окна фиксированной длины, а вызовы разносятся во времени
//! под лимит. Здесь — чистая, тестируемая логика планирования (без сети);
//! сетевой драйвер подключается поверх трейта [`crate::MarketData`].

use std::time::Duration;

/// Полуинтервал времени `[from_ts, to_ts)` в UNIX-секундах — одно окно запроса.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub from_ts: i64,
    pub to_ts: i64,
}

impl Window {
    pub fn len_secs(&self) -> i64 {
        (self.to_ts - self.from_ts).max(0)
    }
}

/// Разбить диапазон `[from_ts, to_ts)` на окна длиной не более `window_secs`.
///
/// Последнее окно может быть короче. Пустой результат, если диапазон вырожден
/// (`from_ts >= to_ts`) или `window_secs <= 0`.
pub fn plan_windows(from_ts: i64, to_ts: i64, window_secs: i64) -> Vec<Window> {
    if from_ts >= to_ts || window_secs <= 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut cursor = from_ts;
    while cursor < to_ts {
        let end = (cursor.saturating_add(window_secs)).min(to_ts);
        out.push(Window { from_ts: cursor, to_ts: end });
        cursor = end;
    }
    out
}

/// Минимальная пауза между запросами, удерживающая частоту под лимитом
/// `max_per_min` (запросов в минуту на метод). При `0` — без паузы.
pub fn min_interval(max_per_min: u32) -> Duration {
    if max_per_min == 0 {
        return Duration::ZERO;
    }
    Duration::from_secs_f64(60.0 / max_per_min as f64)
}

/// Оценка числа запросов и минимальной длительности бэкфилла одного инструмента.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackfillPlan {
    pub windows: usize,
    pub min_duration: Duration,
}

/// Спланировать бэкфилл: сколько окон и сколько это займёт минимум при лимите.
pub fn plan_backfill(
    from_ts: i64,
    to_ts: i64,
    window_secs: i64,
    max_per_min: u32,
) -> BackfillPlan {
    let windows = plan_windows(from_ts, to_ts, window_secs).len();
    // N запросов требуют (N-1) пауз между ними.
    let gaps = windows.saturating_sub(1) as u32;
    let min_duration = min_interval(max_per_min) * gaps;
    BackfillPlan { windows, min_duration }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_cover_range_without_gaps_or_overlap() {
        let w = plan_windows(0, 100, 30);
        assert_eq!(
            w,
            vec![
                Window { from_ts: 0, to_ts: 30 },
                Window { from_ts: 30, to_ts: 60 },
                Window { from_ts: 60, to_ts: 90 },
                Window { from_ts: 90, to_ts: 100 }, // хвост короче
            ]
        );
        // Стыковка: конец предыдущего == начало следующего.
        for pair in w.windows(2) {
            assert_eq!(pair[0].to_ts, pair[1].from_ts);
        }
        // Суммарная длина == длине диапазона.
        assert_eq!(w.iter().map(Window::len_secs).sum::<i64>(), 100);
    }

    #[test]
    fn exact_multiple_has_no_short_tail() {
        let w = plan_windows(0, 90, 30);
        assert_eq!(w.len(), 3);
        assert!(w.iter().all(|x| x.len_secs() == 30));
    }

    #[test]
    fn degenerate_ranges_yield_no_windows() {
        assert!(plan_windows(100, 100, 30).is_empty());
        assert!(plan_windows(100, 50, 30).is_empty());
        assert!(plan_windows(0, 100, 0).is_empty());
        assert!(plan_windows(0, 100, -5).is_empty());
    }

    #[test]
    fn min_interval_respects_rate_limit() {
        assert_eq!(min_interval(0), Duration::ZERO);
        assert_eq!(min_interval(60), Duration::from_secs(1));
        assert_eq!(min_interval(200), Duration::from_secs_f64(0.3));
    }

    #[test]
    fn backfill_plan_counts_windows_and_paces_gaps() {
        // 100 окон по 1 минуте при лимите 200/мин → 99 пауз по 0.3с.
        let plan = plan_backfill(0, 100 * 60, 60, 200);
        assert_eq!(plan.windows, 100);
        assert_eq!(plan.min_duration, Duration::from_secs_f64(0.3) * 99);
        // Одно окно — пауз нет.
        let one = plan_backfill(0, 30, 60, 200);
        assert_eq!(one.windows, 1);
        assert_eq!(one.min_duration, Duration::ZERO);
    }
}
