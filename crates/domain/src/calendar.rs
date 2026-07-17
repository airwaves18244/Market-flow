//! Календарная арифметика: перевод между григорианской датой и числом дней
//! от эпохи UNIX (1970-01-01).
//!
//! Алгоритм Ховарда Хинанта (`days_from_civil`/`civil_from_days`) — корректен
//! для всего пролептического григорианского календаря, без внешних
//! зависимостей. Раньше обе половины (прямая и обратная) жили независимо в
//! `data::moex::parse` и `data::history` (по одной копии на каждый источник
//! истории) — вынесены сюда, в `domain`, как чистая математика без сети/БД,
//! которую могут переиспользовать оба адаптера `data`.

/// Дни от эпохи (1970-01-01) для григорианской даты `(y, m, d)`.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (i64::from(m) + 9) % 12; // [0, 11]
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

/// Григорианская дата `(year, month, day)` из числа дней от эпохи
/// (1970-01-01). Обратная к [`days_from_civil`].
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_day_zero_is_1970_01_01() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }

    #[test]
    fn round_trip_across_wide_range() {
        // Проверяем обратимость на широком диапазоне дней, включая даты до
        // эпохи (отрицательные) и через границы месяцев/годов/високосных лет.
        for days in (-800_000..800_000).step_by(3719) {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days, "day {days} -> {y}-{m}-{d}");
        }
    }

    #[test]
    fn leap_year_boundary_feb_29_2024() {
        let days = days_from_civil(2024, 2, 29);
        assert_eq!(civil_from_days(days), (2024, 2, 29));
        assert_eq!(civil_from_days(days + 1), (2024, 3, 1));
    }
}
