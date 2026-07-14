//! Obstats (датасет ALGOPACK `obstats`): статистика стакана заявок — спред
//! best bid/offer (и по глубине), число уровней и объём/стоимость на
//! бид/аск-сторонах, из которых считается дисбаланс ликвидности.
//!
//! Точная форма колонок ISS не сверена по живому ответу (см.
//! `crates/data/tests/fixtures/moex/README.md`, `(unverified)`) — поэтому все
//! поля, кроме идентификации точки (`ts`/`secid`), мягкие (`Option`): парсер
//! `data::moex` не должен падать при переименовании/отсутствии колонки.

use serde::{Deserialize, Serialize};

/// Точка Obstats: снимок статистики стакана по инструменту на момент `ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObstatsPoint {
    pub ts: i64,
    pub secid: String,
    /// Относительный спред best bid/offer, доля цены.
    pub spread_bbo: Option<f64>,
    /// Относительный спред по глубине топ-10 уровней.
    pub spread_lv10: Option<f64>,
    /// Относительный спред на объём заявки в 1 млн (доля).
    pub spread_1mio: Option<f64>,
    /// Число уровней на стороне бид/аск.
    pub levels_b: Option<f64>,
    pub levels_s: Option<f64>,
    /// Объём (в штуках/контрактах) на стороне бид/аск.
    pub vol_b: Option<f64>,
    pub vol_s: Option<f64>,
    /// Стоимость (денежный объём) на стороне бид/аск.
    pub val_b: Option<f64>,
    pub val_s: Option<f64>,
    /// VWAP стакана на стороне бид/аск.
    pub vwap_b: Option<f64>,
    pub vwap_s: Option<f64>,
}

impl ObstatsPoint {
    /// Пустая точка (все метрики отсутствуют) на момент `ts`.
    pub fn at(ts: i64, secid: impl Into<String>) -> Self {
        ObstatsPoint {
            ts,
            secid: secid.into(),
            spread_bbo: None,
            spread_lv10: None,
            spread_1mio: None,
            levels_b: None,
            levels_s: None,
            vol_b: None,
            vol_s: None,
            val_b: None,
            val_s: None,
            vwap_b: None,
            vwap_s: None,
        }
    }

    /// Дисбаланс объёма стакана (−1..1): положительный — перевес бида
    /// (спрос) над аском (предложением). `None`, если объёмы не заданы или
    /// суммарный объём нулевой.
    pub fn volume_imbalance(&self) -> Option<f64> {
        imbalance(self.vol_b?, self.vol_s?)
    }

    /// Дисбаланс стоимости стакана (−1..1), аналогично [`volume_imbalance`]
    /// на денежных объёмах.
    ///
    /// [`volume_imbalance`]: Self::volume_imbalance
    pub fn value_imbalance(&self) -> Option<f64> {
        imbalance(self.val_b?, self.val_s?)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_imbalance_favours_bid() {
        let mut p = ObstatsPoint::at(0, "SBER");
        p.vol_b = Some(700.0);
        p.vol_s = Some(300.0);
        assert!((p.volume_imbalance().unwrap() - 0.4).abs() < 1e-12);
    }

    #[test]
    fn value_imbalance_favours_ask() {
        let mut p = ObstatsPoint::at(0, "SBER");
        p.val_b = Some(200.0);
        p.val_s = Some(800.0);
        assert!((p.value_imbalance().unwrap() - (-0.6)).abs() < 1e-12);
    }

    #[test]
    fn missing_side_is_none() {
        let mut p = ObstatsPoint::at(0, "SBER");
        p.vol_b = Some(100.0); // vol_s отсутствует
        assert_eq!(p.volume_imbalance(), None);
        assert_eq!(p.value_imbalance(), None);
    }

    #[test]
    fn zero_total_is_none() {
        let mut p = ObstatsPoint::at(0, "SBER");
        p.vol_b = Some(0.0);
        p.vol_s = Some(0.0);
        assert_eq!(p.volume_imbalance(), None);
    }

    #[test]
    fn empty_point_has_all_none() {
        let p = ObstatsPoint::at(1, "X");
        assert_eq!(p.spread_bbo, None);
        assert_eq!(p.volume_imbalance(), None);
    }
}
