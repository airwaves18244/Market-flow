//! MOEX ALGOPACK: доменные типы и аналитика поверх датасетов Super Candles
//! (tradestats), FUTOI и HI2, а также движок «Mega Alerts» (фаза 10).
//!
//! Дисциплина слоёв — как во всём `domain`: только чистая математика, без сети,
//! БД и UI. Сетевой транспорт (`data::moex` поверх ISS REST) переводит сырые
//! ответы ALGOPACK в эти типы; здесь — агрегация, метрики и сигналы.
//!
//! Имена полей соответствуют датасетам ALGOPACK (`tradestats`/`futoi`/`hi2`);
//! их точная форма фиксируется по живым фикстурам в `data` (задачи `10.0.4`),
//! доменные типы устойчивы к отсутствию отдельных полей.
//!
//! Подмодули:
//! - [`tradestats`] — Super Candles: типы и аналитика (агрегация TF, VWAP-полоса,
//!   buy-pressure, аномальный объём);
//! - [`futoi`] — открытый интерес физ/юр и его динамика;
//! - [`hi2`] — индекс концентрации участников;
//! - [`mega_alerts`] — движок сигналов поверх tradestats/futoi/hi2.

pub mod futoi;
pub mod hi2;
pub mod mega_alerts;
pub mod tradestats;

pub use futoi::{ClientGroup, FutoiPoint};
pub use hi2::Hi2Point;
pub use mega_alerts::{MegaAlert, MegaAlertEngine, MegaAlertKind, MegaThresholds};
pub use tradestats::SuperCandle;

/// z-score последнего значения относительно скользящего окна предыдущих
/// `window` точек (исключая текущую). `None`, если истории недостаточно или
/// дисперсия нулевая.
///
/// Переиспользуется аналитикой объёма/OI/HI2 и движком Mega Alerts.
pub(crate) fn rolling_zscore(values: &[f64], idx: usize, window: usize) -> Option<f64> {
    if window == 0 || idx < window {
        return None;
    }
    let slice = &values[idx - window..idx];
    let n = slice.len() as f64;
    let mean = slice.iter().sum::<f64>() / n;
    let var = slice.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let sd = var.sqrt();
    if sd <= f64::EPSILON {
        return None;
    }
    Some((values[idx] - mean) / sd)
}

#[cfg(test)]
mod tests {
    use super::rolling_zscore;

    #[test]
    fn zscore_detects_spike() {
        // Реалистичный (не вырожденный) базовый уровень: дисперсия > 0.
        let v = [9.0, 11.0, 10.0, 10.0, 30.0];
        let z = rolling_zscore(&v, 4, 4).unwrap();
        assert!(z > 3.0, "z={z}");
    }

    #[test]
    fn zscore_zero_variance_baseline_is_none() {
        // Идеально плоский базовый уровень → стандартное отклонение 0 → None.
        let v = [10.0, 10.0, 10.0, 10.0, 30.0];
        assert!(rolling_zscore(&v, 4, 4).is_none());
    }

    #[test]
    fn zscore_needs_history() {
        let v = [1.0, 2.0, 3.0];
        assert!(rolling_zscore(&v, 1, 4).is_none());
    }

    #[test]
    fn zscore_zero_variance_is_none() {
        let v = [5.0, 5.0, 5.0, 5.0, 5.0];
        assert!(rolling_zscore(&v, 4, 4).is_none());
    }
}
