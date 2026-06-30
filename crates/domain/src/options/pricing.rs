//! Ценообразование опционов: Блэк-76 и Башелье (нормальная модель), греки и
//! решатель подразумеваемой волатильности (IV).
//!
//! Конвенция — **форвардная**: цена строится от форварда `F` базового актива,
//! а дисконт учитывается множителем `e^{-rT}`. Для маржируемых опционов MOEX
//! ставка `r = 0` (расчёт по вариационной марже), поэтому дисконт-фактор
//! вырождается в единицу — это значение по умолчанию.
//!
//! Все функции чистые и детерминированные. Греки реализованы аналитически и в
//! тестах сверяются с численным дифференцированием цены.

use serde::{Deserialize, Serialize};

/// Тип опциона.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionType {
    /// Колл (право купить).
    Call,
    /// Пут (право продать).
    Put,
}

impl OptionType {
    /// Знак выплаты: +1 для колла, −1 для пута. Удобно в общих формулах.
    fn sign(self) -> f64 {
        match self {
            OptionType::Call => 1.0,
            OptionType::Put => -1.0,
        }
    }
}

/// Модель ценообразования.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceModel {
    /// Блэк-76 (лог-нормальная) — для опционов с положительной ценой базового.
    Black76,
    /// Башелье (нормальная) — для низких/отрицательных цен (товарные спреды,
    /// ставки), где лог-нормальность неуместна.
    Bachelier,
}

/// Входные параметры ценообразования (форвардная конвенция).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PriceInputs {
    /// Форвард базового актива.
    pub forward: f64,
    /// Страйк.
    pub strike: f64,
    /// Время до экспирации в годах.
    pub t: f64,
    /// Волатильность: для Блэка — относительная (σ, доля), для Башелье —
    /// абсолютная (в единицах цены за √год).
    pub vol: f64,
    /// Безрисковая ставка для дисконта. По умолчанию `0.0` (MOEX-маржируемые).
    pub rate: f64,
    /// Тип опциона.
    pub kind: OptionType,
    /// Модель ценообразования.
    pub model: PriceModel,
}

impl PriceInputs {
    /// Конструктор с `rate = 0` (MOEX) и моделью Блэк-76 — самый частый случай.
    pub fn black(forward: f64, strike: f64, t: f64, vol: f64, kind: OptionType) -> Self {
        PriceInputs {
            forward,
            strike,
            t,
            vol,
            rate: 0.0,
            kind,
            model: PriceModel::Black76,
        }
    }

    /// Дисконт-фактор `e^{-rT}`.
    fn discount(&self) -> f64 {
        (-self.rate * self.t).exp()
    }
}

/// Греки опциона (чувствительности цены).
///
/// Единицы согласованы и «сырые» (на единичное изменение аргумента), без
/// масштабирования на 1 %/1 день — масштабирование оставлено представлению.
/// - `delta` — ∂V/∂F;
/// - `gamma` — ∂²V/∂F²;
/// - `vega`  — ∂V/∂σ (на единичную волатильность, т.е. на 1.00, не 1 %);
/// - `theta` — ∂V/∂t = −∂V/∂T (календарный распад на единицу времени, в годах);
/// - `rho`   — ∂V/∂r.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Greeks {
    pub delta: f64,
    pub gamma: f64,
    pub vega: f64,
    pub theta: f64,
    pub rho: f64,
}

/// Плотность стандартного нормального распределения φ(x).
fn norm_pdf(x: f64) -> f64 {
    const INV_SQRT_2PI: f64 = 0.398_942_280_401_432_7;
    INV_SQRT_2PI * (-0.5 * x * x).exp()
}

/// Функция распределения стандартного нормального Φ(x) через erf.
fn norm_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x * std::f64::consts::FRAC_1_SQRT_2))
}

/// Функция ошибок erf(x). Аппроксимация Abramowitz & Stegun 7.1.26
/// (абсолютная погрешность < 1.5e-7) — достаточно для аналитики и сверки
/// греков с конечными разностями.
fn erf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let y = 1.0
        - (((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736) * t
            + 0.254_829_592)
            * t
            * (-x * x).exp();
    sign * y
}

/// Цена опциона по выбранной модели.
///
/// Вырожденные случаи (нулевые `t`/`vol`) сводятся к внутренней стоимости от
/// форварда, дисконтированной множителем `e^{-rT}`.
pub fn price(inputs: &PriceInputs) -> f64 {
    let df = inputs.discount();
    let intrinsic = (inputs.kind.sign() * (inputs.forward - inputs.strike)).max(0.0);
    if inputs.t <= 0.0 || inputs.vol <= 0.0 {
        return df * intrinsic;
    }
    match inputs.model {
        PriceModel::Black76 => black76_price(inputs, df),
        PriceModel::Bachelier => bachelier_price(inputs, df),
    }
}

/// d1/d2 для модели Блэк-76. Предполагает `t > 0` и `vol > 0`.
fn black76_d1d2(inputs: &PriceInputs) -> (f64, f64) {
    let sqrt_t = inputs.t.sqrt();
    let v = inputs.vol * sqrt_t;
    let d1 = ((inputs.forward / inputs.strike).ln() + 0.5 * inputs.vol * inputs.vol * inputs.t) / v;
    (d1, d1 - v)
}

fn black76_price(inputs: &PriceInputs, df: f64) -> f64 {
    let (d1, d2) = black76_d1d2(inputs);
    match inputs.kind {
        OptionType::Call => df * (inputs.forward * norm_cdf(d1) - inputs.strike * norm_cdf(d2)),
        OptionType::Put => df * (inputs.strike * norm_cdf(-d2) - inputs.forward * norm_cdf(-d1)),
    }
}

/// d для нормальной модели Башелье. Предполагает `t > 0` и `vol > 0`.
fn bachelier_d(inputs: &PriceInputs) -> f64 {
    (inputs.forward - inputs.strike) / (inputs.vol * inputs.t.sqrt())
}

fn bachelier_price(inputs: &PriceInputs, df: f64) -> f64 {
    let sqrt_t = inputs.t.sqrt();
    let d = bachelier_d(inputs);
    let diff = inputs.forward - inputs.strike;
    let body = match inputs.kind {
        OptionType::Call => diff * norm_cdf(d) + inputs.vol * sqrt_t * norm_pdf(d),
        OptionType::Put => -diff * norm_cdf(-d) + inputs.vol * sqrt_t * norm_pdf(d),
    };
    df * body
}

/// Аналитические греки по выбранной модели.
pub fn greeks(inputs: &PriceInputs) -> Greeks {
    if inputs.t <= 0.0 || inputs.vol <= 0.0 {
        // На границе экспирации/нулевой волатильности чувствительности
        // вырождаются; rho остаётся как −T·V (дисконт).
        let v = price(inputs);
        return Greeks {
            delta: 0.0,
            gamma: 0.0,
            vega: 0.0,
            theta: 0.0,
            rho: -inputs.t * v,
        };
    }
    match inputs.model {
        PriceModel::Black76 => black76_greeks(inputs),
        PriceModel::Bachelier => bachelier_greeks(inputs),
    }
}

fn black76_greeks(inputs: &PriceInputs) -> Greeks {
    let df = inputs.discount();
    let sqrt_t = inputs.t.sqrt();
    let (d1, _d2) = black76_d1d2(inputs);
    let pdf = norm_pdf(d1);
    let delta = match inputs.kind {
        OptionType::Call => df * norm_cdf(d1),
        OptionType::Put => -df * norm_cdf(-d1),
    };
    let gamma = df * pdf / (inputs.forward * inputs.vol * sqrt_t);
    let vega = df * inputs.forward * pdf * sqrt_t;
    // theta = ∂V/∂t = −∂V/∂T. Для Блэк-76 (форвард): распад за счёт диффузии
    // плюс изменение дисконта.
    let v = black76_price(inputs, df);
    let decay = -df * inputs.forward * pdf * inputs.vol / (2.0 * sqrt_t);
    let theta = decay + inputs.rate * v;
    // Под форвардной конвенцией единственная зависимость от r — дисконт-фактор.
    let rho = -inputs.t * v;
    Greeks {
        delta,
        gamma,
        vega,
        theta,
        rho,
    }
}

fn bachelier_greeks(inputs: &PriceInputs) -> Greeks {
    let df = inputs.discount();
    let sqrt_t = inputs.t.sqrt();
    let d = bachelier_d(inputs);
    let pdf = norm_pdf(d);
    let delta = match inputs.kind {
        OptionType::Call => df * norm_cdf(d),
        OptionType::Put => -df * norm_cdf(-d),
    };
    let gamma = df * pdf / (inputs.vol * sqrt_t);
    let vega = df * sqrt_t * pdf;
    let v = bachelier_price(inputs, df);
    let decay = -df * inputs.vol * pdf / (2.0 * sqrt_t);
    let theta = decay + inputs.rate * v;
    let rho = -inputs.t * v;
    Greeks {
        delta,
        gamma,
        vega,
        theta,
        rho,
    }
}

/// Подразумеваемая волатильность из рыночной цены.
///
/// Ньютон по vega с фолбэком на бисекцию; устойчив на крыльях и при вырождениях
/// (нулевая vega, цена ≤ внутренней стоимости). Возвращает `None`, если цена вне
/// допустимого диапазона (ниже внутренней стоимости или выше предела модели).
pub fn implied_vol(
    market_price: f64,
    forward: f64,
    strike: f64,
    t: f64,
    rate: f64,
    kind: OptionType,
    model: PriceModel,
) -> Option<f64> {
    if t <= 0.0 || market_price < 0.0 {
        return None;
    }
    let df = (-rate * t).exp();
    let intrinsic = df * (kind.sign() * (forward - strike)).max(0.0);
    // Цена ниже внутренней стоимости недостижима положительной волатильностью.
    if market_price < intrinsic - 1e-12 {
        return None;
    }
    let mut inputs = PriceInputs {
        forward,
        strike,
        t,
        vol: 0.0,
        rate,
        kind,
        model,
    };
    let price_at = |vol: f64, inputs: &mut PriceInputs| {
        inputs.vol = vol;
        price(inputs)
    };

    // Скобка для бисекции: расширяем верхнюю границу, пока цена не превысит цель.
    let mut lo = 1e-9;
    let mut hi = 1.0;
    let mut hi_price = price_at(hi, &mut inputs);
    let mut guard = 0;
    while hi_price < market_price {
        hi *= 2.0;
        hi_price = price_at(hi, &mut inputs);
        guard += 1;
        if guard > 60 {
            // Цена недостижима даже при экстремальной волатильности.
            return None;
        }
    }

    // Старт Ньютона из середины скобки.
    let mut vol = 0.2_f64.clamp(lo, hi);
    for _ in 0..100 {
        inputs.vol = vol;
        let diff = price(&inputs) - market_price;
        if diff.abs() < 1e-10 {
            return Some(vol);
        }
        // Поддерживаем скобку для фолбэка.
        if diff > 0.0 {
            hi = vol;
        } else {
            lo = vol;
        }
        let vega = greeks(&inputs).vega;
        if vega.abs() < 1e-12 {
            break; // переключаемся на бисекцию
        }
        let step = diff / vega;
        let next = vol - step;
        if !next.is_finite() || next <= lo || next >= hi {
            break; // выход за скобку — бисекция
        }
        vol = next;
    }

    // Фолбэк: бисекция в гарантированной скобке [lo, hi].
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        inputs.vol = mid;
        let diff = price(&inputs) - market_price;
        if diff.abs() < 1e-10 || (hi - lo) < 1e-12 {
            return Some(mid);
        }
        if diff > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some(0.5 * (lo + hi))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn norm_cdf_known_points() {
        assert!(approx(norm_cdf(0.0), 0.5, 1e-9));
        assert!(approx(norm_cdf(1.0), 0.841_344_75, 1e-6));
        assert!(approx(norm_cdf(-1.0), 0.158_655_25, 1e-6));
        // Симметрия Φ(x)+Φ(−x)=1.
        for &x in &[-2.3, -0.7, 0.4, 1.9] {
            assert!(approx(norm_cdf(x) + norm_cdf(-x), 1.0, 1e-9));
        }
    }

    #[test]
    fn black76_atm_matches_closed_form() {
        // ATM (F=K): цена колла ≈ F·σ·√T/√(2π) при r=0 (малый порядок T).
        let f = 100.0;
        let t = 0.25;
        let vol = 0.30;
        let inputs = PriceInputs::black(f, f, t, vol, OptionType::Call);
        let c = price(&inputs);
        let approx_atm = f * vol * t.sqrt() / (2.0 * std::f64::consts::PI).sqrt();
        // Приближение первого порядка — допускаем 1.5 %.
        assert!(
            approx(c, approx_atm, approx_atm * 0.015),
            "{c} vs {approx_atm}"
        );
    }

    #[test]
    fn put_call_parity_black76() {
        // C − P = e^{-rT}(F − K).
        let (f, k, t, vol, r) = (105.0, 100.0, 0.5, 0.25, 0.04);
        let call = price(&PriceInputs {
            forward: f,
            strike: k,
            t,
            vol,
            rate: r,
            kind: OptionType::Call,
            model: PriceModel::Black76,
        });
        let put = price(&PriceInputs {
            forward: f,
            strike: k,
            t,
            vol,
            rate: r,
            kind: OptionType::Put,
            model: PriceModel::Black76,
        });
        let df = (-r * t).exp();
        assert!(approx(call - put, df * (f - k), 1e-8));
    }

    #[test]
    fn put_call_parity_bachelier() {
        let (f, k, t, vol, r) = (100.0, 102.0, 0.75, 12.0, 0.03);
        let call = price(&PriceInputs {
            forward: f,
            strike: k,
            t,
            vol,
            rate: r,
            kind: OptionType::Call,
            model: PriceModel::Bachelier,
        });
        let put = price(&PriceInputs {
            forward: f,
            strike: k,
            t,
            vol,
            rate: r,
            kind: OptionType::Put,
            model: PriceModel::Bachelier,
        });
        let df = (-r * t).exp();
        assert!(approx(call - put, df * (f - k), 1e-7));
    }

    #[test]
    fn greeks_match_finite_differences_black76() {
        let base = PriceInputs {
            forward: 100.0,
            strike: 95.0,
            t: 0.5,
            vol: 0.28,
            rate: 0.03,
            kind: OptionType::Call,
            model: PriceModel::Black76,
        };
        let g = greeks(&base);

        // delta
        let h = 1e-3;
        let mut up = base;
        let mut dn = base;
        up.forward += h;
        dn.forward -= h;
        let fd_delta = (price(&up) - price(&dn)) / (2.0 * h);
        assert!(
            approx(g.delta, fd_delta, 1e-4),
            "delta {} vs {}",
            g.delta,
            fd_delta
        );

        // gamma
        let fd_gamma = (price(&up) - 2.0 * price(&base) + price(&dn)) / (h * h);
        assert!(
            approx(g.gamma, fd_gamma, 1e-3),
            "gamma {} vs {}",
            g.gamma,
            fd_gamma
        );

        // vega
        let hv = 1e-4;
        let mut vu = base;
        let mut vd = base;
        vu.vol += hv;
        vd.vol -= hv;
        let fd_vega = (price(&vu) - price(&vd)) / (2.0 * hv);
        assert!(
            approx(g.vega, fd_vega, 1e-2),
            "vega {} vs {}",
            g.vega,
            fd_vega
        );

        // rho
        let hr = 1e-5;
        let mut ru = base;
        let mut rd = base;
        ru.rate += hr;
        rd.rate -= hr;
        let fd_rho = (price(&ru) - price(&rd)) / (2.0 * hr);
        assert!(approx(g.rho, fd_rho, 1e-3), "rho {} vs {}", g.rho, fd_rho);

        // theta = -dV/dT
        let ht = 1e-5;
        let mut tu = base;
        let mut td = base;
        tu.t += ht;
        td.t -= ht;
        let fd_theta = -(price(&tu) - price(&td)) / (2.0 * ht);
        assert!(
            approx(g.theta, fd_theta, 1e-2),
            "theta {} vs {}",
            g.theta,
            fd_theta
        );
    }

    #[test]
    fn greeks_match_finite_differences_bachelier() {
        let base = PriceInputs {
            forward: 100.0,
            strike: 105.0,
            t: 0.4,
            vol: 15.0,
            rate: 0.02,
            kind: OptionType::Put,
            model: PriceModel::Bachelier,
        };
        let g = greeks(&base);
        let h = 1e-3;
        let mut up = base;
        let mut dn = base;
        up.forward += h;
        dn.forward -= h;
        let fd_delta = (price(&up) - price(&dn)) / (2.0 * h);
        assert!(approx(g.delta, fd_delta, 1e-4));
        let fd_gamma = (price(&up) - 2.0 * price(&base) + price(&dn)) / (h * h);
        assert!(approx(g.gamma, fd_gamma, 1e-3));
        let hv = 1e-3;
        let mut vu = base;
        let mut vd = base;
        vu.vol += hv;
        vd.vol -= hv;
        let fd_vega = (price(&vu) - price(&vd)) / (2.0 * hv);
        assert!(approx(g.vega, fd_vega, 1e-3));
    }

    #[test]
    fn implied_vol_roundtrips() {
        for &kind in &[OptionType::Call, OptionType::Put] {
            for &(f, k) in &[(100.0, 100.0), (100.0, 90.0), (100.0, 115.0)] {
                let true_vol = 0.27;
                let inputs = PriceInputs {
                    forward: f,
                    strike: k,
                    t: 0.5,
                    vol: true_vol,
                    rate: 0.03,
                    kind,
                    model: PriceModel::Black76,
                };
                let mkt = price(&inputs);
                let iv = implied_vol(mkt, f, k, 0.5, 0.03, kind, PriceModel::Black76)
                    .expect("iv exists");
                assert!(approx(iv, true_vol, 1e-6), "iv {iv} vs {true_vol}");
            }
        }
    }

    #[test]
    fn implied_vol_bachelier_roundtrips() {
        let inputs = PriceInputs {
            forward: 100.0,
            strike: 98.0,
            t: 0.3,
            vol: 18.0,
            rate: 0.0,
            kind: OptionType::Call,
            model: PriceModel::Bachelier,
        };
        let mkt = price(&inputs);
        let iv = implied_vol(
            mkt,
            100.0,
            98.0,
            0.3,
            0.0,
            OptionType::Call,
            PriceModel::Bachelier,
        )
        .expect("iv");
        assert!(approx(iv, 18.0, 1e-5));
    }

    #[test]
    fn implied_vol_rejects_below_intrinsic() {
        // Цена ниже внутренней стоимости недостижима.
        let iv = implied_vol(
            0.5,
            110.0,
            100.0,
            0.5,
            0.0,
            OptionType::Call,
            PriceModel::Black76,
        );
        assert!(iv.is_none());
    }
}
