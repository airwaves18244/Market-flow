//! FUTOI (датасет ALGOPACK `futoi`, только рынок `fo`): открытый интерес по
//! фьючерсам с разбивкой по группам участников (физлица/юрлица) и аналитика
//! динамики позиций.
//!
//! Производные метрики: нетто-позиция, доли long/short, изменение OI за период,
//! дивергенция «цена ↔ позиция» и экстремумы.

use serde::{Deserialize, Serialize};

/// Группа клиентов FUTOI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientGroup {
    /// Физические лица (FIZ).
    Fiz,
    /// Юридические лица (YUR).
    Yur,
}

impl ClientGroup {
    /// Машинный код группы (как в ALGOPACK).
    pub fn code(self) -> &'static str {
        match self {
            ClientGroup::Fiz => "fiz",
            ClientGroup::Yur => "yur",
        }
    }

    /// Разобрать группу из кода ALGOPACK (`fiz`/`yur`, без учёта регистра).
    pub fn from_code(code: &str) -> Option<ClientGroup> {
        match code.to_ascii_lowercase().as_str() {
            "fiz" => Some(ClientGroup::Fiz),
            "yur" => Some(ClientGroup::Yur),
            _ => None,
        }
    }
}

/// Точка FUTOI: открытый интерес группы участников по инструменту на момент `ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FutoiPoint {
    pub ts: i64,
    pub secid: String,
    pub clgroup: ClientGroup,
    /// Суммарная позиция (контрактов).
    pub pos: f64,
    /// Длинная/короткая позиция (контрактов).
    pub pos_long: f64,
    pub pos_short: f64,
    /// Число длинных/коротких позиций (участников).
    pub pos_long_num: f64,
    pub pos_short_num: f64,
}

impl FutoiPoint {
    /// Нетто-позиция: длинные минус короткие.
    pub fn net(&self) -> f64 {
        self.pos_long - self.pos_short
    }

    /// Доля длинных в суммарной позиции (0..1). При нулевой позиции — `0.5`.
    pub fn long_share(&self) -> f64 {
        let total = self.pos_long + self.pos_short;
        if total <= 0.0 {
            0.5
        } else {
            self.pos_long / total
        }
    }

    /// Доля коротких (0..1).
    pub fn short_share(&self) -> f64 {
        1.0 - self.long_share()
    }
}

/// Изменение OI за период между первой и последней точками серии (по нетто).
/// `None`, если точек меньше двух.
pub fn oi_change(points: &[FutoiPoint]) -> Option<f64> {
    let first = points.first()?;
    let last = points.last()?;
    Some(last.net() - first.net())
}

/// Дивергенция «цена ↔ нетто-позиция»: знак расхождения направлений.
///
/// Возвращает `true`, если цена и нетто-позиция группы двигались
/// **в противоположных** направлениях за период (классический сигнал
/// разворота: толпа набирает противоположную тренду позицию).
/// `prices` и `points` должны быть одной длины (≥ 2).
pub fn price_position_divergence(points: &[FutoiPoint], prices: &[f64]) -> Option<bool> {
    if points.len() < 2 || points.len() != prices.len() {
        return None;
    }
    let dp = prices[prices.len() - 1] - prices[0];
    let dpos = points[points.len() - 1].net() - points[0].net();
    if dp == 0.0 || dpos == 0.0 {
        return Some(false);
    }
    Some(dp.signum() != dpos.signum())
}

/// Экстремумы нетто-позиции по серии: индексы минимума и максимума.
/// `None` для пустой серии.
pub fn net_extremes(points: &[FutoiPoint]) -> Option<(usize, usize)> {
    if points.is_empty() {
        return None;
    }
    let mut min_i = 0;
    let mut max_i = 0;
    for (i, p) in points.iter().enumerate() {
        if p.net() < points[min_i].net() {
            min_i = i;
        }
        if p.net() > points[max_i].net() {
            max_i = i;
        }
    }
    Some((min_i, max_i))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(ts: i64, long: f64, short: f64) -> FutoiPoint {
        FutoiPoint {
            ts,
            secid: "RIH5".into(),
            clgroup: ClientGroup::Fiz,
            pos: long + short,
            pos_long: long,
            pos_short: short,
            pos_long_num: long / 10.0,
            pos_short_num: short / 10.0,
        }
    }

    #[test]
    fn net_and_shares() {
        let p = point(0, 700.0, 300.0);
        assert_eq!(p.net(), 400.0);
        assert!((p.long_share() - 0.7).abs() < 1e-12);
        assert!((p.short_share() - 0.3).abs() < 1e-12);
    }

    #[test]
    fn group_code_roundtrips() {
        assert_eq!(ClientGroup::from_code("FIZ"), Some(ClientGroup::Fiz));
        assert_eq!(ClientGroup::from_code("yur"), Some(ClientGroup::Yur));
        assert_eq!(ClientGroup::from_code("x"), None);
        assert_eq!(ClientGroup::Yur.code(), "yur");
    }

    #[test]
    fn oi_change_over_series() {
        let pts = vec![point(0, 500.0, 500.0), point(300, 800.0, 200.0)];
        // net: 0 → 600.
        assert_eq!(oi_change(&pts), Some(600.0));
    }

    #[test]
    fn divergence_when_price_up_position_down() {
        let pts = vec![point(0, 700.0, 300.0), point(300, 300.0, 700.0)]; // net 400 → −400
        let prices = vec![100.0, 110.0]; // цена вверх
        assert_eq!(price_position_divergence(&pts, &prices), Some(true));
    }

    #[test]
    fn no_divergence_when_aligned() {
        let pts = vec![point(0, 300.0, 700.0), point(300, 700.0, 300.0)]; // net −400 → 400
        let prices = vec![100.0, 110.0]; // оба вверх
        assert_eq!(price_position_divergence(&pts, &prices), Some(false));
    }

    #[test]
    fn extremes_indices() {
        let pts = vec![
            point(0, 500.0, 500.0),   // net 0
            point(300, 900.0, 100.0), // net 800 (max)
            point(600, 100.0, 900.0), // net −800 (min)
        ];
        assert_eq!(net_extremes(&pts), Some((2, 1)));
    }
}
