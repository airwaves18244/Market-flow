//! Кросс-актив: доли оборота по классам активов и матрица перетоков.
//!
//! Питает представление «Сумма всех»: donut долей оборота, stacked area доли во
//! времени и Sankey перетоков период-к-периоду. «Переток» здесь — это сдвиг
//! _доли_ оборота между классами активов от прошлого периода к текущему.

use crate::model::AssetClass;
use std::collections::BTreeMap;

/// Доли оборота по классам активов (значения суммируются в 1.0, если общий
/// оборот положителен).
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct TurnoverShares {
    pub total: f64,
    /// Доля каждого класса (0..1). Отсутствующий класс трактуется как 0.
    pub shares: BTreeMap<AssetClass, f64>,
}

impl TurnoverShares {
    pub fn share_of(&self, class: AssetClass) -> f64 {
        self.shares.get(&class).copied().unwrap_or(0.0)
    }
}

/// Посчитать доли оборота по классам активов.
pub fn turnover_shares(turnover_by_class: &BTreeMap<AssetClass, f64>) -> TurnoverShares {
    let total: f64 = turnover_by_class.values().copied().sum();
    let mut shares = BTreeMap::new();
    if total > 0.0 {
        for (&class, &t) in turnover_by_class {
            shares.insert(class, t / total);
        }
    }
    TurnoverShares { total, shares }
}

/// Ребро перетока для Sankey: из класса `from` в класс `to` «перетекла» доля
/// `weight` (в долях общего оборота).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FlowEdge {
    pub from: AssetClass,
    pub to: AssetClass,
    pub weight: f64,
}

/// Построить ребра перетоков между двумя периодами.
///
/// Модель проста и наглядна: классы, у которых доля _упала_, отдали суммарно
/// `Σ снижений` долей; классы, у которых доля _выросла_, забрали `Σ ростов`.
/// Перетоки распределяются пропорционально (источники × приёмники), что и
/// рисует Sankey. Сумма весаов рёбер равна суммарному сдвигу долей.
///
/// Возвращает только значимые рёбра (вес ≥ `min_weight`).
pub fn flow_matrix(prev: &TurnoverShares, curr: &TurnoverShares, min_weight: f64) -> Vec<FlowEdge> {
    // dec: кто отдал долю (положительная величина потери), inc: кто забрал.
    let mut sources: Vec<(AssetClass, f64)> = Vec::new();
    let mut sinks: Vec<(AssetClass, f64)> = Vec::new();

    for &class in &AssetClass::ALL {
        let delta = curr.share_of(class) - prev.share_of(class);
        if delta < 0.0 {
            sources.push((class, -delta));
        } else if delta > 0.0 {
            sinks.push((class, delta));
        }
    }

    let total_out: f64 = sources.iter().map(|(_, w)| w).sum();
    if total_out <= 0.0 {
        return Vec::new();
    }

    let mut edges = Vec::new();
    for &(src, out) in &sources {
        for &(dst, inc) in &sinks {
            // доля приёмника в общем притоке × сколько отдал источник
            let weight = out * (inc / total_out);
            if weight >= min_weight {
                edges.push(FlowEdge {
                    from: src,
                    to: dst,
                    weight,
                });
            }
        }
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shares(eq: f64, fut: f64, bond: f64) -> TurnoverShares {
        let mut m = BTreeMap::new();
        m.insert(AssetClass::Equity, eq);
        m.insert(AssetClass::Future, fut);
        m.insert(AssetClass::Bond, bond);
        turnover_shares(&m)
    }

    #[test]
    fn shares_sum_to_one() {
        let s = shares(60.0, 30.0, 10.0);
        assert_eq!(s.total, 100.0);
        assert!((s.share_of(AssetClass::Equity) - 0.6).abs() < 1e-12);
        assert!((s.share_of(AssetClass::Future) - 0.3).abs() < 1e-12);
        assert!((s.share_of(AssetClass::Bond) - 0.1).abs() < 1e-12);
    }

    #[test]
    fn empty_turnover_has_no_shares() {
        let s = shares(0.0, 0.0, 0.0);
        assert!(s.shares.is_empty());
        assert_eq!(s.share_of(AssetClass::Equity), 0.0);
    }

    #[test]
    fn flow_goes_from_losers_to_gainers() {
        // Деньги ушли из акций (0.6 -> 0.4) в фьючерсы (0.3 -> 0.5).
        let prev = shares(60.0, 30.0, 10.0);
        let curr = shares(40.0, 50.0, 10.0);
        let edges = flow_matrix(&prev, &curr, 0.0);
        assert_eq!(edges.len(), 1);
        let e = edges[0];
        assert_eq!(e.from, AssetClass::Equity);
        assert_eq!(e.to, AssetClass::Future);
        assert!((e.weight - 0.2).abs() < 1e-12); // переток 20% доли
    }

    #[test]
    fn no_change_means_no_edges() {
        let s = shares(50.0, 30.0, 20.0);
        assert!(flow_matrix(&s, &s, 0.0).is_empty());
    }

    #[test]
    fn weights_conserve_total_shift() {
        // один источник, два приёмника
        let prev = shares(80.0, 10.0, 10.0);
        let curr = shares(40.0, 30.0, 30.0); // акции -0.4, фьюч +0.2, бонды +0.2
        let edges = flow_matrix(&prev, &curr, 0.0);
        let total: f64 = edges.iter().map(|e| e.weight).sum();
        assert!((total - 0.4).abs() < 1e-12);
    }
}
