//! Orderstats (датасет ALGOPACK `orderstats`): статистика заявок — сколько
//! выставлено (`put_*`) и снято (`cancel_*`) на бид/аск-сторонах за интервал.
//!
//! Как и [`super::obstats`], точная форма колонок ISS не сверена по живому
//! ответу (`(unverified)`, см. `crates/data/tests/fixtures/moex/README.md`),
//! поэтому все метрики — мягкие (`Option`).

use serde::{Deserialize, Serialize};

/// Точка Orderstats: активность заявок по инструменту за интервал,
/// заканчивающийся в `ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderstatsPoint {
    pub ts: i64,
    pub secid: String,
    /// Число выставленных заявок на покупку/продажу.
    pub put_orders_b: Option<f64>,
    pub put_orders_s: Option<f64>,
    /// Объём выставленных заявок на покупку/продажу.
    pub put_vol_b: Option<f64>,
    pub put_vol_s: Option<f64>,
    /// Стоимость выставленных заявок на покупку/продажу.
    pub put_val_b: Option<f64>,
    pub put_val_s: Option<f64>,
    /// Число снятых заявок на покупку/продажу.
    pub cancel_orders_b: Option<f64>,
    pub cancel_orders_s: Option<f64>,
    /// Объём снятых заявок на покупку/продажу.
    pub cancel_vol_b: Option<f64>,
    pub cancel_vol_s: Option<f64>,
    /// Стоимость снятых заявок на покупку/продажу.
    pub cancel_val_b: Option<f64>,
    pub cancel_val_s: Option<f64>,
}

impl OrderstatsPoint {
    /// Пустая точка (все метрики отсутствуют) на момент `ts`.
    pub fn at(ts: i64, secid: impl Into<String>) -> Self {
        OrderstatsPoint {
            ts,
            secid: secid.into(),
            put_orders_b: None,
            put_orders_s: None,
            put_vol_b: None,
            put_vol_s: None,
            put_val_b: None,
            put_val_s: None,
            cancel_orders_b: None,
            cancel_orders_s: None,
            cancel_vol_b: None,
            cancel_vol_s: None,
            cancel_val_b: None,
            cancel_val_s: None,
        }
    }

    /// Дисбаланс объёма выставленных заявок (−1..1): положительный — перевес
    /// заявок на покупку. `None`, если объёмы не заданы или суммарно нулевые.
    pub fn put_volume_imbalance(&self) -> Option<f64> {
        imbalance(self.put_vol_b?, self.put_vol_s?)
    }

    /// Доля снятых заявок на покупку от выставленных на покупку (0..1) —
    /// «эфемерность» бид-ликвидности. `None` без обеих величин или при
    /// нулевом выставленном объёме.
    pub fn cancel_ratio_b(&self) -> Option<f64> {
        cancel_ratio(self.cancel_vol_b?, self.put_vol_b?)
    }

    /// То же для стороны продаж.
    pub fn cancel_ratio_s(&self) -> Option<f64> {
        cancel_ratio(self.cancel_vol_s?, self.put_vol_s?)
    }
}

fn imbalance(b: f64, s: f64) -> Option<f64> {
    let total = b + s;
    if total <= 0.0 {
        None
    } else {
        Some((b - s) / total)
    }
}

fn cancel_ratio(cancelled: f64, put: f64) -> Option<f64> {
    if put <= 0.0 {
        None
    } else {
        Some(cancelled / put)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_volume_imbalance_favours_buy() {
        let mut p = OrderstatsPoint::at(0, "SBER");
        p.put_vol_b = Some(800.0);
        p.put_vol_s = Some(200.0);
        assert!((p.put_volume_imbalance().unwrap() - 0.6).abs() < 1e-12);
    }

    #[test]
    fn cancel_ratio_computed_per_side() {
        let mut p = OrderstatsPoint::at(0, "SBER");
        p.put_vol_b = Some(1000.0);
        p.cancel_vol_b = Some(400.0);
        p.put_vol_s = Some(500.0);
        p.cancel_vol_s = Some(100.0);
        assert!((p.cancel_ratio_b().unwrap() - 0.4).abs() < 1e-12);
        assert!((p.cancel_ratio_s().unwrap() - 0.2).abs() < 1e-12);
    }

    #[test]
    fn missing_fields_are_none() {
        let p = OrderstatsPoint::at(0, "SBER");
        assert_eq!(p.put_volume_imbalance(), None);
        assert_eq!(p.cancel_ratio_b(), None);
    }

    #[test]
    fn zero_put_volume_cancel_ratio_is_none() {
        let mut p = OrderstatsPoint::at(0, "SBER");
        p.put_vol_b = Some(0.0);
        p.cancel_vol_b = Some(0.0);
        assert_eq!(p.cancel_ratio_b(), None);
    }
}
