//! Встроенная библиотека стратегий — «хорошо известные сценарии».
//!
//! Каждая стратегия — это `Strategy`-реализация, выбираемая по `id` и
//! настраиваемая параметрами. Часть сценариев (равные лоты, айсберг) зеркалит
//! детектирующих роботов вкладки Delta: там их *распознают* на ленте, здесь —
//! *торгуют* в бэктесте.
//!
//! Список и схема параметров отдаются в UI через [`descriptors`]; фабрика
//! [`strategy_from_id`] собирает стратегию из `id` + параметров.

use crate::metrics::sma;

use super::strategy::{param, BarContext, Signal, Strategy, StrategyParams};

/// Описание одного параметра стратегии (для формы в UI).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamSpec {
    pub name: &'static str,
    pub label: &'static str,
    pub default: f64,
}

/// Описание стратегии: идентификатор, подпись и схема параметров.
#[derive(Debug, Clone, PartialEq)]
pub struct StrategyDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub params: Vec<ParamSpec>,
}

/// Каталог встроенных стратегий с их параметрами (для UI и валидации).
pub fn descriptors() -> Vec<StrategyDescriptor> {
    vec![
        StrategyDescriptor {
            id: "ma_cross",
            label: "Пересечение скользящих (MA cross)",
            params: vec![
                ParamSpec { name: "fast", label: "Быстрая MA", default: 5.0 },
                ParamSpec { name: "slow", label: "Медленная MA", default: 20.0 },
                ParamSpec { name: "lot", label: "Лот", default: 1.0 },
            ],
        },
        StrategyDescriptor {
            id: "same_lot",
            label: "Равные лоты (пробой)",
            params: vec![
                ParamSpec { name: "lot", label: "Лот", default: 1.0 },
                ParamSpec { name: "lookback", label: "Окно пробоя", default: 10.0 },
            ],
        },
        StrategyDescriptor {
            id: "iceberg",
            label: "Айсберг (набор равными клипами)",
            params: vec![
                ParamSpec { name: "clip", label: "Клип", default: 1.0 },
                ParamSpec { name: "clips", label: "Число клипов", default: 5.0 },
                ParamSpec { name: "period", label: "Период тренда", default: 20.0 },
            ],
        },
        StrategyDescriptor {
            id: "cvd_momentum",
            label: "Импульс дельты объёма (CVD)",
            params: vec![
                ParamSpec { name: "lot", label: "Лот", default: 1.0 },
                ParamSpec { name: "period", label: "Окно дельты", default: 14.0 },
            ],
        },
    ]
}

/// Собрать стратегию по идентификатору и параметрам. `None` — неизвестный `id`.
pub fn strategy_from_id(id: &str, params: &StrategyParams) -> Option<Box<dyn Strategy>> {
    Some(match id {
        "ma_cross" => Box::new(MaCrossStrategy {
            fast: param(params, "fast", 5.0).max(1.0) as usize,
            slow: param(params, "slow", 20.0).max(2.0) as usize,
            lot: param(params, "lot", 1.0),
        }),
        "same_lot" => Box::new(SameLotStrategy {
            lot: param(params, "lot", 1.0),
            lookback: param(params, "lookback", 10.0).max(1.0) as usize,
        }),
        "iceberg" => Box::new(IcebergStrategy {
            clip: param(params, "clip", 1.0),
            clips: param(params, "clips", 5.0).max(1.0),
            period: param(params, "period", 20.0).max(1.0) as usize,
        }),
        "cvd_momentum" => Box::new(CvdMomentumStrategy {
            lot: param(params, "lot", 1.0),
            period: param(params, "period", 14.0).max(1.0) as usize,
        }),
        _ => return None,
    })
}

/// Пересечение быстрой и медленной скользящих средних по ценам закрытия.
/// Лонг, пока быстрая выше медленной; шорт — пока ниже (реверсивная).
pub struct MaCrossStrategy {
    pub fast: usize,
    pub slow: usize,
    pub lot: f64,
}

impl Strategy for MaCrossStrategy {
    fn id(&self) -> &'static str {
        "ma_cross"
    }
    fn on_bar(&mut self, ctx: &BarContext) -> Option<Signal> {
        let closes = ctx.closes();
        if closes.len() < self.slow {
            return None;
        }
        let fast = (*sma(&closes, self.fast).last()?)?;
        let slow = (*sma(&closes, self.slow).last()?)?;
        if fast > slow {
            Some(Signal::target(self.lot))
        } else if fast < slow {
            Some(Signal::target(-self.lot))
        } else {
            None
        }
    }
}

/// Пробой канала фиксированным («равным») лотом: лонг при закрытии выше цены
/// `lookback` баров назад, шорт — при закрытии ниже. Каждый вход — один и тот
/// же лот (сигнатура «равных лотов»).
pub struct SameLotStrategy {
    pub lot: f64,
    pub lookback: usize,
}

impl Strategy for SameLotStrategy {
    fn id(&self) -> &'static str {
        "same_lot"
    }
    fn on_bar(&mut self, ctx: &BarContext) -> Option<Signal> {
        let n = ctx.bars.len();
        if n <= self.lookback {
            return None;
        }
        let reference = ctx.bars[n - 1 - self.lookback].close;
        let close = ctx.current().close;
        if close > reference {
            Some(Signal::target(self.lot))
        } else if close < reference {
            Some(Signal::target(-self.lot))
        } else {
            None
        }
    }
}

/// Набор позиции равными клипами в стиле айсберга: пока цена выше скользящей,
/// добавляем по одному клипу до `clip × clips`; пока ниже — сокращаем/переворачиваем
/// тем же клипом. Каждая сделка — ровно один клип (равные лоты).
pub struct IcebergStrategy {
    pub clip: f64,
    pub clips: f64,
    pub period: usize,
}

impl Strategy for IcebergStrategy {
    fn id(&self) -> &'static str {
        "iceberg"
    }
    fn on_bar(&mut self, ctx: &BarContext) -> Option<Signal> {
        let closes = ctx.closes();
        if closes.len() < self.period {
            return None;
        }
        let ma = (*sma(&closes, self.period).last()?)?;
        let close = ctx.current().close;
        let cap = self.clip * self.clips;
        let dir = if close > ma {
            1.0
        } else if close < ma {
            -1.0
        } else {
            return None;
        };
        // Сдвиг текущей позиции на один клип в сторону тренда, с ограничением ±cap.
        let target = (ctx.position + dir * self.clip).clamp(-cap, cap);
        Some(Signal::target(target))
    }
}

/// Импульс дельты объёма. По барам строит прокси дельты
/// `sign(close − open) × volume` (аналог накопленной дельты объёма, CVD, когда
/// тиковой ленты нет), и берёт лонг при положительной дельте за окно, шорт — при
/// отрицательной.
pub struct CvdMomentumStrategy {
    pub lot: f64,
    pub period: usize,
}

impl Strategy for CvdMomentumStrategy {
    fn id(&self) -> &'static str {
        "cvd_momentum"
    }
    fn on_bar(&mut self, ctx: &BarContext) -> Option<Signal> {
        let n = ctx.bars.len();
        if n < self.period {
            return None;
        }
        let window = &ctx.bars[n - self.period..];
        let delta: f64 = window
            .iter()
            .map(|b| (b.close - b.open).signum() * b.volume)
            .sum();
        if delta > 0.0 {
            Some(Signal::target(self.lot))
        } else if delta < 0.0 {
            Some(Signal::target(-self.lot))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtest::engine::{run_backtest, BacktestConfig};
    use crate::model::Bar;

    fn bar(ts: i64, open: f64, close: f64, vol: f64) -> Bar {
        Bar {
            ts,
            open,
            high: open.max(close),
            low: open.min(close),
            close,
            volume: vol,
        }
    }

    #[test]
    fn factory_knows_all_descriptors_and_rejects_unknown() {
        for d in descriptors() {
            assert!(strategy_from_id(d.id, &StrategyParams::new()).is_some());
        }
        assert!(strategy_from_id("nope", &StrategyParams::new()).is_none());
    }

    #[test]
    fn same_lot_enters_fixed_lot_on_breakout() {
        // Рост: после `lookback` баров пробой вверх → целевая позиция = +lot.
        let bars: Vec<Bar> = (0..20)
            .map(|i| bar(i, 100.0 + i as f64, 100.0 + i as f64, 1.0))
            .collect();
        let mut s = SameLotStrategy { lot: 3.0, lookback: 5 };
        let r = run_backtest(&bars, &mut s, BacktestConfig::default());
        assert!(!r.trades.is_empty());
        // Все сделки одного «равного» размера (3 лота на первый вход).
        assert!((r.trades[0].qty - 3.0).abs() < 1e-9);
    }

    #[test]
    fn iceberg_scales_in_equal_clips() {
        // Устойчивый рост выше MA → позиция растёт по одному клипу за бар.
        let bars: Vec<Bar> = (0..30)
            .map(|i| bar(i, 100.0 + i as f64, 100.5 + i as f64, 1.0))
            .collect();
        let mut s = IcebergStrategy { clip: 2.0, clips: 3.0, period: 5 };
        let r = run_backtest(&bars, &mut s, BacktestConfig::default());
        // Каждая сделка — ровно один клип (2 лота); позиция не превышает clip*clips=6.
        assert!(r.trades.iter().all(|t| (t.qty - 2.0).abs() < 1e-9));
        assert!(r.trades.len() >= 3); // как минимум набрали 3 клипа
    }

    #[test]
    fn cvd_momentum_goes_long_on_buying_pressure() {
        // Все бары растут (close>open) → положительная дельта → лонг.
        let bars: Vec<Bar> = (0..20)
            .map(|i| bar(i, 100.0, 101.0, 10.0))
            .collect();
        let mut s = CvdMomentumStrategy { lot: 1.0, period: 5 };
        let r = run_backtest(&bars, &mut s, BacktestConfig::default());
        assert!(r.trades.iter().any(|t| t.side == crate::model::Side::Buy));
    }
}
