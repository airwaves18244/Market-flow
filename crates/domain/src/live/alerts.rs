//! Движок алёртов: правила и их срабатывание по снимку рынка.
//!
//! Правило [`AlertRule`] описывает условие на инструмент (цена, изменение,
//! всплеск объёма, спред). [`check`] проверяет одно правило против
//! [`MarketSnapshot`], [`evaluate`] — набор правил, доставая снимок по символу.
//! Логика чистая: доставку уведомлений и хранение правил берёт на себя `app`.

use serde::{Deserialize, Serialize};

/// Условие срабатывания алёрта. Сериализуется тегированно
/// (`{"kind":"price_above","value":300.0}`) — удобно для фронта.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum AlertKind {
    /// Цена достигла/превысила порог.
    PriceAbove(f64),
    /// Цена опустилась до/ниже порога.
    PriceBelow(f64),
    /// Абсолютное изменение к закрытию ≥ порога (в долях, `0.05` = 5%).
    PctChange(f64),
    /// Объём ≥ множитель × средний объём.
    VolumeSpike(f64),
    /// Спред ≥ порога (в деньгах).
    SpreadAbove(f64),
}

/// Серьёзность сработавшего алёрта.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

/// Правило алёрта, привязанное к инструменту.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertRule {
    /// Стабильный идентификатор правила.
    pub id: String,
    /// Символ инструмента (`SBER@MISX`).
    pub symbol: String,
    /// Условие.
    pub kind: AlertKind,
}

/// Снимок рынка по инструменту для проверки правил.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketSnapshot {
    /// Последняя цена.
    pub last: f64,
    /// Цена закрытия предыдущего дня (база для изменения).
    pub prev_close: f64,
    /// Текущий накопленный объём.
    pub volume: f64,
    /// Средний (нормальный) объём для сравнения.
    pub avg_volume: f64,
    /// Текущий спред (в деньгах).
    pub spread: f64,
}

impl MarketSnapshot {
    /// Изменение к закрытию в долях (`0.01` = +1%). `0`, если базы нет.
    pub fn pct_change(&self) -> f64 {
        if self.prev_close == 0.0 {
            0.0
        } else {
            (self.last - self.prev_close) / self.prev_close
        }
    }
}

/// Сработавший алёрт — готовое к показу уведомление.
#[derive(Debug, Clone, PartialEq)]
pub struct TriggeredAlert {
    pub rule_id: String,
    pub symbol: String,
    pub message: String,
    pub severity: Severity,
}

/// Проверить одно правило против снимка. `None`, если не сработало.
pub fn check(rule: &AlertRule, snap: &MarketSnapshot) -> Option<TriggeredAlert> {
    let (fired, message, severity) = match rule.kind {
        AlertKind::PriceAbove(th) => (
            snap.last >= th,
            format!("цена {:.2} ≥ {:.2}", snap.last, th),
            Severity::Warning,
        ),
        AlertKind::PriceBelow(th) => (
            snap.last <= th,
            format!("цена {:.2} ≤ {:.2}", snap.last, th),
            Severity::Warning,
        ),
        AlertKind::PctChange(th) => {
            let change = snap.pct_change();
            (
                change.abs() >= th,
                format!(
                    "изменение {:+.2}% (порог {:.2}%)",
                    change * 100.0,
                    th * 100.0
                ),
                Severity::Info,
            )
        }
        AlertKind::VolumeSpike(mult) => (
            snap.avg_volume > 0.0 && snap.volume >= mult * snap.avg_volume,
            format!(
                "объём {:.0} ≥ {:.1}× среднего ({:.0})",
                snap.volume, mult, snap.avg_volume
            ),
            Severity::Critical,
        ),
        AlertKind::SpreadAbove(th) => (
            snap.spread >= th,
            format!("спред {:.2} ≥ {:.2}", snap.spread, th),
            Severity::Info,
        ),
    };
    if fired {
        Some(TriggeredAlert {
            rule_id: rule.id.clone(),
            symbol: rule.symbol.clone(),
            message,
            severity,
        })
    } else {
        None
    }
}

/// Проверить набор правил, доставая снимок по символу через `snapshot_for`.
/// Возвращает только сработавшие, сохраняя порядок правил.
pub fn evaluate(
    rules: &[AlertRule],
    snapshot_for: impl Fn(&str) -> Option<MarketSnapshot>,
) -> Vec<TriggeredAlert> {
    rules
        .iter()
        .filter_map(|r| snapshot_for(&r.symbol).and_then(|s| check(r, &s)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(last: f64, prev: f64, vol: f64, avg: f64, spread: f64) -> MarketSnapshot {
        MarketSnapshot {
            last,
            prev_close: prev,
            volume: vol,
            avg_volume: avg,
            spread,
        }
    }

    fn rule(kind: AlertKind) -> AlertRule {
        AlertRule {
            id: "r1".into(),
            symbol: "SBER@MISX".into(),
            kind,
        }
    }

    #[test]
    fn price_above_fires_at_threshold() {
        let s = snap(305.0, 300.0, 0.0, 0.0, 0.0);
        assert!(check(&rule(AlertKind::PriceAbove(300.0)), &s).is_some());
        assert!(check(&rule(AlertKind::PriceAbove(310.0)), &s).is_none());
    }

    #[test]
    fn price_below_fires() {
        let s = snap(290.0, 300.0, 0.0, 0.0, 0.0);
        assert!(check(&rule(AlertKind::PriceBelow(295.0)), &s).is_some());
        assert!(check(&rule(AlertKind::PriceBelow(280.0)), &s).is_none());
    }

    #[test]
    fn pct_change_uses_absolute_value() {
        let down = snap(270.0, 300.0, 0.0, 0.0, 0.0); // -10%
        assert!(check(&rule(AlertKind::PctChange(0.05)), &down).is_some());
        let flat = snap(303.0, 300.0, 0.0, 0.0, 0.0); // +1%
        assert!(check(&rule(AlertKind::PctChange(0.05)), &flat).is_none());
    }

    #[test]
    fn volume_spike_needs_positive_average() {
        let spiked = snap(300.0, 300.0, 5_000.0, 1_000.0, 0.0);
        assert!(check(&rule(AlertKind::VolumeSpike(3.0)), &spiked).is_some());
        // нулевой средний объём не должен ложно срабатывать
        let no_avg = snap(300.0, 300.0, 5_000.0, 0.0, 0.0);
        assert!(check(&rule(AlertKind::VolumeSpike(3.0)), &no_avg).is_none());
    }

    #[test]
    fn spread_above_fires() {
        let wide = snap(300.0, 300.0, 0.0, 0.0, 1.5);
        assert!(check(&rule(AlertKind::SpreadAbove(1.0)), &wide).is_some());
    }

    #[test]
    fn severity_is_assigned_per_kind() {
        let s = snap(305.0, 300.0, 5_000.0, 1_000.0, 2.0);
        assert_eq!(
            check(&rule(AlertKind::PriceAbove(300.0)), &s)
                .unwrap()
                .severity,
            Severity::Warning
        );
        assert_eq!(
            check(&rule(AlertKind::VolumeSpike(3.0)), &s)
                .unwrap()
                .severity,
            Severity::Critical
        );
        assert_eq!(
            check(&rule(AlertKind::SpreadAbove(1.0)), &s)
                .unwrap()
                .severity,
            Severity::Info
        );
    }

    #[test]
    fn evaluate_filters_to_fired_and_keeps_order() {
        let rules = vec![
            AlertRule {
                id: "a".into(),
                symbol: "SBER@MISX".into(),
                kind: AlertKind::PriceAbove(300.0),
            },
            AlertRule {
                id: "b".into(),
                symbol: "LKOH@MISX".into(),
                kind: AlertKind::PriceAbove(9_999.0),
            },
            AlertRule {
                id: "c".into(),
                symbol: "MISSING".into(),
                kind: AlertKind::PriceAbove(1.0),
            },
        ];
        let fired = evaluate(&rules, |sym| match sym {
            "SBER@MISX" => Some(snap(305.0, 300.0, 0.0, 0.0, 0.0)),
            "LKOH@MISX" => Some(snap(7_000.0, 7_000.0, 0.0, 0.0, 0.0)),
            _ => None, // MISSING — нет снимка
        });
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].rule_id, "a");
    }
}
