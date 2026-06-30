//! Конструктор опционных стратегий: ноги, готовые шаблоны, профиль выплаты
//! (payoff на экспирацию), агрегированные греки, точки безубытка и
//! максимальные прибыль/убыток.
//!
//! Профиль выплаты кусочно-линеен по цене базового (изломы в страйках), поэтому
//! безубыток и экстремумы вычисляются точно по вершинам, без численного
//! сканирования сетки. Текущая оценка (mark-to-model) и греки портфеля
//! считаются через [`super::pricing`].

use serde::{Deserialize, Serialize};

use super::pricing::{greeks, price, Greeks, OptionType, PriceInputs, PriceModel};

/// Тип ноги стратегии.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegKind {
    /// Колл-опцион.
    Call,
    /// Пут-опцион.
    Put,
    /// Базовый актив (фьючерс/акция).
    Underlying,
}

/// Сторона позиции.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    /// Покупка (long): платим премию/цену.
    Long,
    /// Продажа (short): получаем премию/цену.
    Short,
}

impl Side {
    /// Знак позиции: +1 long, −1 short.
    pub fn sign(self) -> f64 {
        match self {
            Side::Long => 1.0,
            Side::Short => -1.0,
        }
    }
}

/// Нога стратегии.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Leg {
    /// Тип ноги.
    pub kind: LegKind,
    /// Сторона.
    pub side: Side,
    /// Страйк (для `Underlying` игнорируется).
    pub strike: f64,
    /// Время до экспирации в годах (для текущей оценки/греков).
    pub expiry_t: f64,
    /// Количество контрактов (> 0).
    pub quantity: f64,
    /// Цена входа (премия для опциона, цена для базового).
    pub entry_price: f64,
}

impl Leg {
    /// Внутренняя стоимость ноги на экспирацию при цене базового `spot`.
    fn intrinsic(&self, spot: f64) -> f64 {
        match self.kind {
            LegKind::Call => (spot - self.strike).max(0.0),
            LegKind::Put => (self.strike - spot).max(0.0),
            LegKind::Underlying => spot,
        }
    }

    /// P&L ноги на экспирацию при цене базового `spot`.
    fn payoff(&self, spot: f64) -> f64 {
        self.side.sign() * self.quantity * (self.intrinsic(spot) - self.entry_price)
    }

    /// Чистый дебет ноги при входе (`+` платим, `−` получаем).
    fn net_cost(&self) -> f64 {
        self.side.sign() * self.quantity * self.entry_price
    }

    /// Тип опциона ноги, если это опцион.
    fn option_type(&self) -> Option<OptionType> {
        match self.kind {
            LegKind::Call => Some(OptionType::Call),
            LegKind::Put => Some(OptionType::Put),
            LegKind::Underlying => None,
        }
    }
}

/// Опционная стратегия — набор ног.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Strategy {
    pub legs: Vec<Leg>,
}

/// Сводный результат оценки стратегии на экспирацию.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyResult {
    /// Точки безубытка (отсортированы).
    pub breakevens: Vec<f64>,
    /// Максимальная прибыль (`None` — не ограничена сверху).
    pub max_profit: Option<f64>,
    /// Максимальный убыток (`None` — не ограничен снизу).
    pub max_loss: Option<f64>,
    /// Чистый дебет при входе (`+` заплачено, `−` получено).
    pub net_cost: f64,
}

impl Strategy {
    /// Пустая стратегия.
    pub fn new() -> Self {
        Strategy { legs: Vec::new() }
    }

    /// Добавить ногу (билдер-стиль).
    pub fn with_leg(mut self, leg: Leg) -> Self {
        self.legs.push(leg);
        self
    }

    /// Суммарный P&L на экспирацию при цене базового `spot`.
    pub fn payoff(&self, spot: f64) -> f64 {
        self.legs.iter().map(|l| l.payoff(spot)).sum()
    }

    /// Чистый дебет входа (`+` платим, `−` получаем).
    pub fn net_cost(&self) -> f64 {
        self.legs.iter().map(Leg::net_cost).sum()
    }

    /// Текущая оценка портфеля (mark-to-model) при форварде `forward` и
    /// волатильности `vol`: суммарная рыночная стоимость минус стоимость входа.
    pub fn mark_pnl(&self, forward: f64, vol: f64, rate: f64, model: PriceModel) -> f64 {
        let mut pnl = 0.0;
        for leg in &self.legs {
            let value = match leg.option_type() {
                Some(kind) => price(&PriceInputs {
                    forward,
                    strike: leg.strike,
                    t: leg.expiry_t,
                    vol,
                    rate,
                    kind,
                    model,
                }),
                None => forward,
            };
            pnl += leg.side.sign() * leg.quantity * (value - leg.entry_price);
        }
        pnl
    }

    /// Агрегированные греки портфеля при форварде `forward` и волатильности
    /// `vol`. Базовый актив вносит дельту `±quantity`.
    pub fn greeks(&self, forward: f64, vol: f64, rate: f64, model: PriceModel) -> Greeks {
        let mut acc = Greeks {
            delta: 0.0,
            gamma: 0.0,
            vega: 0.0,
            theta: 0.0,
            rho: 0.0,
        };
        for leg in &self.legs {
            let w = leg.side.sign() * leg.quantity;
            match leg.option_type() {
                Some(kind) => {
                    let g = greeks(&PriceInputs {
                        forward,
                        strike: leg.strike,
                        t: leg.expiry_t,
                        vol,
                        rate,
                        kind,
                        model,
                    });
                    acc.delta += w * g.delta;
                    acc.gamma += w * g.gamma;
                    acc.vega += w * g.vega;
                    acc.theta += w * g.theta;
                    acc.rho += w * g.rho;
                }
                None => acc.delta += w, // базовый: дельта = 1 на контракт
            }
        }
        acc
    }

    /// Вершины кусочно-линейного профиля: `0` и все страйки опционных ног.
    fn vertices(&self) -> Vec<f64> {
        let mut v = vec![0.0];
        for leg in &self.legs {
            if leg.option_type().is_some() {
                v.push(leg.strike);
            }
        }
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        v
    }

    /// Точки безубытка (нули профиля выплаты) на экспирацию.
    ///
    /// Профиль кусочно-линеен с изломами в страйках, поэтому достаточно
    /// просканировать соседние вершины и линейно интерполировать нули.
    pub fn breakevens(&self) -> Vec<f64> {
        let mut verts = self.vertices();
        // Добавить точку справа от старшего страйка, чтобы поймать нули на
        // правом крыле.
        let max_v = *verts.last().unwrap_or(&0.0);
        verts.push(max_v + max_v.max(1.0));
        let mut out = Vec::new();
        for w in verts.windows(2) {
            let (x0, x1) = (w[0], w[1]);
            let (y0, y1) = (self.payoff(x0), self.payoff(x1));
            if y0.abs() < 1e-9 {
                push_unique(&mut out, x0);
            }
            if (y0 < 0.0 && y1 > 0.0) || (y0 > 0.0 && y1 < 0.0) {
                let x = x0 - y0 * (x1 - x0) / (y1 - y0);
                push_unique(&mut out, x);
            }
        }
        // Проверить последнюю вершину отдельно.
        let last = *verts.last().unwrap();
        if self.payoff(last).abs() < 1e-9 {
            push_unique(&mut out, last);
        }
        out.sort_by(|a, b| a.partial_cmp(b).unwrap());
        out
    }

    /// Полная оценка: безубыток, максимумы прибыли/убытка, чистый дебет.
    pub fn evaluate(&self) -> StrategyResult {
        let verts = self.vertices();
        // Значения в вершинах (включая S=0 — левую границу области).
        let mut vals: Vec<f64> = verts.iter().map(|&s| self.payoff(s)).collect();
        let max_v = *verts.last().unwrap_or(&0.0);

        // Наклон правого крыла (S → ∞).
        let probe = max_v + max_v.max(1.0);
        let right_slope = self.payoff(probe) - self.payoff(max_v);
        // Значение в дальней правой точке для оценки экстремума на крыле.
        vals.push(self.payoff(probe));

        let finite_max = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let finite_min = vals.iter().cloned().fold(f64::INFINITY, f64::min);

        let (max_profit, max_loss) = if right_slope > 1e-9 {
            (None, Some(finite_min)) // прибыль не ограничена сверху
        } else if right_slope < -1e-9 {
            (Some(finite_max), None) // убыток не ограничен снизу
        } else {
            (Some(finite_max), Some(finite_min))
        };

        StrategyResult {
            breakevens: self.breakevens(),
            max_profit,
            max_loss,
            net_cost: self.net_cost(),
        }
    }

    /// Профиль риска: P&L на экспирацию по равномерной сетке цен
    /// `[lo, hi]` из `steps` точек — для тепловой карты/графика.
    pub fn risk_profile(&self, lo: f64, hi: f64, steps: usize) -> Vec<(f64, f64)> {
        if steps == 0 {
            return Vec::new();
        }
        let n = steps.max(2);
        (0..n)
            .map(|i| {
                let s = lo + (hi - lo) * i as f64 / (n - 1) as f64;
                (s, self.payoff(s))
            })
            .collect()
    }
}

fn push_unique(out: &mut Vec<f64>, x: f64) {
    if !out.iter().any(|&e| (e - x).abs() < 1e-6) {
        out.push(x);
    }
}

/// Готовые шаблоны стратегий.
pub mod templates {
    use super::*;

    /// Вертикальный колл-спред: длинный колл `k_long`, короткий колл `k_short`.
    pub fn vertical_call_spread(
        k_long: f64,
        prem_long: f64,
        k_short: f64,
        prem_short: f64,
        t: f64,
        qty: f64,
    ) -> Strategy {
        Strategy::new()
            .with_leg(Leg {
                kind: LegKind::Call,
                side: Side::Long,
                strike: k_long,
                expiry_t: t,
                quantity: qty,
                entry_price: prem_long,
            })
            .with_leg(Leg {
                kind: LegKind::Call,
                side: Side::Short,
                strike: k_short,
                expiry_t: t,
                quantity: qty,
                entry_price: prem_short,
            })
    }

    /// Стрэддл: длинный колл и длинный пут на одном страйке.
    pub fn straddle(strike: f64, call_prem: f64, put_prem: f64, t: f64, qty: f64) -> Strategy {
        Strategy::new()
            .with_leg(Leg {
                kind: LegKind::Call,
                side: Side::Long,
                strike,
                expiry_t: t,
                quantity: qty,
                entry_price: call_prem,
            })
            .with_leg(Leg {
                kind: LegKind::Put,
                side: Side::Long,
                strike,
                expiry_t: t,
                quantity: qty,
                entry_price: put_prem,
            })
    }

    /// Стрэнгл: длинный OTM-колл `k_call` и длинный OTM-пут `k_put`.
    pub fn strangle(
        k_put: f64,
        put_prem: f64,
        k_call: f64,
        call_prem: f64,
        t: f64,
        qty: f64,
    ) -> Strategy {
        Strategy::new()
            .with_leg(Leg {
                kind: LegKind::Put,
                side: Side::Long,
                strike: k_put,
                expiry_t: t,
                quantity: qty,
                entry_price: put_prem,
            })
            .with_leg(Leg {
                kind: LegKind::Call,
                side: Side::Long,
                strike: k_call,
                expiry_t: t,
                quantity: qty,
                entry_price: call_prem,
            })
    }

    /// Колл-баттерфляй: long `k_low`, 2× short `k_mid`, long `k_high`.
    #[allow(clippy::too_many_arguments)]
    pub fn call_butterfly(
        k_low: f64,
        p_low: f64,
        k_mid: f64,
        p_mid: f64,
        k_high: f64,
        p_high: f64,
        t: f64,
        qty: f64,
    ) -> Strategy {
        Strategy::new()
            .with_leg(Leg {
                kind: LegKind::Call,
                side: Side::Long,
                strike: k_low,
                expiry_t: t,
                quantity: qty,
                entry_price: p_low,
            })
            .with_leg(Leg {
                kind: LegKind::Call,
                side: Side::Short,
                strike: k_mid,
                expiry_t: t,
                quantity: 2.0 * qty,
                entry_price: p_mid,
            })
            .with_leg(Leg {
                kind: LegKind::Call,
                side: Side::Long,
                strike: k_high,
                expiry_t: t,
                quantity: qty,
                entry_price: p_high,
            })
    }

    /// Covered call: длинный базовый + короткий колл.
    pub fn covered_call(
        underlying_price: f64,
        strike: f64,
        call_prem: f64,
        t: f64,
        qty: f64,
    ) -> Strategy {
        Strategy::new()
            .with_leg(Leg {
                kind: LegKind::Underlying,
                side: Side::Long,
                strike: 0.0,
                expiry_t: t,
                quantity: qty,
                entry_price: underlying_price,
            })
            .with_leg(Leg {
                kind: LegKind::Call,
                side: Side::Short,
                strike,
                expiry_t: t,
                quantity: qty,
                entry_price: call_prem,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::templates::*;
    use super::*;

    fn approx(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn long_call_payoff_and_breakeven() {
        // Длинный колл K=100, премия 5. Безубыток = 105, убыток ограничен −5,
        // прибыль не ограничена.
        let s = Strategy::new().with_leg(Leg {
            kind: LegKind::Call,
            side: Side::Long,
            strike: 100.0,
            expiry_t: 0.25,
            quantity: 1.0,
            entry_price: 5.0,
        });
        assert!(approx(s.payoff(100.0), -5.0, 1e-9));
        assert!(approx(s.payoff(110.0), 5.0, 1e-9));
        let r = s.evaluate();
        assert_eq!(r.breakevens.len(), 1);
        assert!(approx(r.breakevens[0], 105.0, 1e-6));
        assert!(approx(r.max_loss.unwrap(), -5.0, 1e-9));
        assert!(r.max_profit.is_none());
        assert!(approx(r.net_cost, 5.0, 1e-9));
    }

    #[test]
    fn vertical_spread_caps_profit_and_loss() {
        // Бычий колл-спред: long 100 @4, short 110 @1.5. Дебет 2.5.
        // Max loss = −2.5, max profit = (110−100) − 2.5 = 7.5, безубыток 102.5.
        let s = vertical_call_spread(100.0, 4.0, 110.0, 1.5, 0.25, 1.0);
        let r = s.evaluate();
        assert!(approx(r.net_cost, 2.5, 1e-9));
        assert!(approx(r.max_loss.unwrap(), -2.5, 1e-6));
        assert!(approx(r.max_profit.unwrap(), 7.5, 1e-6));
        assert_eq!(r.breakevens.len(), 1);
        assert!(approx(r.breakevens[0], 102.5, 1e-6));
    }

    #[test]
    fn straddle_has_two_breakevens() {
        // Long straddle K=100, премии 5+5=10. Безубытки 90 и 110.
        let s = straddle(100.0, 5.0, 5.0, 0.25, 1.0);
        let r = s.evaluate();
        assert_eq!(r.breakevens.len(), 2);
        assert!(approx(r.breakevens[0], 90.0, 1e-6));
        assert!(approx(r.breakevens[1], 110.0, 1e-6));
        // Максимальный убыток в ATM = −10.
        assert!(approx(r.max_loss.unwrap(), -10.0, 1e-6));
        assert!(r.max_profit.is_none());
    }

    #[test]
    fn butterfly_is_bounded_both_sides() {
        // Симметричный колл-баттерфляй 95/100/105, дебет = p_low+p_high−2·p_mid.
        let s = call_butterfly(95.0, 7.0, 100.0, 3.5, 105.0, 1.5, 0.25, 1.0);
        let r = s.evaluate();
        // Оба крыла ограничены.
        assert!(r.max_profit.is_some());
        assert!(r.max_loss.is_some());
        let debit = 7.0 + 1.5 - 2.0 * 3.5; // = 1.5
        assert!(approx(r.net_cost, debit, 1e-9));
        // Пик прибыли в середине: (100−95) − debit.
        assert!(approx(s.payoff(100.0), 5.0 - debit, 1e-9));
    }

    #[test]
    fn strangle_two_breakevens_outside_strikes() {
        let s = strangle(95.0, 2.0, 105.0, 2.0, 0.25, 1.0);
        let r = s.evaluate();
        assert_eq!(r.breakevens.len(), 2);
        // Безубытки: 95−4=91 и 105+4=109.
        assert!(approx(r.breakevens[0], 91.0, 1e-6));
        assert!(approx(r.breakevens[1], 109.0, 1e-6));
    }

    #[test]
    fn covered_call_caps_upside() {
        // Базовый куплен по 100, продан колл 110 @3. Сверху ограничено.
        let s = covered_call(100.0, 110.0, 3.0, 0.25, 1.0);
        let r = s.evaluate();
        // При S=110: (110−100) + 3 = 13.
        assert!(approx(s.payoff(110.0), 13.0, 1e-9));
        assert!(approx(s.payoff(120.0), 13.0, 1e-9)); // полка
        assert!(r.max_profit.is_some());
        // Безубыток = 100 − 3 = 97.
        assert!(approx(r.breakevens[0], 97.0, 1e-6));
    }

    #[test]
    fn aggregate_delta_sums_legs() {
        // Дельта длинного ATM-колла + короткого ATM-пута ≈ дельта форварда (1).
        let s = Strategy::new()
            .with_leg(Leg {
                kind: LegKind::Call,
                side: Side::Long,
                strike: 100.0,
                expiry_t: 0.5,
                quantity: 1.0,
                entry_price: 6.0,
            })
            .with_leg(Leg {
                kind: LegKind::Put,
                side: Side::Short,
                strike: 100.0,
                expiry_t: 0.5,
                quantity: 1.0,
                entry_price: 6.0,
            });
        let g = s.greeks(100.0, 0.3, 0.0, PriceModel::Black76);
        // Синтетический форвард: дельта ≈ 1.
        assert!(approx(g.delta, 1.0, 1e-6), "delta {}", g.delta);
    }

    #[test]
    fn mark_pnl_zero_at_entry_prices() {
        // Если цены входа равны модельным, P&L mark-to-model = 0.
        let inputs = PriceInputs::black(100.0, 100.0, 0.5, 0.3, OptionType::Call);
        let prem = price(&inputs);
        let s = Strategy::new().with_leg(Leg {
            kind: LegKind::Call,
            side: Side::Long,
            strike: 100.0,
            expiry_t: 0.5,
            quantity: 1.0,
            entry_price: prem,
        });
        let pnl = s.mark_pnl(100.0, 0.3, 0.0, PriceModel::Black76);
        assert!(approx(pnl, 0.0, 1e-9));
    }
}
