//! Секторная ротация в стиле RRG (Relative Rotation Graph).
//!
//! RRG показывает, какие сектора лидируют, а какие отстают относительно
//! бенчмарка (напр. индекса MOEX), и в какую сторону они движутся. Две оси:
//! - **RS-Ratio** — относительная сила (>100 сильнее бенчмарка);
//! - **RS-Momentum** — импульс относительной силы (>100 сила растёт).
//!
//! Это упрощённая, наглядная реализация в духе JdK RS-Ratio/RS-Momentum
//! (нормировка через скользящую среднюю, а не полная z-score-формула провайдера).

use super::sma;

/// Квадрант RRG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quadrant {
    /// Сильнее рынка и продолжает усиливаться.
    Leading,
    /// Сильнее рынка, но импульс падает.
    Weakening,
    /// Слабее рынка и продолжает слабеть.
    Lagging,
    /// Слабее рынка, но импульс растёт.
    Improving,
}

/// Точка на плоскости RRG для одного периода.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RrgPoint {
    pub rs_ratio: f64,
    pub rs_momentum: f64,
}

impl RrgPoint {
    /// Квадрант относительно центра (100, 100).
    pub fn quadrant(&self) -> Quadrant {
        match (self.rs_ratio >= 100.0, self.rs_momentum >= 100.0) {
            (true, true) => Quadrant::Leading,
            (true, false) => Quadrant::Weakening,
            (false, false) => Quadrant::Lagging,
            (false, true) => Quadrant::Improving,
        }
    }
}

/// Построить траекторию RRG сектора относительно бенчмарка.
///
/// `sector` и `benchmark` — выровненные по времени ценовые серии равной длины
/// (напр. индекс сектора и широкий индекс). `period` — окно нормировки.
///
/// Возвращает точки только для тех периодов, где заполнены оба окна (т.е.
/// «хвост» серии); ранние периоды отбрасываются. `None`, если длины не равны,
/// `period == 0` или данных недостаточно.
pub fn rrg_trajectory(sector: &[f64], benchmark: &[f64], period: usize) -> Option<Vec<RrgPoint>> {
    if period == 0 || sector.len() != benchmark.len() || sector.is_empty() {
        return None;
    }
    // Относительная сила, масштабированная к 100.
    let rs: Vec<f64> = sector
        .iter()
        .zip(benchmark)
        .map(|(s, b)| if *b == 0.0 { 100.0 } else { 100.0 * s / b })
        .collect();

    let rs_sma = sma(&rs, period);
    // RS-Ratio: насколько относительная сила выше своей средней.
    let rs_ratio: Vec<Option<f64>> = rs
        .iter()
        .zip(&rs_sma)
        .map(|(v, avg)| avg.map(|a| if a == 0.0 { 100.0 } else { 100.0 * v / a }))
        .collect();

    // Для момента берём SMA по уже посчитанным значениям RS-Ratio.
    let ratio_vals: Vec<f64> = rs_ratio.iter().filter_map(|x| *x).collect();
    let ratio_sma = sma(&ratio_vals, period);

    // Сопоставляем momentum обратно к позициям, где rs_ratio определён.
    let mut points = Vec::new();
    // k — индекс по определённым значениям RS-Ratio (ratio_vals/ratio_sma).
    for (k, ratio) in rs_ratio.iter().flatten().enumerate() {
        if let Some(Some(avg)) = ratio_sma.get(k) {
            let momentum = if *avg == 0.0 {
                100.0
            } else {
                100.0 * ratio / avg
            };
            points.push(RrgPoint {
                rs_ratio: *ratio,
                rs_momentum: momentum,
            });
        }
    }

    if points.is_empty() {
        None
    } else {
        Some(points)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_mismatched_lengths() {
        assert!(rrg_trajectory(&[1.0, 2.0], &[1.0], 2).is_none());
        assert!(rrg_trajectory(&[], &[], 2).is_none());
        assert!(rrg_trajectory(&[1.0], &[1.0], 0).is_none());
    }

    // Бенчмарк держим плоским (100), тогда rs == sector и мы можем задать
    // нужную траекторию относительной силы напрямую. Квадратичный рост/спад даёт
    // ускоряющуюся дивергенцию, поэтому момент уверенно отходит от 100
    // (постоянный темп дал бы момент ровно 100 — это корректно, но не тестирует
    // знак момента).

    #[test]
    fn outperforming_sector_lands_leading() {
        let benchmark: Vec<f64> = (0..30).map(|_| 100.0).collect();
        let sector: Vec<f64> = (0..30).map(|i| 1000.0 + (i * i) as f64).collect();
        let pts = rrg_trajectory(&sector, &benchmark, 5).unwrap();
        let last = *pts.last().unwrap();
        assert!(last.rs_ratio > 100.0, "rs_ratio = {}", last.rs_ratio);
        assert!(last.rs_momentum > 100.0, "rs_mom = {}", last.rs_momentum);
        assert_eq!(last.quadrant(), Quadrant::Leading);
    }

    #[test]
    fn lagging_sector_lands_lagging() {
        let benchmark: Vec<f64> = (0..30).map(|_| 100.0).collect();
        let sector: Vec<f64> = (0..30).map(|i| 2000.0 - (i * i) as f64).collect();
        let pts = rrg_trajectory(&sector, &benchmark, 5).unwrap();
        let last = *pts.last().unwrap();
        assert!(last.rs_ratio < 100.0, "rs_ratio = {}", last.rs_ratio);
        assert!(last.rs_momentum < 100.0, "rs_mom = {}", last.rs_momentum);
        assert_eq!(last.quadrant(), Quadrant::Lagging);
    }

    #[test]
    fn quadrant_boundaries() {
        assert_eq!(
            RrgPoint {
                rs_ratio: 100.0,
                rs_momentum: 100.0
            }
            .quadrant(),
            Quadrant::Leading
        );
        assert_eq!(
            RrgPoint {
                rs_ratio: 101.0,
                rs_momentum: 99.0
            }
            .quadrant(),
            Quadrant::Weakening
        );
        assert_eq!(
            RrgPoint {
                rs_ratio: 99.0,
                rs_momentum: 101.0
            }
            .quadrant(),
            Quadrant::Improving
        );
    }
}
