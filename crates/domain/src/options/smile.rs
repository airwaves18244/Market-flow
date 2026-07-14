//! Модели улыбки подразумеваемой волатильности: MOEX-параметрическая, SABR
//! (Hagan), SVI (raw, Gatheral) и Каленкович.
//!
//! Формулы и обозначения — как в `docs/options-smile-models.html` (источник
//! правды). Лог-моней­ность `k = ln(K/F)`; для каждой модели IV — функция
//! страйка, форварда и времени. Калибровка — взвешенный МНК по рыночным точкам
//! через встроенный симплекс Нелдера–Мида (без внешних зависимостей).
//!
//! Все вычисления чистые и детерминированные; калибровка стартует из
//! фиксированных начальных точек, поэтому результат воспроизводим в тестах.

use serde::{Deserialize, Serialize};

/// Рыночная точка улыбки для калибровки.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmilePoint {
    /// Страйк.
    pub strike: f64,
    /// Наблюдаемая подразумеваемая волатильность.
    pub iv: f64,
    /// Вес (ликвидность/OI); по умолчанию `1.0`.
    pub weight: f64,
}

impl SmilePoint {
    /// Точка с единичным весом.
    pub fn new(strike: f64, iv: f64) -> Self {
        SmilePoint {
            strike,
            iv,
            weight: 1.0,
        }
    }
}

/// Лог-моней­ность `k = ln(K/F)`.
fn log_moneyness(strike: f64, forward: f64) -> f64 {
    (strike / forward).ln()
}

/// Общий интерфейс модели улыбки.
pub trait SmileModel: Sized {
    /// Подразумеваемая волатильность для страйка при заданных форварде и сроке.
    fn iv(&self, strike: f64, forward: f64, t: f64) -> f64;

    /// Калибровать модель по рыночным точкам (взвешенный МНК по IV).
    fn calibrate(points: &[SmilePoint], forward: f64, t: f64) -> Self;

    /// Взвешенный RMSE подгонки по точкам (метрика качества).
    fn rmse(&self, points: &[SmilePoint], forward: f64, t: f64) -> f64 {
        if points.is_empty() {
            return 0.0;
        }
        let (mut sse, mut wsum) = (0.0, 0.0);
        for p in points {
            let d = self.iv(p.strike, forward, t) - p.iv;
            sse += p.weight * d * d;
            wsum += p.weight;
        }
        if wsum <= 0.0 {
            0.0
        } else {
            (sse / wsum).sqrt()
        }
    }
}

// ---------------------------------------------------------------------------
// MOEX — биржевая параметрическая улыбка
// ---------------------------------------------------------------------------

/// MOEX-параметрическая улыбка: парабола по **стандартизованной** моней­ности с
/// раздельными крыльями и насыщающим (tanh) демпфированием.
///
/// `σ(d) = s0 + skew·d + c(d)·damp(d)²`, где `d = ln(K/F)/(s0·√T)` —
/// стандартизованная моней­ность (страйк в единицах `σ·√T`, «сколько сигм до
/// страйка»), `c = cl` при `d<0` (путовое крыло), `cr` при `d≥0` (колловое),
/// `damp(d) = tanh(d/wing)·wing`.
///
/// Нормировка на `s0·√T` даёт кривой **срочную структуру** («подъём крыльев» с
/// приближением экспирации) и делает `skew`/крылья безразмерными и слабо
/// зависящими от `T` — как в семействе кривых MOEX/НКЦ и модели Каленковича
/// (см. `docs/options-smile-models.html`).
///
/// Верификация формы: конвенции ценообразования (Black-76/Bachelier, `r=0`,
/// нормировка моней­ности по `σ·√T`, срочная структура/подъём крыльев) сверены
/// по методике MOEX/НКЦ; точные коэффициенты биржевой кривой закрыты в деталях
/// методики, поэтому конкретный демпфер крыльев (tanh) — модельный выбор
/// (unverified), сохраняющий документированное качественное поведение
/// (ограниченный, насыщающийся наклон крыльев).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MoexSmile {
    /// ATM-уровень волатильности.
    pub s0: f64,
    /// Наклон (skew) в единицах стандартизованной моней­ности.
    pub skew: f64,
    /// Кривизна путового крыла (`d<0`).
    pub cl: f64,
    /// Кривизна коллового крыла (`d≥0`).
    pub cr: f64,
    /// Масштаб насыщения крыльев (в единицах `σ·√T`).
    pub wing: f64,
}

impl Default for MoexSmile {
    fn default() -> Self {
        MoexSmile {
            s0: 0.30,
            skew: -0.10,
            cl: 0.8,
            cr: 0.5,
            wing: 0.20,
        }
    }
}

impl SmileModel for MoexSmile {
    fn iv(&self, strike: f64, forward: f64, t: f64) -> f64 {
        let k = log_moneyness(strike, forward);
        // Стандартизованная моней­ность в единицах σ·√T («сколько сигм до
        // страйка»): даёт кривой срочную структуру и делает наклон/крылья
        // безразмерными (методика MOEX/НКЦ, Каленкович).
        let tt = if t <= 0.0 { 1e-9 } else { t };
        let d = k / (self.s0 * tt.sqrt());
        let c = if d < 0.0 { self.cl } else { self.cr };
        // Насыщающее (tanh) демпфирование крыльев: на больших |d| кривизна
        // ограничена и IV не «улетает».
        let damp = (d / self.wing).tanh() * self.wing;
        (self.s0 + self.skew * d + c * damp * damp).max(1e-6)
    }

    fn calibrate(points: &[SmilePoint], forward: f64, t: f64) -> Self {
        let start = MoexSmile::default();
        let x0 = vec![start.s0, start.skew, start.cl, start.cr, start.wing];
        let best = nelder_mead(&x0, &[0.05, 0.05, 0.2, 0.2, 0.05], 400, |x| {
            let m = MoexSmile {
                s0: x[0].max(1e-4),
                skew: x[1],
                cl: x[2],
                cr: x[3],
                wing: x[4].abs().max(1e-3),
            };
            weighted_sse(&m, points, forward, t)
        });
        MoexSmile {
            s0: best[0].max(1e-4),
            skew: best[1],
            cl: best[2],
            cr: best[3],
            wing: best[4].abs().max(1e-3),
        }
    }
}

// ---------------------------------------------------------------------------
// SABR (Hagan, лог-нормальное приближение)
// ---------------------------------------------------------------------------

/// Параметры SABR.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SabrParams {
    /// Уровень волатильности α (> 0).
    pub alpha: f64,
    /// Эластичность β ∈ [0, 1] (фиксируется при калибровке).
    pub beta: f64,
    /// Корреляция ρ ∈ (−1, 1).
    pub rho: f64,
    /// Вол-оф-вол ν (≥ 0).
    pub nu: f64,
}

impl Default for SabrParams {
    fn default() -> Self {
        SabrParams {
            alpha: 0.30,
            beta: 1.0,
            rho: -0.3,
            nu: 0.5,
        }
    }
}

impl SabrParams {
    /// IV по приближению Hagan (как в `docs/options-smile-models.html`).
    pub fn hagan_iv(&self, strike: f64, forward: f64, t: f64) -> f64 {
        let f = forward;
        let k = strike;
        let beta = self.beta;
        let log_fk = (f / k).ln();
        let fk_b = (f * k).powf((1.0 - beta) / 2.0);
        let corr = 1.0
            + (((1.0 - beta).powi(2) / 24.0) * self.alpha * self.alpha / (f * k).powf(1.0 - beta)
                + 0.25 * self.rho * beta * self.nu * self.alpha / fk_b
                + ((2.0 - 3.0 * self.rho * self.rho) / 24.0) * self.nu * self.nu)
                * t;
        if log_fk.abs() < 1e-8 {
            // ATM-предел.
            return (self.alpha / f.powf(1.0 - beta) * corr).max(1e-6);
        }
        let z = (self.nu / self.alpha) * fk_b * log_fk;
        let x =
            (((1.0 - 2.0 * self.rho * z + z * z).sqrt() + z - self.rho) / (1.0 - self.rho)).ln();
        let denom = fk_b
            * (1.0
                + ((1.0 - beta).powi(2) / 24.0) * log_fk * log_fk
                + ((1.0 - beta).powi(4) / 1920.0) * log_fk.powi(4));
        (self.alpha / denom * (z / x) * corr).max(1e-6)
    }

    /// Калибровать α, ρ, ν при фиксированном β.
    pub fn calibrate_with_beta(
        points: &[SmilePoint],
        forward: f64,
        t: f64,
        beta: f64,
    ) -> SabrParams {
        let start = SabrParams {
            beta,
            ..Default::default()
        };
        let x0 = vec![start.alpha, start.rho, start.nu];
        let best = nelder_mead(&x0, &[0.05, 0.1, 0.1], 500, |x| {
            let p = SabrParams {
                alpha: x[0].abs().max(1e-4),
                beta,
                rho: x[1].clamp(-0.999, 0.999),
                nu: x[2].abs().max(1e-6),
            };
            weighted_sse(&p, points, forward, t)
        });
        SabrParams {
            alpha: best[0].abs().max(1e-4),
            beta,
            rho: best[1].clamp(-0.999, 0.999),
            nu: best[2].abs().max(1e-6),
        }
    }
}

impl SmileModel for SabrParams {
    fn iv(&self, strike: f64, forward: f64, t: f64) -> f64 {
        self.hagan_iv(strike, forward, t)
    }

    fn calibrate(points: &[SmilePoint], forward: f64, t: f64) -> Self {
        // По умолчанию β = 1 (лог-нормальный режим).
        SabrParams::calibrate_with_beta(points, forward, t, 1.0)
    }
}

// ---------------------------------------------------------------------------
// SVI (Gatheral, raw)
// ---------------------------------------------------------------------------

/// Параметры raw-SVI: полная дисперсия
/// `w(k) = a + b·[ρ(k−m) + √((k−m)² + σ²)]`, `σ_IV = √(w/T)`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SviParams {
    pub a: f64,
    pub b: f64,
    pub rho: f64,
    pub m: f64,
    pub sigma: f64,
}

impl Default for SviParams {
    fn default() -> Self {
        SviParams {
            a: 0.02,
            b: 0.10,
            rho: -0.3,
            m: 0.0,
            sigma: 0.10,
        }
    }
}

impl SviParams {
    /// Полная дисперсия `w(k)`.
    pub fn total_variance(&self, k: f64) -> f64 {
        let km = k - self.m;
        self.a + self.b * (self.rho * km + (km * km + self.sigma * self.sigma).sqrt())
    }

    /// Базовые условия отсутствия арбитража (бабочка): `b ≥ 0`, `|ρ| < 1`,
    /// `σ > 0`, минимум полной дисперсии `a + b·σ·√(1−ρ²) ≥ 0`, и
    /// `b·(1+|ρ|) ≤ 4/T` (ограничение Gatheral на наклон крыльев).
    pub fn is_arbitrage_free(&self, t: f64) -> bool {
        self.b >= 0.0
            && self.rho.abs() < 1.0
            && self.sigma > 0.0
            && self.a + self.b * self.sigma * (1.0 - self.rho * self.rho).sqrt() >= -1e-12
            && (t <= 0.0 || self.b * (1.0 + self.rho.abs()) <= 4.0 / t + 1e-9)
    }
}

impl SmileModel for SviParams {
    fn iv(&self, strike: f64, forward: f64, t: f64) -> f64 {
        let k = log_moneyness(strike, forward);
        let w = self.total_variance(k).max(1e-8);
        let tt = if t <= 0.0 { 1e-9 } else { t };
        (w / tt).sqrt()
    }

    fn calibrate(points: &[SmilePoint], forward: f64, t: f64) -> Self {
        let start = SviParams::default();
        let x0 = vec![start.a, start.b, start.rho, start.m, start.sigma];
        let best = nelder_mead(&x0, &[0.01, 0.05, 0.1, 0.05, 0.05], 600, |x| {
            let p = clamp_svi(x);
            weighted_sse(&p, points, forward, t)
        });
        clamp_svi(&best)
    }
}

/// Привести вектор параметров к допустимому SVI (положительные `b`/`σ`,
/// `|ρ| < 1`).
fn clamp_svi(x: &[f64]) -> SviParams {
    SviParams {
        a: x[0],
        b: x[1].abs().max(1e-6),
        rho: x[2].clamp(-0.999, 0.999),
        m: x[3],
        sigma: x[4].abs().max(1e-4),
    }
}

// ---------------------------------------------------------------------------
// Каленкович (стандартизованная моней­ность)
// ---------------------------------------------------------------------------

/// Улыбка Каленковича: `σ(d) = s0·(1 + skew·d + kurt·d²)`, где
/// `d = k / (s0·√T)` — стандартизованная моней­ность («сколько сигм до страйка»).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct KalenkovichSmile {
    /// ATM-уровень волатильности `s0`.
    pub s0: f64,
    /// Наклон (skew).
    pub skew: f64,
    /// Кривизна (kurtosis).
    pub kurt: f64,
}

impl Default for KalenkovichSmile {
    fn default() -> Self {
        KalenkovichSmile {
            s0: 0.30,
            skew: -0.15,
            kurt: 0.20,
        }
    }
}

impl SmileModel for KalenkovichSmile {
    fn iv(&self, strike: f64, forward: f64, t: f64) -> f64 {
        let k = log_moneyness(strike, forward);
        let tt = if t <= 0.0 { 1e-9 } else { t };
        let d = k / (self.s0 * tt.sqrt());
        (self.s0 * (1.0 + self.skew * d + self.kurt * d * d)).max(1e-6)
    }

    fn calibrate(points: &[SmilePoint], forward: f64, t: f64) -> Self {
        let start = KalenkovichSmile::default();
        let x0 = vec![start.s0, start.skew, start.kurt];
        let best = nelder_mead(&x0, &[0.05, 0.05, 0.05], 400, |x| {
            let m = KalenkovichSmile {
                s0: x[0].max(1e-4),
                skew: x[1],
                kurt: x[2],
            };
            weighted_sse(&m, points, forward, t)
        });
        KalenkovichSmile {
            s0: best[0].max(1e-4),
            skew: best[1],
            kurt: best[2],
        }
    }
}

// ---------------------------------------------------------------------------
// Калибратор (взвешенный МНК) и симплекс Нелдера–Мида
// ---------------------------------------------------------------------------

/// Взвешенная сумма квадратов отклонений модельной IV от рыночной.
fn weighted_sse<M: SmileModel>(model: &M, points: &[SmilePoint], forward: f64, t: f64) -> f64 {
    let mut sse = 0.0;
    for p in points {
        let d = model.iv(p.strike, forward, t) - p.iv;
        sse += p.weight * d * d;
    }
    sse
}

/// Минимизация Нелдера–Мида (downhill simplex) без внешних зависимостей.
///
/// `x0` — старт, `step` — начальные шаги по координатам, `iters` — потолок
/// итераций. Детерминированно (без рандома), поэтому воспроизводимо в тестах.
fn nelder_mead<F: Fn(&[f64]) -> f64>(x0: &[f64], step: &[f64], iters: usize, f: F) -> Vec<f64> {
    let n = x0.len();
    // Построить начальный симплекс из n+1 вершин.
    let mut simplex: Vec<Vec<f64>> = Vec::with_capacity(n + 1);
    simplex.push(x0.to_vec());
    for i in 0..n {
        let mut v = x0.to_vec();
        v[i] += step[i];
        simplex.push(v);
    }
    let mut fvals: Vec<f64> = simplex.iter().map(|v| f(v)).collect();

    // Стандартные коэффициенты.
    let (alpha, gamma, rho_c, sigma) = (1.0, 2.0, 0.5, 0.5);

    for _ in 0..iters {
        // Сортировка вершин по значению функции.
        let mut order: Vec<usize> = (0..=n).collect();
        order.sort_by(|&a, &b| fvals[a].partial_cmp(&fvals[b]).unwrap());
        let best = order[0];
        let worst = order[n];
        let second_worst = order[n - 1];

        // Сходимость по разбросу значений.
        if (fvals[worst] - fvals[best]).abs() < 1e-12 {
            break;
        }

        // Центроид без худшей вершины.
        let mut centroid = vec![0.0; n];
        for (i, &oi) in order.iter().enumerate() {
            if i == n {
                continue;
            }
            for j in 0..n {
                centroid[j] += simplex[oi][j] / n as f64;
            }
        }

        // Отражение.
        let reflected: Vec<f64> = (0..n)
            .map(|j| centroid[j] + alpha * (centroid[j] - simplex[worst][j]))
            .collect();
        let fr = f(&reflected);

        if fr < fvals[best] {
            // Растяжение.
            let expanded: Vec<f64> = (0..n)
                .map(|j| centroid[j] + gamma * (reflected[j] - centroid[j]))
                .collect();
            let fe = f(&expanded);
            if fe < fr {
                simplex[worst] = expanded;
                fvals[worst] = fe;
            } else {
                simplex[worst] = reflected;
                fvals[worst] = fr;
            }
        } else if fr < fvals[second_worst] {
            simplex[worst] = reflected;
            fvals[worst] = fr;
        } else {
            // Сжатие.
            let contracted: Vec<f64> = (0..n)
                .map(|j| centroid[j] + rho_c * (simplex[worst][j] - centroid[j]))
                .collect();
            let fc = f(&contracted);
            if fc < fvals[worst] {
                simplex[worst] = contracted;
                fvals[worst] = fc;
            } else {
                // Глобальное сжатие к лучшей вершине.
                let bestv = simplex[best].clone();
                for i in 0..=n {
                    if i == best {
                        continue;
                    }
                    for j in 0..n {
                        simplex[i][j] = bestv[j] + sigma * (simplex[i][j] - bestv[j]);
                    }
                    fvals[i] = f(&simplex[i]);
                }
            }
        }
    }

    // Лучшая вершина.
    let mut best = 0;
    for i in 1..=n {
        if fvals[i] < fvals[best] {
            best = i;
        }
    }
    simplex[best].clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    /// Сгенерировать точки из «истинной» модели и проверить, что калибровка их
    /// восстанавливает с малым RMSE.
    fn synth_points<M: SmileModel>(truth: &M, forward: f64, t: f64) -> Vec<SmilePoint> {
        let mut pts = Vec::new();
        let mut k: f64 = -0.3;
        while k <= 0.3001 {
            let strike = forward * k.exp();
            pts.push(SmilePoint::new(strike, truth.iv(strike, forward, t)));
            k += 0.05;
        }
        pts
    }

    #[test]
    fn sabr_atm_is_alpha_when_beta_one() {
        let p = SabrParams {
            alpha: 0.25,
            beta: 1.0,
            rho: 0.0,
            nu: 0.0,
        };
        // При β=1, ν=0 ATM-волатильность = α (поправка ×(1+0·T)).
        assert!(approx(p.iv(100.0, 100.0, 0.5), 0.25, 1e-9));
    }

    #[test]
    fn svi_total_variance_minimum_at_m_shift() {
        let p = SviParams {
            a: 0.04,
            b: 0.1,
            rho: 0.0,
            m: 0.0,
            sigma: 0.1,
        };
        // При ρ=0 минимум w(k) в k=m=0 равен a + b·σ.
        let w0 = p.total_variance(0.0);
        assert!(approx(w0, 0.04 + 0.1 * 0.1, 1e-12));
        assert!(p.total_variance(0.2) > w0);
        assert!(p.total_variance(-0.2) > w0);
    }

    #[test]
    fn svi_arbitrage_free_flags() {
        let good = SviParams::default();
        assert!(good.is_arbitrage_free(0.25));
        let bad = SviParams {
            b: -1.0,
            ..SviParams::default()
        };
        assert!(!bad.is_arbitrage_free(0.25));
    }

    #[test]
    fn moex_smile_is_symmetric_when_no_skew_equal_wings() {
        let m = MoexSmile {
            s0: 0.3,
            skew: 0.0,
            cl: 0.5,
            cr: 0.5,
            wing: 0.2,
        };
        let f = 100.0;
        let up = m.iv(f * 0.1_f64.exp(), f, 0.25);
        let dn = m.iv(f * (-0.1_f64).exp(), f, 0.25);
        assert!(approx(up, dn, 1e-12));
        // Крылья выше ATM.
        assert!(up > m.iv(f, f, 0.25));
    }

    #[test]
    fn moex_atm_equals_s0_any_maturity() {
        // При K=F (d=0) улыбка возвращает ровно ATM-уровень s0 на любом сроке.
        let m = MoexSmile::default();
        let f = 100.0;
        for &t in &[0.02, 0.25, 1.0, 3.0] {
            assert!(approx(m.iv(f, f, t), m.s0, 1e-12), "t={t}");
        }
    }

    #[test]
    fn moex_wings_lift_as_expiry_approaches() {
        // Срочная структура: на фиксированном страйке вне денег IV на крыле
        // растёт с приближением экспирации (моней­ность нормирована на σ·√T,
        // поэтому при малом T |d| больше и вклад крыла выше).
        let m = MoexSmile {
            s0: 0.30,
            skew: 0.0,
            cl: 0.8,
            cr: 0.8,
            wing: 0.6,
        };
        let f = 100.0;
        let strike = f * 0.15_f64.exp(); // колловое крыло, k=+0.15
        let long = m.iv(strike, f, 1.0);
        let mid = m.iv(strike, f, 0.25);
        let short = m.iv(strike, f, 0.05);
        assert!(short > mid && mid > long, "{short} {mid} {long}");
    }

    #[test]
    fn moex_skew_makes_puts_richer() {
        // При skew<0 путовое крыло (k<0, d<0) дороже симметричного коллового.
        let m = MoexSmile {
            s0: 0.30,
            skew: -0.05,
            cl: 0.5,
            cr: 0.5,
            wing: 0.5,
        };
        let f = 100.0;
        let put = m.iv(f * (-0.1_f64).exp(), f, 0.25);
        let call = m.iv(f * 0.1_f64.exp(), f, 0.25);
        assert!(put > call, "put {put} vs call {call}");
    }

    #[test]
    fn moex_wings_rise_monotonically_from_atm() {
        // Симметричные крылья без skew: IV монотонно растёт при удалении от ATM.
        let m = MoexSmile {
            s0: 0.30,
            skew: 0.0,
            cl: 0.6,
            cr: 0.6,
            wing: 0.8,
        };
        let (f, t) = (100.0, 0.25);
        let mut prev = m.iv(f, f, t);
        for i in 1..=6 {
            let k = 0.05 * i as f64;
            let up = m.iv(f * k.exp(), f, t);
            let dn = m.iv(f * (-k).exp(), f, t);
            assert!(approx(up, dn, 1e-12), "asymmetry at k={k}");
            assert!(up > prev, "not monotone at k={k}: {up} <= {prev}");
            prev = up;
        }
    }

    #[test]
    fn svi_calibration_recovers_truth() {
        let truth = SviParams {
            a: 0.03,
            b: 0.12,
            rho: -0.4,
            m: 0.02,
            sigma: 0.08,
        };
        let f = 100.0;
        let t = 0.25;
        let pts = synth_points(&truth, f, t);
        let fit = SviParams::calibrate(&pts, f, t);
        assert!(fit.rmse(&pts, f, t) < 1e-3, "rmse {}", fit.rmse(&pts, f, t));
    }

    #[test]
    fn sabr_calibration_recovers_truth() {
        let truth = SabrParams {
            alpha: 0.28,
            beta: 1.0,
            rho: -0.35,
            nu: 0.6,
        };
        let f = 100.0;
        let t = 0.3;
        let pts = synth_points(&truth, f, t);
        let fit = SabrParams::calibrate_with_beta(&pts, f, t, 1.0);
        assert!(fit.rmse(&pts, f, t) < 2e-3, "rmse {}", fit.rmse(&pts, f, t));
    }

    #[test]
    fn moex_calibration_reduces_error() {
        let truth = MoexSmile {
            s0: 0.32,
            skew: -0.12,
            cl: 0.9,
            cr: 0.4,
            wing: 0.18,
        };
        let f = 100.0;
        let t = 0.25;
        let pts = synth_points(&truth, f, t);
        let fit = MoexSmile::calibrate(&pts, f, t);
        assert!(fit.rmse(&pts, f, t) < 5e-3, "rmse {}", fit.rmse(&pts, f, t));
    }

    #[test]
    fn kalenkovich_calibration_reduces_error() {
        let truth = KalenkovichSmile {
            s0: 0.30,
            skew: -0.18,
            kurt: 0.25,
        };
        let f = 100.0;
        let t = 0.25;
        let pts = synth_points(&truth, f, t);
        let fit = KalenkovichSmile::calibrate(&pts, f, t);
        assert!(fit.rmse(&pts, f, t) < 5e-3, "rmse {}", fit.rmse(&pts, f, t));
    }

    #[test]
    fn nelder_mead_minimizes_quadratic() {
        // f(x) = (x0−3)² + (x1+1)² → минимум в (3, −1).
        let best = nelder_mead(&[0.0, 0.0], &[1.0, 1.0], 500, |x| {
            (x[0] - 3.0).powi(2) + (x[1] + 1.0).powi(2)
        });
        assert!(approx(best[0], 3.0, 1e-3));
        assert!(approx(best[1], -1.0, 1e-3));
    }
}
