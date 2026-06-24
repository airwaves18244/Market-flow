//! Оборот — главная «денежная» метрика терминала.
//!
//! Оборот (turnover) = денежный объём = Σ цена × объём. Он показывает, сколько
//! денег реально прошло через инструмент, и служит размером плиток в treemap,
//! базой для долей в кросс-активе и для скана «необычного объёма».

use crate::model::Bar;

/// Суммарный оборот по серии баров.
pub fn total_turnover(bars: &[Bar]) -> f64 {
    bars.iter().map(Bar::turnover).sum()
}

/// Направленный оборот: часть оборота «на росте» и «на падении».
///
/// Бар относится к росту, если `close > open`, к падению — если `close < open`.
/// Бары без изменения (`close == open`) не учитываются ни там, ни там.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DirectionalTurnover {
    pub up: f64,
    pub down: f64,
}

impl DirectionalTurnover {
    /// Чистый направленный оборот: `up - down`.
    pub fn net(&self) -> f64 {
        self.up - self.down
    }

    /// Полный оборот: `up + down` (без учёта неизменившихся баров).
    pub fn gross(&self) -> f64 {
        self.up + self.down
    }
}

/// Разложить оборот серии на «вверх»/«вниз» по знаку изменения бара.
pub fn directional_turnover(bars: &[Bar]) -> DirectionalTurnover {
    let mut acc = DirectionalTurnover::default();
    for b in bars {
        let t = b.turnover();
        if b.close > b.open {
            acc.up += t;
        } else if b.close < b.open {
            acc.down += t;
        }
    }
    acc
}

/// Коэффициент «необычности» объёма: оборот последнего бара относительно
/// среднего оборота предыдущих `lookback` баров.
///
/// Возвращает `None`, если истории недостаточно или среднее равно нулю.
/// Значение `2.0` означает «вдвое выше обычного».
pub fn unusual_volume_ratio(bars: &[Bar], lookback: usize) -> Option<f64> {
    if lookback == 0 || bars.len() <= lookback {
        return None;
    }
    let last = bars.last()?.turnover();
    let window = &bars[bars.len() - 1 - lookback..bars.len() - 1];
    let avg = total_turnover(window) / lookback as f64;
    if avg <= 0.0 {
        return None;
    }
    Some(last / avg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(open: f64, close: f64, volume: f64) -> Bar {
        // high/low берём как обёртку вокруг open/close, чтобы typical_price был
        // предсказуемым для тестов.
        Bar {
            ts: 0,
            open,
            high: open.max(close),
            low: open.min(close),
            close,
            volume,
        }
    }

    #[test]
    fn total_turnover_sums_typical_price_times_volume() {
        // typical = (h+l+c)/3 = (10+10+10)/3 = 10; turnover = 10*5 = 50
        let bars = [bar(10.0, 10.0, 5.0), bar(10.0, 10.0, 5.0)];
        assert_eq!(total_turnover(&bars), 100.0);
    }

    #[test]
    fn directional_splits_by_sign() {
        let bars = [
            bar(10.0, 12.0, 10.0), // up
            bar(12.0, 11.0, 10.0), // down
            bar(11.0, 11.0, 10.0), // unchanged -> ignored
        ];
        let d = directional_turnover(&bars);
        assert!(d.up > 0.0);
        assert!(d.down > 0.0);
        // up bar typical=(12+10+12)/3≈11.33; down bar typical=(12+11+11)/3≈11.33
        assert!(d.net().abs() < d.gross());
    }

    #[test]
    fn unusual_volume_detects_spike() {
        let mut bars: Vec<Bar> = (0..5).map(|_| bar(100.0, 100.0, 1.0)).collect();
        bars.push(bar(100.0, 100.0, 5.0)); // 5x объём
        let ratio = unusual_volume_ratio(&bars, 5).unwrap();
        assert!((ratio - 5.0).abs() < 1e-9, "ratio = {ratio}");
    }

    #[test]
    fn unusual_volume_needs_history() {
        let bars = [bar(1.0, 1.0, 1.0)];
        assert_eq!(unusual_volume_ratio(&bars, 5), None);
    }
}
