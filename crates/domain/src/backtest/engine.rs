//! Движок бэктеста: проигрывает бары, исполняет сигналы стратегии и считает
//! P&L по позиции (mark-to-market).
//!
//! Чистый и детерминированный: при одинаковых барах/конфиге результат
//! повторяется бит-в-бит. Учёт позиции (средняя цена, реализованный P&L,
//! наличность) ведётся здесь же — тот же принцип позднее переиспользует
//! симулятор исполнения `trading::sim`.

use crate::model::{Bar, Side};

use super::report::{BacktestReport, SimTrade};
use super::strategy::{BarContext, Strategy};

/// Когда исполнять сигнал, сгенерированный на баре `i`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillTiming {
    /// По открытию следующего бара (реалистичнее: сигнал на закрытии бара `i`
    /// исполняется на открытии `i+1`). Сигнал на последнем баре не исполняется.
    NextOpen,
    /// По закрытию текущего бара (мгновенно, оптимистично).
    ThisClose,
}

/// Конфигурация прогона бэктеста.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BacktestConfig {
    /// Стартовый капитал (наличность).
    pub initial_capital: f64,
    /// Комиссия за единицу исполненного объёма (списывается с наличности).
    pub commission: f64,
    /// Проскальзывание в цене за единицу: покупка платит `price + slippage`,
    /// продажа получает `price − slippage`.
    pub slippage: f64,
    /// Момент исполнения сигнала.
    pub fill_timing: FillTiming,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            initial_capital: 100_000.0,
            commission: 0.0,
            slippage: 0.0,
            fill_timing: FillTiming::NextOpen,
        }
    }
}

/// Внутренний учёт позиции и наличности.
struct Book {
    cash: f64,
    position: f64,
    avg_price: f64,
    realized: f64,
}

impl Book {
    fn new(cash: f64) -> Self {
        Self {
            cash,
            position: 0.0,
            avg_price: 0.0,
            realized: 0.0,
        }
    }

    /// Исполнить рыночную заявку (`side`, `qty`) по цене `price` (уже со
    /// слиппеджем). Возвращает реализованный этой сделкой P&L.
    fn fill(&mut self, side: Side, qty: f64, price: f64, commission: f64) -> f64 {
        self.cash -= commission * qty;
        let signed = side.sign() * qty;
        let mut realized = 0.0;

        if self.position == 0.0 || self.position.signum() == signed.signum() {
            // Открытие или наращивание в ту же сторону: средняя цена — взвешенная.
            let new_pos = self.position + signed;
            self.avg_price =
                (self.avg_price * self.position.abs() + price * qty) / new_pos.abs();
            self.position = new_pos;
        } else {
            // Сокращение/закрытие (возможно с переворотом).
            let closing = qty.min(self.position.abs());
            realized = closing * (price - self.avg_price) * self.position.signum();
            self.realized += realized;
            let new_pos = self.position + signed;
            self.position = new_pos;
            // Переворот за ноль: остаток открывает позицию по цене сделки.
            if self.position != 0.0 && self.position.signum() == signed.signum() {
                self.avg_price = price;
            }
        }

        // Денежный поток сделки: покупка тратит наличность, продажа — пополняет.
        self.cash -= signed * price;
        realized
    }

    /// Капитал mark-to-market при цене `mark`.
    fn equity(&self, mark: f64) -> f64 {
        self.cash + self.position * mark
    }
}

/// Прогнать бэктест стратегии по серии баров.
///
/// Бары должны идти по возрастанию `ts`. На каждом баре стратегия видит окно
/// `bars[..=i]` и текущую позицию; её целевая позиция исполняется в момент,
/// заданный `config.fill_timing`. Капитал фиксируется по каждому бару
/// (mark-to-market по закрытию).
pub fn run_backtest(
    bars: &[Bar],
    strategy: &mut dyn Strategy,
    config: BacktestConfig,
) -> BacktestReport {
    let mut book = Book::new(config.initial_capital);
    let mut trades: Vec<SimTrade> = Vec::new();
    let mut equity_curve: Vec<(i64, f64)> = Vec::with_capacity(bars.len());

    for i in 0..bars.len() {
        let ctx = BarContext {
            bars: &bars[..=i],
            index: i,
            position: book.position,
        };
        if let Some(signal) = strategy.on_bar(&ctx) {
            // Цена и время исполнения по выбранному режиму.
            let exec = match config.fill_timing {
                FillTiming::ThisClose => Some((bars[i].close, bars[i].ts)),
                FillTiming::NextOpen => bars.get(i + 1).map(|b| (b.open, b.ts)),
            };
            if let Some((base_price, exec_ts)) = exec {
                let delta = signal.target_position - book.position;
                if delta.abs() > f64::EPSILON {
                    let side = if delta > 0.0 { Side::Buy } else { Side::Sell };
                    let qty = delta.abs();
                    let price = base_price + side.sign() * config.slippage;
                    let realized = book.fill(side, qty, price, config.commission);
                    trades.push(SimTrade {
                        ts: exec_ts,
                        side,
                        qty,
                        price,
                        realized_pnl: realized,
                    });
                }
            }
        }
        equity_curve.push((bars[i].ts, book.equity(bars[i].close)));
    }

    BacktestReport::compute(trades, equity_curve, config.initial_capital)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backtest::strategy::Signal;

    fn bar(ts: i64, open: f64, close: f64) -> Bar {
        Bar {
            ts,
            open,
            high: open.max(close),
            low: open.min(close),
            close,
            volume: 1.0,
        }
    }

    /// Стратегия-заглушка: возвращает заранее заданные целевые позиции по индексу.
    struct Scripted {
        targets: Vec<Option<f64>>,
    }
    impl Strategy for Scripted {
        fn id(&self) -> &'static str {
            "scripted"
        }
        fn on_bar(&mut self, ctx: &BarContext) -> Option<Signal> {
            self.targets
                .get(ctx.index)
                .copied()
                .flatten()
                .map(Signal::target)
        }
    }

    #[test]
    fn long_round_trip_books_realized_pnl() {
        // Купить на баре0 (исполнение на open бара1 = 100), продать на баре2
        // (исполнение на open бара3 = 110). P&L = +10.
        let bars = [
            bar(1, 100.0, 100.0),
            bar(2, 100.0, 105.0),
            bar(3, 110.0, 110.0),
            bar(4, 110.0, 110.0),
        ];
        let mut strat = Scripted {
            targets: vec![Some(1.0), None, Some(0.0), None],
        };
        let r = run_backtest(&bars, &mut strat, BacktestConfig::default());
        assert_eq!(r.trades.len(), 2);
        assert_eq!(r.trades[0].side, Side::Buy);
        assert!((r.trades[0].price - 100.0).abs() < 1e-9);
        assert_eq!(r.trades[1].side, Side::Sell);
        assert!((r.trades[1].realized_pnl - 10.0).abs() < 1e-9);
        assert!((r.metrics.net_pnl - 10.0).abs() < 1e-9);
        // Капитал в конце = старт + 10.
        assert!((r.equity_curve.last().unwrap().1 - 100_010.0).abs() < 1e-9);
    }

    #[test]
    fn this_close_fills_immediately_and_slippage_applies() {
        let bars = [bar(1, 100.0, 100.0), bar(2, 100.0, 100.0)];
        let mut strat = Scripted {
            targets: vec![Some(2.0), Some(0.0)],
        };
        let cfg = BacktestConfig {
            initial_capital: 1_000.0,
            commission: 1.0,
            slippage: 0.5,
            fill_timing: FillTiming::ThisClose,
        };
        let r = run_backtest(&bars, &mut strat, cfg);
        // Покупка 2 @ 100.5 (slippage), продажа 2 @ 99.5; убыток 2*1 от слиппеджа
        // плюс комиссия 2*1 (вход) + 2*1 (выход) = 4. Итого P&L = -2 - 4 = -6.
        assert_eq!(r.trades.len(), 2);
        assert!((r.trades[0].price - 100.5).abs() < 1e-9);
        assert!((r.trades[1].price - 99.5).abs() < 1e-9);
        assert!((r.metrics.net_pnl + 6.0).abs() < 1e-9);
    }

    #[test]
    fn next_open_skips_signal_on_last_bar() {
        let bars = [bar(1, 100.0, 100.0)];
        let mut strat = Scripted {
            targets: vec![Some(1.0)],
        };
        let r = run_backtest(&bars, &mut strat, BacktestConfig::default());
        // Нет следующего бара для исполнения → сделок нет.
        assert!(r.trades.is_empty());
        assert_eq!(r.equity_curve.len(), 1);
    }
}
