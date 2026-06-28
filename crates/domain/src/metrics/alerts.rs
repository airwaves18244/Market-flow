//! Алёрты по рыночным условиям (Фаза 7).
//!
//! Чистый, фронт-агностичный движок правил: на вход — наблюдение по инструменту
//! ([`Observation`]: цена, дневное изменение), на выход — события срабатывания
//! ([`AlertEvent`]). [`AlertEngine`] срабатывает по фронту (edge-triggered):
//! событие генерируется один раз при переходе условия из «ложно» в «истинно» и
//! сбрасывается, когда условие перестаёт выполняться, — чтобы не спамить
//! одинаковыми алёртами на каждом тике.

/// Наблюдение по инструменту на момент `ts` (UNIX-секунды).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Observation {
    pub ts: i64,
    /// Последняя цена.
    pub price: f64,
    /// Дневное изменение в долях (`0.01` = +1%).
    pub change: f64,
}

/// Условие срабатывания алёрта.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlertCondition {
    /// Цена выше порога.
    PriceAbove(f64),
    /// Цена ниже порога.
    PriceBelow(f64),
    /// Дневное изменение выше порога (в долях).
    ChangeAbove(f64),
    /// Дневное изменение ниже порога (в долях).
    ChangeBelow(f64),
}

impl AlertCondition {
    /// Выполняется ли условие для наблюдения.
    pub fn holds(&self, obs: &Observation) -> bool {
        match *self {
            AlertCondition::PriceAbove(t) => obs.price > t,
            AlertCondition::PriceBelow(t) => obs.price < t,
            AlertCondition::ChangeAbove(t) => obs.change > t,
            AlertCondition::ChangeBelow(t) => obs.change < t,
        }
    }

    fn describe(&self) -> String {
        match *self {
            AlertCondition::PriceAbove(t) => format!("цена выше {t}"),
            AlertCondition::PriceBelow(t) => format!("цена ниже {t}"),
            AlertCondition::ChangeAbove(t) => format!("изменение выше {:+.2}%", t * 100.0),
            AlertCondition::ChangeBelow(t) => format!("изменение ниже {:+.2}%", t * 100.0),
        }
    }
}

/// Правило: условие по конкретному инструменту.
#[derive(Debug, Clone, PartialEq)]
pub struct AlertRule {
    pub symbol: String,
    pub condition: AlertCondition,
}

impl AlertRule {
    pub fn new(symbol: impl Into<String>, condition: AlertCondition) -> Self {
        Self {
            symbol: symbol.into(),
            condition,
        }
    }
}

/// Событие срабатывания алёрта.
#[derive(Debug, Clone, PartialEq)]
pub struct AlertEvent {
    pub symbol: String,
    pub ts: i64,
    pub price: f64,
    pub change: f64,
    /// Человекочитаемое описание сработавшего условия.
    pub message: String,
}

/// Движок алёртов с фронтовым срабатыванием.
#[derive(Debug, Clone)]
pub struct AlertEngine {
    rules: Vec<AlertRule>,
    /// Параллельно правилам: было ли условие выполнено на прошлом наблюдении.
    active: Vec<bool>,
}

impl AlertEngine {
    /// Движок из набора правил.
    pub fn new(rules: Vec<AlertRule>) -> Self {
        let active = vec![false; rules.len()];
        Self { rules, active }
    }

    /// Число правил.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Обработать наблюдение по `symbol`. Возвращает события, сработавшие именно
    /// сейчас (переход условия из «ложно» в «истинно»). Правила других
    /// инструментов не затрагиваются.
    pub fn observe(&mut self, symbol: &str, obs: &Observation) -> Vec<AlertEvent> {
        let mut fired = Vec::new();
        for (i, rule) in self.rules.iter().enumerate() {
            if rule.symbol != symbol {
                continue;
            }
            if rule.condition.holds(obs) {
                if !self.active[i] {
                    self.active[i] = true;
                    fired.push(AlertEvent {
                        symbol: rule.symbol.clone(),
                        ts: obs.ts,
                        price: obs.price,
                        change: obs.change,
                        message: rule.condition.describe(),
                    });
                }
            } else {
                self.active[i] = false;
            }
        }
        fired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(ts: i64, price: f64, change: f64) -> Observation {
        Observation { ts, price, change }
    }

    #[test]
    fn price_thresholds_trigger() {
        let mut e = AlertEngine::new(vec![
            AlertRule::new("SBER@MISX", AlertCondition::PriceAbove(300.0)),
            AlertRule::new("SBER@MISX", AlertCondition::PriceBelow(250.0)),
        ]);
        assert!(e.observe("SBER@MISX", &obs(1, 290.0, 0.0)).is_empty());
        let fired = e.observe("SBER@MISX", &obs(2, 305.0, 0.0));
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].symbol, "SBER@MISX");
        assert!(fired[0].message.contains("выше"));
    }

    #[test]
    fn edge_triggered_does_not_repeat() {
        let mut e = AlertEngine::new(vec![AlertRule::new("X", AlertCondition::PriceAbove(100.0))]);
        assert_eq!(e.observe("X", &obs(1, 101.0, 0.0)).len(), 1); // переход → срабатывает
        assert_eq!(e.observe("X", &obs(2, 102.0, 0.0)).len(), 0); // всё ещё true → молчит
        assert_eq!(e.observe("X", &obs(3, 99.0, 0.0)).len(), 0); // сброс
        assert_eq!(e.observe("X", &obs(4, 105.0, 0.0)).len(), 1); // снова переход → срабатывает
    }

    #[test]
    fn change_conditions_and_symbol_filtering() {
        let mut e = AlertEngine::new(vec![
            AlertRule::new("A", AlertCondition::ChangeAbove(0.05)),
            AlertRule::new("B", AlertCondition::ChangeBelow(-0.05)),
        ]);
        // Наблюдение по A не трогает правило B.
        assert!(e.observe("A", &obs(1, 10.0, 0.06)).len() == 1);
        assert!(e.observe("A", &obs(2, 10.0, -0.10)).is_empty()); // правило B не для A
        let fired_b = e.observe("B", &obs(3, 20.0, -0.07));
        assert_eq!(fired_b.len(), 1);
        assert!(fired_b[0].message.contains("ниже"));
    }

    #[test]
    fn empty_engine_is_inert() {
        let mut e = AlertEngine::new(Vec::new());
        assert!(e.is_empty());
        assert!(e.observe("X", &obs(1, 1.0, 1.0)).is_empty());
    }
}
