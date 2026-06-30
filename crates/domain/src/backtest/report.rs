//! Результат бэктеста: сделки, кривая капитала и метрики эффективности.
//!
//! Чистые типы и расчёт метрик (P&L, win-rate, profit factor, просадка,
//! Sharpe) — без I/O, тестируются на синтетических данных.

use crate::model::Side;

/// Одна смоделированная сделка (исполнение заявки бэктестера).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimTrade {
    /// Время исполнения, UNIX-секунды UTC (время бара исполнения).
    pub ts: i64,
    pub side: Side,
    /// Исполненный объём (в единицах/лотах), всегда положительный.
    pub qty: f64,
    /// Цена исполнения (с учётом проскальзывания).
    pub price: f64,
    /// Реализованный P&L, зафиксированный этой сделкой (0 для входов/наращивания,
    /// ненулевой — когда сделка закрывает/сокращает позицию).
    pub realized_pnl: f64,
}

/// Сводные метрики эффективности стратегии.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerfMetrics {
    /// Чистый P&L: итоговый капитал − стартовый.
    pub net_pnl: f64,
    /// Доходность в долях от стартового капитала.
    pub return_pct: f64,
    /// Число смоделированных сделок.
    pub trades: usize,
    /// Закрывающие сделки с положительным/отрицательным реализованным P&L.
    pub wins: usize,
    pub losses: usize,
    /// Доля прибыльных среди закрывающих сделок (0..1), либо `0` если их нет.
    pub win_rate: f64,
    /// Profit factor: Σ прибыли / Σ |убытков|. `f64::INFINITY`, если убытков нет
    /// при наличии прибыли; `0`, если нет прибыли.
    pub profit_factor: f64,
    /// Максимальная просадка кривой капитала в долях от пика (0..1).
    pub max_drawdown: f64,
    /// Коэффициент Шарпа по пошаговым доходностям капитала (без risk-free),
    /// аннуализация не применяется — сырое `mean/std`.
    pub sharpe: f64,
    /// Средняя прибыль выигрышной / средний убыток проигрышной сделки.
    pub avg_win: f64,
    pub avg_loss: f64,
}

/// Полный отчёт прогона бэктеста.
#[derive(Debug, Clone, PartialEq)]
pub struct BacktestReport {
    pub trades: Vec<SimTrade>,
    /// Кривая капитала: `(ts, equity)` по каждому бару (mark-to-market).
    pub equity_curve: Vec<(i64, f64)>,
    pub metrics: PerfMetrics,
}

impl BacktestReport {
    /// Посчитать метрики из сделок и кривой капитала.
    ///
    /// `initial_capital` нужен для доходности в долях. Кривая капитала — это
    /// equity по каждому бару (включая нереализованный P&L открытой позиции).
    pub fn compute(
        trades: Vec<SimTrade>,
        equity_curve: Vec<(i64, f64)>,
        initial_capital: f64,
    ) -> Self {
        let final_equity = equity_curve
            .last()
            .map(|(_, e)| *e)
            .unwrap_or(initial_capital);
        let net_pnl = final_equity - initial_capital;
        let return_pct = if initial_capital != 0.0 {
            net_pnl / initial_capital
        } else {
            0.0
        };

        // Победы/поражения — по закрывающим сделкам (ненулевой realized_pnl).
        let mut gross_win = 0.0;
        let mut gross_loss = 0.0; // положительная сумма абсолютных убытков
        let mut wins = 0usize;
        let mut losses = 0usize;
        for t in &trades {
            if t.realized_pnl > 0.0 {
                wins += 1;
                gross_win += t.realized_pnl;
            } else if t.realized_pnl < 0.0 {
                losses += 1;
                gross_loss += -t.realized_pnl;
            }
        }
        let closing = wins + losses;
        let win_rate = if closing > 0 {
            wins as f64 / closing as f64
        } else {
            0.0
        };
        let profit_factor = if gross_loss > 0.0 {
            gross_win / gross_loss
        } else if gross_win > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };
        let avg_win = if wins > 0 {
            gross_win / wins as f64
        } else {
            0.0
        };
        let avg_loss = if losses > 0 {
            gross_loss / losses as f64
        } else {
            0.0
        };

        let max_drawdown = max_drawdown(&equity_curve);
        let sharpe = sharpe_ratio(&equity_curve);

        Self {
            trades,
            equity_curve,
            metrics: PerfMetrics {
                net_pnl,
                return_pct,
                trades: 0, // заполняется ниже
                wins,
                losses,
                win_rate,
                profit_factor,
                max_drawdown,
                sharpe,
                avg_win,
                avg_loss,
            },
        }
        .with_trade_count()
    }

    fn with_trade_count(mut self) -> Self {
        self.metrics.trades = self.trades.len();
        self
    }
}

/// Максимальная относительная просадка кривой капитала (peak-to-trough), 0..1.
fn max_drawdown(curve: &[(i64, f64)]) -> f64 {
    let mut peak = f64::NEG_INFINITY;
    let mut max_dd = 0.0;
    for &(_, eq) in curve {
        if eq > peak {
            peak = eq;
        }
        if peak > 0.0 {
            let dd = (peak - eq) / peak;
            if dd > max_dd {
                max_dd = dd;
            }
        }
    }
    max_dd
}

/// Коэффициент Шарпа по пошаговым доходностям капитала (mean/std), без
/// risk-free и аннуализации. `0`, если точек мало или волатильность нулевая.
fn sharpe_ratio(curve: &[(i64, f64)]) -> f64 {
    if curve.len() < 2 {
        return 0.0;
    }
    let rets: Vec<f64> = curve
        .windows(2)
        .map(|w| {
            let prev = w[0].1;
            if prev != 0.0 {
                (w[1].1 - prev) / prev
            } else {
                0.0
            }
        })
        .collect();
    let n = rets.len() as f64;
    let mean = rets.iter().sum::<f64>() / n;
    let var = rets.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / n;
    let std = var.sqrt();
    if std > 0.0 {
        mean / std
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_summarise_wins_and_losses() {
        let trades = vec![
            SimTrade {
                ts: 1,
                side: Side::Buy,
                qty: 1.0,
                price: 100.0,
                realized_pnl: 0.0,
            },
            SimTrade {
                ts: 2,
                side: Side::Sell,
                qty: 1.0,
                price: 110.0,
                realized_pnl: 10.0,
            },
            SimTrade {
                ts: 3,
                side: Side::Buy,
                qty: 1.0,
                price: 110.0,
                realized_pnl: 0.0,
            },
            SimTrade {
                ts: 4,
                side: Side::Sell,
                qty: 1.0,
                price: 105.0,
                realized_pnl: -5.0,
            },
        ];
        let curve = vec![(1, 1000.0), (2, 1010.0), (3, 1010.0), (4, 1005.0)];
        let r = BacktestReport::compute(trades, curve, 1000.0);
        assert_eq!(r.metrics.trades, 4);
        assert_eq!(r.metrics.wins, 1);
        assert_eq!(r.metrics.losses, 1);
        assert!((r.metrics.win_rate - 0.5).abs() < 1e-12);
        assert!((r.metrics.profit_factor - 2.0).abs() < 1e-12); // 10 / 5
        assert!((r.metrics.net_pnl - 5.0).abs() < 1e-12);
        assert!((r.metrics.return_pct - 0.005).abs() < 1e-12);
        assert!((r.metrics.avg_win - 10.0).abs() < 1e-12);
        assert!((r.metrics.avg_loss - 5.0).abs() < 1e-12);
    }

    #[test]
    fn max_drawdown_detects_peak_to_trough() {
        // пик 1200, дно 900 → просадка 0.25
        let curve = vec![(1, 1000.0), (2, 1200.0), (3, 900.0), (4, 1100.0)];
        let dd = max_drawdown(&curve);
        assert!((dd - 0.25).abs() < 1e-12);
    }

    #[test]
    fn profit_factor_infinite_without_losses() {
        let trades = vec![SimTrade {
            ts: 1,
            side: Side::Sell,
            qty: 1.0,
            price: 110.0,
            realized_pnl: 10.0,
        }];
        let curve = vec![(1, 1010.0)];
        let r = BacktestReport::compute(trades, curve, 1000.0);
        assert!(r.metrics.profit_factor.is_infinite());
    }
}
