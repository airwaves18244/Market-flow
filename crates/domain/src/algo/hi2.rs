//! HI2 (датасет ALGOPACK `hi2`): индекс концентрации участников торгов и его
//! интерпретация.
//!
//! Высокое значение → поток сконцентрирован в немногих участниках (вплоть до
//! доминирования одного игрока); низкое → распределённый поток. Аналитика:
//! классификация уровня концентрации по порогам, обнаружение всплесков
//! (z-score) и ранжирование инструментов.

use serde::{Deserialize, Serialize};

use super::rolling_zscore;

/// Точка HI2: индекс концентрации по инструменту на момент `ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hi2Point {
    pub ts: i64,
    pub secid: String,
    /// Индекс концентрации (Херфиндаль-подобный), 0..1: чем выше — тем сильнее
    /// доминирование немногих участников.
    pub concentration: f64,
}

impl Hi2Point {
    /// Построить точку из долей участников (`shares` суммируются к 1):
    /// индекс Херфиндаля `HHI = Σ sᵢ²`.
    pub fn from_shares(ts: i64, secid: impl Into<String>, shares: &[f64]) -> Self {
        let hhi = shares.iter().map(|s| s * s).sum::<f64>();
        Hi2Point {
            ts,
            secid: secid.into(),
            concentration: hhi,
        }
    }

    /// Интерпретация уровня концентрации по порогам.
    pub fn level(&self) -> ConcentrationLevel {
        ConcentrationLevel::classify(self.concentration)
    }
}

/// Качественный уровень концентрации потока.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConcentrationLevel {
    /// Распределённый поток (много участников).
    Distributed,
    /// Умеренная концентрация.
    Moderate,
    /// Высокая концентрация.
    Concentrated,
    /// Доминирование одного участника.
    Dominated,
}

impl ConcentrationLevel {
    /// Классификация по индексу концентрации (пороги — рабочие значения для
    /// HHI-подобной метрики; уточняются по живым данным `(verify)`).
    pub fn classify(c: f64) -> ConcentrationLevel {
        if c >= 0.5 {
            ConcentrationLevel::Dominated
        } else if c >= 0.25 {
            ConcentrationLevel::Concentrated
        } else if c >= 0.15 {
            ConcentrationLevel::Moderate
        } else {
            ConcentrationLevel::Distributed
        }
    }
}

/// Индексы точек со всплеском концентрации (z-score ≥ `threshold` по окну).
pub fn concentration_spikes(points: &[Hi2Point], window: usize, threshold: f64) -> Vec<usize> {
    let vals: Vec<f64> = points.iter().map(|p| p.concentration).collect();
    (0..points.len())
        .filter(|&i| rolling_zscore(&vals, i, window).is_some_and(|z| z >= threshold))
        .collect()
}

/// Ранжировать инструменты по убыванию концентрации (для последних значений
/// каждого инструмента). На вход — по одной свежей точке на инструмент.
/// Возвращает `(secid, concentration)` в порядке убывания.
pub fn rank_by_concentration(points: &[Hi2Point]) -> Vec<(String, f64)> {
    let mut ranked: Vec<(String, f64)> = points
        .iter()
        .map(|p| (p.secid.clone(), p.concentration))
        .collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hhi_from_equal_shares() {
        // 4 равных участника: HHI = 4·0.25² = 0.25.
        let p = Hi2Point::from_shares(0, "SBER", &[0.25, 0.25, 0.25, 0.25]);
        assert!((p.concentration - 0.25).abs() < 1e-12);
        assert_eq!(p.level(), ConcentrationLevel::Concentrated);
    }

    #[test]
    fn hhi_monopoly_is_one() {
        let p = Hi2Point::from_shares(0, "X", &[1.0]);
        assert!((p.concentration - 1.0).abs() < 1e-12);
        assert_eq!(p.level(), ConcentrationLevel::Dominated);
    }

    #[test]
    fn distributed_flow_low_index() {
        let shares = vec![0.1; 10]; // 10 равных → HHI 0.1
        let p = Hi2Point::from_shares(0, "X", &shares);
        assert!((p.concentration - 0.1).abs() < 1e-12);
        assert_eq!(p.level(), ConcentrationLevel::Distributed);
    }

    #[test]
    fn spikes_detected() {
        let mut pts = Vec::new();
        // Ненулевая дисперсия базового уровня концентрации.
        for (i, &c) in [0.09, 0.11, 0.10, 0.10, 0.10].iter().enumerate() {
            pts.push(Hi2Point {
                ts: i as i64 * 300,
                secid: "X".into(),
                concentration: c,
            });
        }
        pts.push(Hi2Point {
            ts: 1500,
            secid: "X".into(),
            concentration: 0.40,
        });
        assert_eq!(concentration_spikes(&pts, 5, 3.0), vec![5]);
    }

    #[test]
    fn ranking_descending() {
        let pts = vec![
            Hi2Point {
                ts: 0,
                secid: "A".into(),
                concentration: 0.2,
            },
            Hi2Point {
                ts: 0,
                secid: "B".into(),
                concentration: 0.5,
            },
            Hi2Point {
                ts: 0,
                secid: "C".into(),
                concentration: 0.1,
            },
        ];
        let r = rank_by_concentration(&pts);
        assert_eq!(r[0].0, "B");
        assert_eq!(r[2].0, "C");
    }
}
