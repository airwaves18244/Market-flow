//! Mega Alerts: чистый движок сигналов поверх метрик ALGOPACK
//! (tradestats/futoi/obstats/hi2).
//!
//! Встроенные детекторы: всплеск объёма (z-score), крупный дисбаланс
//! покупок/продаж, расширение спреда, скачок открытого интереса, рост
//! концентрации HI2. Срабатывание по фронту (edge-trigger) — событие
//! генерируется один раз при переходе условия в «истинно» и сбрасывается, когда
//! условие перестаёт выполняться (как [`crate::metrics::alerts::AlertEngine`]).
//! Пороги параметризуемы.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Сводное наблюдение по инструменту для детекторов Mega Alerts.
///
/// Поля, которых нет в текущем такте (например, спред без obstats),
/// передаются как `None` — соответствующий детектор тогда не срабатывает.
#[derive(Debug, Clone, PartialEq)]
pub struct MegaObservation {
    pub ts: i64,
    /// z-score объёма (из tradestats, см. [`super::tradestats::volume_zscore`]).
    pub vol_z: Option<f64>,
    /// Дисбаланс потока −1..1 (`disb`).
    pub disb: Option<f64>,
    /// Относительный спред BBO (из obstats), доля.
    pub spread: Option<f64>,
    /// Изменение нетто-OI за период (из futoi).
    pub oi_change: Option<f64>,
    /// Индекс концентрации HI2.
    pub hi2: Option<f64>,
}

impl MegaObservation {
    /// Пустое наблюдение (все метрики отсутствуют) на момент `ts`.
    pub fn at(ts: i64) -> Self {
        MegaObservation {
            ts,
            vol_z: None,
            disb: None,
            spread: None,
            oi_change: None,
            hi2: None,
        }
    }
}

/// Тип Mega-сигнала.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MegaAlertKind {
    /// Всплеск объёма.
    VolumeSpike,
    /// Крупный перевес покупок.
    BuyImbalance,
    /// Крупный перевес продаж.
    SellImbalance,
    /// Расширение спреда.
    SpreadWidening,
    /// Скачок открытого интереса.
    OiJump,
    /// Рост концентрации участников (HI2).
    ConcentrationRise,
}

impl MegaAlertKind {
    fn describe(self) -> &'static str {
        match self {
            MegaAlertKind::VolumeSpike => "всплеск объёма",
            MegaAlertKind::BuyImbalance => "перевес покупок",
            MegaAlertKind::SellImbalance => "перевес продаж",
            MegaAlertKind::SpreadWidening => "расширение спреда",
            MegaAlertKind::OiJump => "скачок открытого интереса",
            MegaAlertKind::ConcentrationRise => "рост концентрации",
        }
    }

    /// Машинный код типа сигнала (для сериализации во фронт/IPC) — то же, что
    /// даёт `#[serde(rename_all = "snake_case")]` на этом enum, но как метод,
    /// удобный без похода через `serde_json`.
    pub fn code(self) -> &'static str {
        match self {
            MegaAlertKind::VolumeSpike => "volume_spike",
            MegaAlertKind::BuyImbalance => "buy_imbalance",
            MegaAlertKind::SellImbalance => "sell_imbalance",
            MegaAlertKind::SpreadWidening => "spread_widening",
            MegaAlertKind::OiJump => "oi_jump",
            MegaAlertKind::ConcentrationRise => "concentration_rise",
        }
    }
}

/// Пороги детекторов.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MegaThresholds {
    /// Порог z-score объёма.
    pub vol_z: f64,
    /// Порог |дисбаланса| (0..1).
    pub disb: f64,
    /// Порог относительного спреда.
    pub spread: f64,
    /// Порог |изменения OI|.
    pub oi_jump: f64,
    /// Порог индекса концентрации.
    pub hi2: f64,
}

impl Default for MegaThresholds {
    fn default() -> Self {
        MegaThresholds {
            vol_z: 3.0,
            disb: 0.5,
            spread: 0.01,
            oi_jump: 1000.0,
            hi2: 0.4,
        }
    }
}

/// Сработавший Mega-сигнал.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MegaAlert {
    pub secid: String,
    pub ts: i64,
    pub kind: MegaAlertKind,
    /// Значение метрики, вызвавшей сигнал.
    pub value: f64,
    /// Человекочитаемое описание.
    pub message: String,
}

/// Движок Mega Alerts с фронтовым срабатыванием по инструменту и типу.
#[derive(Debug, Clone)]
pub struct MegaAlertEngine {
    thresholds: MegaThresholds,
    /// Состояние «условие активно» по ключу `(secid, kind)`.
    active: HashMap<(String, MegaAlertKind), bool>,
}

impl MegaAlertEngine {
    /// Движок с заданными порогами.
    pub fn new(thresholds: MegaThresholds) -> Self {
        MegaAlertEngine {
            thresholds,
            active: HashMap::new(),
        }
    }

    /// Движок с порогами по умолчанию.
    pub fn with_defaults() -> Self {
        Self::new(MegaThresholds::default())
    }

    /// Обработать наблюдение по `secid`. Возвращает сигналы, сработавшие именно
    /// сейчас (переход условия false→true).
    pub fn observe(&mut self, secid: &str, obs: &MegaObservation) -> Vec<MegaAlert> {
        let t = self.thresholds;
        // (тип, выполняется ли условие, значение метрики)
        let checks: [(MegaAlertKind, bool, f64); 6] = [
            (
                MegaAlertKind::VolumeSpike,
                obs.vol_z.is_some_and(|z| z >= t.vol_z),
                obs.vol_z.unwrap_or(0.0),
            ),
            (
                MegaAlertKind::BuyImbalance,
                obs.disb.is_some_and(|d| d >= t.disb),
                obs.disb.unwrap_or(0.0),
            ),
            (
                MegaAlertKind::SellImbalance,
                obs.disb.is_some_and(|d| d <= -t.disb),
                obs.disb.unwrap_or(0.0),
            ),
            (
                MegaAlertKind::SpreadWidening,
                obs.spread.is_some_and(|s| s >= t.spread),
                obs.spread.unwrap_or(0.0),
            ),
            (
                MegaAlertKind::OiJump,
                obs.oi_change.is_some_and(|o| o.abs() >= t.oi_jump),
                obs.oi_change.unwrap_or(0.0),
            ),
            (
                MegaAlertKind::ConcentrationRise,
                obs.hi2.is_some_and(|h| h >= t.hi2),
                obs.hi2.unwrap_or(0.0),
            ),
        ];

        let mut fired = Vec::new();
        for (kind, holds, value) in checks {
            let key = (secid.to_string(), kind);
            let was_active = self.active.get(&key).copied().unwrap_or(false);
            if holds {
                if !was_active {
                    self.active.insert(key, true);
                    fired.push(MegaAlert {
                        secid: secid.to_string(),
                        ts: obs.ts,
                        kind,
                        value,
                        message: kind.describe().to_string(),
                    });
                }
            } else {
                self.active.insert(key, false);
            }
        }
        fired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_spike_edge_triggers_once() {
        let mut e = MegaAlertEngine::with_defaults();
        let mut o = MegaObservation::at(1);
        o.vol_z = Some(4.0);
        assert_eq!(e.observe("SBER", &o).len(), 1);
        // Всё ещё выше порога — молчит.
        let mut o2 = MegaObservation::at(2);
        o2.vol_z = Some(5.0);
        assert_eq!(e.observe("SBER", &o2).len(), 0);
        // Спад ниже порога — сброс.
        let mut o3 = MegaObservation::at(3);
        o3.vol_z = Some(0.5);
        assert_eq!(e.observe("SBER", &o3).len(), 0);
        // Снова всплеск — снова сигнал.
        let mut o4 = MegaObservation::at(4);
        o4.vol_z = Some(4.0);
        assert_eq!(e.observe("SBER", &o4).len(), 1);
    }

    #[test]
    fn buy_and_sell_imbalance_are_distinct() {
        let mut e = MegaAlertEngine::with_defaults();
        let mut buy = MegaObservation::at(1);
        buy.disb = Some(0.7);
        let fired = e.observe("X", &buy);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].kind, MegaAlertKind::BuyImbalance);

        let mut sell = MegaObservation::at(2);
        sell.disb = Some(-0.8);
        let fired = e.observe("X", &sell);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].kind, MegaAlertKind::SellImbalance);
    }

    #[test]
    fn missing_metrics_do_not_fire() {
        let mut e = MegaAlertEngine::with_defaults();
        let o = MegaObservation::at(1); // всё None
        assert!(e.observe("X", &o).is_empty());
    }

    #[test]
    fn oi_jump_uses_absolute_value() {
        let mut e = MegaAlertEngine::with_defaults();
        let mut o = MegaObservation::at(1);
        o.oi_change = Some(-2000.0);
        let fired = e.observe("RIH5", &o);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].kind, MegaAlertKind::OiJump);
    }

    #[test]
    fn instruments_are_independent() {
        let mut e = MegaAlertEngine::with_defaults();
        let mut o = MegaObservation::at(1);
        o.hi2 = Some(0.6);
        assert_eq!(e.observe("A", &o).len(), 1);
        // Тот же сигнал для другого инструмента — независимое состояние.
        assert_eq!(e.observe("B", &o).len(), 1);
    }

    #[test]
    fn kind_codes_are_stable_and_unique() {
        let kinds = [
            MegaAlertKind::VolumeSpike,
            MegaAlertKind::BuyImbalance,
            MegaAlertKind::SellImbalance,
            MegaAlertKind::SpreadWidening,
            MegaAlertKind::OiJump,
            MegaAlertKind::ConcentrationRise,
        ];
        let mut seen = std::collections::HashSet::new();
        for k in kinds {
            assert!(seen.insert(k.code()), "дубликат кода: {}", k.code());
        }
        assert_eq!(MegaAlertKind::VolumeSpike.code(), "volume_spike");
        assert_eq!(
            MegaAlertKind::ConcentrationRise.code(),
            "concentration_rise"
        );
    }
}
