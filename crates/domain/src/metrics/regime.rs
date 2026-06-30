//! Режим рынка по кросс-актив потокам: «куда уходят большие деньги».
//!
//! Питает вкладку «Сводка» (Summary). На вход — направленный нетто-поток
//! (знаковый, в ₽) по четырём классам: акции / фьючерсы / облигации / FX-спот.
//! На выходе — режим (Risk-ON / Risk-OFF / Neutral) и уверенность сигнала
//! (`conviction`, 0..100), нормированная по суммарной величине перетока.
//!
//! Логика чистая и детерминированная (без сети/времени), поэтому полностью
//! покрывается юнит-тестами и собирается в кросс-платформенном CI. Текстовые
//! пояснения (тезис / решения / риски) — забота UI (`app`/фронт), здесь только
//! числовой сигнал.

/// Направленный нетто-поток по классам активов за период (знаковый, в ₽млрд).
///
/// Положительное значение — приток (деньги пришли в класс), отрицательное —
/// отток. Знаки задаёт вызывающая сторона (агрегатор снимков оборота).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ClassFlows {
    pub equity: f64,
    pub future: f64,
    pub bond: f64,
    pub fx: f64,
}

impl ClassFlows {
    /// Суммарная величина перетока (по модулю) — основа для `conviction`.
    fn magnitude(&self) -> f64 {
        self.equity.abs() + self.bond.abs() + self.fx.abs()
    }
}

/// Режим рынка.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Regime {
    /// Аппетит к риску: деньги идут в акции из защитных активов.
    RiskOn,
    /// Уход от риска: деньги уходят из акций в облигации / валюту.
    RiskOff,
    /// Нет явного межклассового сигнала.
    Neutral,
}

impl Regime {
    /// Машинный код режима (JS-дружелюбный, для DTO/фронта).
    pub fn code(self) -> &'static str {
        match self {
            Regime::RiskOn => "riskOn",
            Regime::RiskOff => "riskOff",
            Regime::Neutral => "neutral",
        }
    }
}

/// Оценка режима: режим + уверенность (0..100).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegimeAssessment {
    pub regime: Regime,
    pub conviction: u8,
}

/// Порог потока по умолчанию (₽млрд): меньше — считаем «нет сигнала».
pub const DEFAULT_FLOW_THRESHOLD: f64 = 10.0;

/// Делитель для нормировки уверенности: `conviction = magnitude / divisor`,
/// зажатое в 0..100. Подобран так, чтобы выраженная защитная ротация
/// (отток из акций десятками ₽млрд + приток в облигации/FX) давала ~100.
const CONVICTION_DIVISOR: f64 = 2.4;

/// Классифицировать режим по нетто-потокам классов.
///
/// Правила (зеркало прототипа дизайна):
/// - отток из акций (`equity < -threshold`) при притоке в защитные активы
///   (`bond + fx > 0`) ⇒ **Risk-OFF**;
/// - приток в акции (`equity > threshold`) при оттоке из облигаций
///   (`bond < 0`) ⇒ **Risk-ON**;
/// - иначе ⇒ **Neutral**.
///
/// `conviction` не зависит от ветки: это нормированная суммарная величина
/// перетока (по модулю), зажатая в 0..100.
pub fn assess_regime(flows: &ClassFlows, threshold: f64) -> RegimeAssessment {
    let defensive_in = flows.bond + flows.fx;
    let regime = if flows.equity < -threshold && defensive_in > 0.0 {
        Regime::RiskOff
    } else if flows.equity > threshold && flows.bond < 0.0 {
        Regime::RiskOn
    } else {
        Regime::Neutral
    };

    let conviction = (flows.magnitude() / CONVICTION_DIVISOR)
        .round()
        .clamp(0.0, 100.0) as u8;

    RegimeAssessment { regime, conviction }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equity_outflow_into_defensives_is_risk_off() {
        let flows = ClassFlows {
            equity: -120.0,
            future: -14.0,
            bond: 70.0,
            fx: 70.0,
        };
        let a = assess_regime(&flows, DEFAULT_FLOW_THRESHOLD);
        assert_eq!(a.regime, Regime::RiskOff);
        // magnitude = 120 + 70 + 70 = 260; 260/2.4 ≈ 108 → зажато в 100.
        assert_eq!(a.conviction, 100);
    }

    #[test]
    fn equity_inflow_out_of_bonds_is_risk_on() {
        let flows = ClassFlows {
            equity: 40.0,
            future: 5.0,
            bond: -20.0,
            fx: -10.0,
        };
        let a = assess_regime(&flows, DEFAULT_FLOW_THRESHOLD);
        assert_eq!(a.regime, Regime::RiskOn);
        // magnitude = 40 + 20 + 10 = 70; 70/2.4 ≈ 29.
        assert_eq!(a.conviction, 29);
    }

    #[test]
    fn small_flows_are_neutral() {
        let flows = ClassFlows {
            equity: 4.0,
            future: 2.0,
            bond: -1.0,
            fx: 1.0,
        };
        let a = assess_regime(&flows, DEFAULT_FLOW_THRESHOLD);
        assert_eq!(a.regime, Regime::Neutral);
    }

    #[test]
    fn equity_outflow_without_defensive_inflow_is_neutral() {
        // Деньги ушли из акций, но и из облигаций/FX тоже (не защитная ротация).
        let flows = ClassFlows {
            equity: -50.0,
            future: 5.0,
            bond: -10.0,
            fx: -5.0,
        };
        let a = assess_regime(&flows, DEFAULT_FLOW_THRESHOLD);
        assert_eq!(a.regime, Regime::Neutral);
    }

    #[test]
    fn conviction_is_clamped_and_nonnegative() {
        let zero = ClassFlows::default();
        assert_eq!(assess_regime(&zero, DEFAULT_FLOW_THRESHOLD).conviction, 0);
    }

    #[test]
    fn regime_codes_are_stable() {
        assert_eq!(Regime::RiskOn.code(), "riskOn");
        assert_eq!(Regime::RiskOff.code(), "riskOff");
        assert_eq!(Regime::Neutral.code(), "neutral");
    }
}
