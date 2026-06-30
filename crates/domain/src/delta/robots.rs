//! Детектирующие «роботы»: распознают на ленте сделок следы хорошо известных
//! алгоритмов — серии равных лотов, айсберг-доливки, поглощение (absorption).
//!
//! Это **анализ, а не исполнение**: роботы только помечают паттерны для оверлея
//! на графике дельты. Все детекторы — чистые функции над лентой (и, для
//! айсберга, снимком стакана), покрытые тестами на «подстроенных» лентах.

use std::collections::BTreeMap;

use crate::metrics::flow::cumulative_volume_delta;
use crate::model::{OrderBook, Trade};

/// Тип распознанного паттерна.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RobotKind {
    /// Серия сделок одинакового объёма (алгоритм режет заявку равными лотами).
    SameLot,
    /// Айсберг: на уровне исполнено заметно больше, чем показывал стакан.
    Iceberg,
    /// Поглощение: большая дельта при малом движении цены.
    Absorption,
}

impl RobotKind {
    /// Машинный код вида робота (для DTO/UI).
    pub fn code(self) -> &'static str {
        match self {
            RobotKind::SameLot => "same_lot",
            RobotKind::Iceberg => "iceberg",
            RobotKind::Absorption => "absorption",
        }
    }
}

/// Сигнал робота: где и что распознано.
#[derive(Debug, Clone, PartialEq)]
pub struct RobotSignal {
    pub kind: RobotKind,
    /// Время паттерна, UNIX-секунды UTC.
    pub ts: i64,
    /// Цена, к которой привязан паттерн.
    pub price: f64,
    /// Сила сигнала (трактовка зависит от вида: длина серии, кратность объёма,
    /// величина дельты) — для ранжирования/прозрачности маркера.
    pub strength: f64,
    /// Короткое человекочитаемое пояснение.
    pub note: String,
}

/// Настройки детекторов (пороги + включение).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RobotConfig {
    pub same_lot_enabled: bool,
    /// Минимальная длина серии равных лотов.
    pub same_lot_run: usize,
    /// Относительный допуск равенства объёма (доля, напр. 0.0 — точное равенство).
    pub lot_tolerance: f64,

    pub iceberg_enabled: bool,
    /// Во сколько раз исполненный на уровне объём должен превзойти показанный
    /// в стакане, чтобы считаться айсбергом.
    pub iceberg_volume_mult: f64,

    pub absorption_enabled: bool,
    /// Минимальная |дельта| окна для поглощения.
    pub absorption_min_delta: f64,
    /// Максимальный размах цены (high−low) окна для поглощения.
    pub absorption_max_move: f64,
}

impl Default for RobotConfig {
    fn default() -> Self {
        Self {
            same_lot_enabled: true,
            same_lot_run: 4,
            lot_tolerance: 0.0,
            iceberg_enabled: true,
            iceberg_volume_mult: 3.0,
            absorption_enabled: true,
            absorption_min_delta: 100.0,
            absorption_max_move: 1.0,
        }
    }
}

/// Сканер: запускает включённые детекторы и собирает сигналы (по возрастанию ts).
#[derive(Debug, Clone, Copy)]
pub struct RobotScanner {
    pub config: RobotConfig,
}

impl RobotScanner {
    pub fn new(config: RobotConfig) -> Self {
        Self { config }
    }

    /// Прогнать включённые детекторы по ленте (и снимку стакана, если есть).
    pub fn scan(&self, trades: &[Trade], book: Option<&OrderBook>) -> Vec<RobotSignal> {
        let c = &self.config;
        let mut out = Vec::new();
        if c.same_lot_enabled {
            out.extend(detect_same_lots(trades, c.same_lot_run, c.lot_tolerance));
        }
        if c.iceberg_enabled {
            if let Some(b) = book {
                out.extend(detect_icebergs(trades, b, c.iceberg_volume_mult));
            }
        }
        if c.absorption_enabled {
            out.extend(detect_absorption(
                trades,
                c.absorption_min_delta,
                c.absorption_max_move,
            ));
        }
        out.sort_by_key(|s| s.ts);
        out
    }
}

/// Равны ли объёмы с заданным относительным допуском.
fn sizes_equal(a: f64, b: f64, tol: f64) -> bool {
    if tol <= 0.0 {
        (a - b).abs() < f64::EPSILON
    } else {
        let base = a.abs().max(b.abs()).max(f64::EPSILON);
        (a - b).abs() / base <= tol
    }
}

/// Найти серии из `run` и более подряд идущих сделок одинакового объёма.
/// Сигнал ставится на последнюю сделку серии; `strength` = длина серии.
pub fn detect_same_lots(trades: &[Trade], run: usize, tol: f64) -> Vec<RobotSignal> {
    let mut out = Vec::new();
    if run < 2 || trades.len() < run {
        return out;
    }
    let mut i = 0;
    while i < trades.len() {
        let mut j = i + 1;
        while j < trades.len() && sizes_equal(trades[j].size, trades[i].size, tol) {
            j += 1;
        }
        let len = j - i;
        if len >= run {
            let last = &trades[j - 1];
            out.push(RobotSignal {
                kind: RobotKind::SameLot,
                ts: last.ts,
                price: last.price,
                strength: len as f64,
                note: format!("{len} сделок по {} лот.", trades[i].size),
            });
        }
        i = j;
    }
    out
}

/// Айсберг: на ценовом уровне исполнено в `mult`+ раз больше, чем показывал
/// стакан на этом уровне. Сигнал ставится на последнюю сделку уровня.
pub fn detect_icebergs(trades: &[Trade], book: &OrderBook, mult: f64) -> Vec<RobotSignal> {
    // Показанный объём по цене (биды и аски).
    let mut shown: BTreeMap<i64, f64> = BTreeMap::new();
    let key = |p: f64| p.to_bits() as i64;
    for lvl in book.bids.iter().chain(book.asks.iter()) {
        *shown.entry(key(lvl.price)).or_insert(0.0) += lvl.size;
    }

    // Исполненный объём и последняя сделка по цене.
    let mut traded: BTreeMap<i64, (f64, i64)> = BTreeMap::new();
    for t in trades {
        let e = traded.entry(key(t.price)).or_insert((0.0, t.ts));
        e.0 += t.size;
        e.1 = t.ts;
    }

    let mut out = Vec::new();
    for (k, (vol, last_ts)) in traded {
        if let Some(&disp) = shown.get(&k) {
            if disp > 0.0 && vol >= mult * disp {
                out.push(RobotSignal {
                    kind: RobotKind::Iceberg,
                    ts: last_ts,
                    price: f64::from_bits(k as u64),
                    strength: vol / disp,
                    note: format!("исполнено {vol} при показанных {disp}"),
                });
            }
        }
    }
    out
}

/// Поглощение: большая |дельта| при малом размахе цены в окне. Сигнал ставится
/// на последнюю сделку окна по медианной цене размаха.
pub fn detect_absorption(trades: &[Trade], min_delta: f64, max_move: f64) -> Vec<RobotSignal> {
    if trades.is_empty() {
        return Vec::new();
    }
    let delta = cumulative_volume_delta(trades);
    if delta.abs() < min_delta {
        return Vec::new();
    }
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for t in trades {
        lo = lo.min(t.price);
        hi = hi.max(t.price);
    }
    let move_ = hi - lo;
    if move_ <= max_move {
        let last = trades.last().unwrap();
        return vec![RobotSignal {
            kind: RobotKind::Absorption,
            ts: last.ts,
            price: (hi + lo) / 2.0,
            strength: delta.abs(),
            note: format!("дельта {delta:.0} при движении {move_:.2}"),
        }];
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::BookLevel;

    fn t(ts: i64, price: f64, size: f64, buy: Option<bool>) -> Trade {
        Trade {
            ts,
            price,
            size,
            buyer_initiated: buy,
        }
    }

    #[test]
    fn same_lots_detects_equal_run() {
        // 4 подряд по 10, затем разрыв.
        let trades = [
            t(1, 100.0, 10.0, Some(true)),
            t(2, 100.0, 10.0, Some(true)),
            t(3, 100.0, 10.0, Some(true)),
            t(4, 100.0, 10.0, Some(true)),
            t(5, 100.0, 7.0, Some(true)),
        ];
        let sigs = detect_same_lots(&trades, 4, 0.0);
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].kind, RobotKind::SameLot);
        assert_eq!(sigs[0].ts, 4); // последняя сделка серии
        assert_eq!(sigs[0].strength, 4.0);
    }

    #[test]
    fn same_lots_ignores_short_runs() {
        let trades = [
            t(1, 100.0, 10.0, Some(true)),
            t(2, 100.0, 10.0, Some(true)),
            t(3, 100.0, 5.0, Some(true)),
        ];
        assert!(detect_same_lots(&trades, 4, 0.0).is_empty());
    }

    #[test]
    fn iceberg_fires_when_traded_exceeds_shown() {
        let book = OrderBook {
            ts: 0,
            bids: vec![BookLevel {
                price: 100.0,
                size: 5.0,
            }],
            asks: vec![BookLevel {
                price: 100.5,
                size: 5.0,
            }],
        };
        // На 100.0 исполнено 20 при показанных 5 → 4× ≥ mult 3.
        let trades = [
            t(1, 100.0, 8.0, Some(false)),
            t(2, 100.0, 12.0, Some(false)),
            t(3, 100.5, 2.0, Some(true)), // 2 < 3×5, не айсберг
        ];
        let sigs = detect_icebergs(&trades, &book, 3.0);
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].kind, RobotKind::Iceberg);
        assert_eq!(sigs[0].price, 100.0);
        assert!((sigs[0].strength - 4.0).abs() < 1e-9);
    }

    #[test]
    fn absorption_fires_on_big_delta_small_move() {
        // Дельта +150, размах цены 0.5 ≤ 1.0.
        let trades = [
            t(1, 100.0, 100.0, Some(true)),
            t(2, 100.5, 50.0, Some(true)),
        ];
        let sigs = detect_absorption(&trades, 100.0, 1.0);
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].kind, RobotKind::Absorption);
        assert!((sigs[0].strength - 150.0).abs() < 1e-9);
    }

    #[test]
    fn absorption_silent_when_price_moves() {
        let trades = [
            t(1, 100.0, 100.0, Some(true)),
            t(2, 105.0, 50.0, Some(true)),
        ];
        assert!(detect_absorption(&trades, 100.0, 1.0).is_empty());
    }

    #[test]
    fn scanner_runs_enabled_detectors_sorted() {
        let cfg = RobotConfig {
            iceberg_enabled: false,
            ..RobotConfig::default()
        };
        let trades: Vec<Trade> = (0..4).map(|i| t(i, 100.0, 10.0, Some(true))).collect();
        let sigs = RobotScanner::new(cfg).scan(&trades, None);
        // same_lot серия из 4 → один сигнал; absorption: дельта 40 < 100 → нет.
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].kind, RobotKind::SameLot);
    }
}
