//! Лента сделок (Time & Sales): классификация сторон и агрегаты.
//!
//! Из обезличенной ленты [`Trade`] получаем сторону каждой сделки и сводные
//! метрики окна: купленный/проданный объём и оборот, накопленную дельту (CVD),
//! VWAP. Если биржа не отдала инициатора (`buyer_initiated == None`), сторона
//! определяется правилом тика по движению цены.

use serde::{Deserialize, Serialize};

use crate::Trade;

/// Сторона сделки относительно агрессора.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    /// Инициатор — покупатель (агрессор по ask).
    Buy,
    /// Инициатор — продавец (агрессор по bid).
    Sell,
}

/// Определить сторону каждой сделки в ленте.
///
/// Приоритет — у явного признака биржи (`buyer_initiated`). Без него работает
/// правило тика: цена выше предыдущей — покупка, ниже — продажа, равна —
/// наследуем предыдущую сторону (первая по умолчанию — покупка).
pub fn classify_sides(trades: &[Trade]) -> Vec<Side> {
    let mut out = Vec::with_capacity(trades.len());
    let mut prev_price: Option<f64> = None;
    let mut prev_side = Side::Buy;
    for t in trades {
        let side = match t.buyer_initiated {
            Some(true) => Side::Buy,
            Some(false) => Side::Sell,
            None => match prev_price {
                Some(p) if t.price > p => Side::Buy,
                Some(p) if t.price < p => Side::Sell,
                _ => prev_side,
            },
        };
        out.push(side);
        prev_side = side;
        prev_price = Some(t.price);
    }
    out
}

/// Сводные агрегаты по ленте сделок за окно.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TapeStats {
    /// Число сделок.
    pub trades: usize,
    /// Купленный объём (сделки на стороне `Buy`).
    pub buy_volume: f64,
    /// Проданный объём (сделки на стороне `Sell`).
    pub sell_volume: f64,
    /// Купленный оборот (денежный).
    pub buy_turnover: f64,
    /// Проданный оборот (денежный).
    pub sell_turnover: f64,
    /// Последняя цена в окне.
    pub last_price: Option<f64>,
}

impl TapeStats {
    /// Накопленная дельта объёма: `buyVol − sellVol`.
    pub fn cvd(&self) -> f64 {
        self.buy_volume - self.sell_volume
    }

    /// Суммарный объём.
    pub fn volume(&self) -> f64 {
        self.buy_volume + self.sell_volume
    }

    /// Суммарный денежный оборот.
    pub fn turnover(&self) -> f64 {
        self.buy_turnover + self.sell_turnover
    }

    /// Средневзвешенная по объёму цена (VWAP). `None` при нулевом объёме.
    pub fn vwap(&self) -> Option<f64> {
        let v = self.volume();
        if v <= 0.0 {
            None
        } else {
            Some(self.turnover() / v)
        }
    }
}

/// Посчитать агрегаты ленты, классифицируя сторону каждой сделки.
pub fn tape_stats(trades: &[Trade]) -> TapeStats {
    let sides = classify_sides(trades);
    let mut stats = TapeStats {
        trades: trades.len(),
        buy_volume: 0.0,
        sell_volume: 0.0,
        buy_turnover: 0.0,
        sell_turnover: 0.0,
        last_price: trades.last().map(|t| t.price),
    };
    for (t, side) in trades.iter().zip(sides) {
        match side {
            Side::Buy => {
                stats.buy_volume += t.size;
                stats.buy_turnover += t.turnover();
            }
            Side::Sell => {
                stats.sell_volume += t.size;
                stats.sell_turnover += t.turnover();
            }
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trade(price: f64, size: f64, bi: Option<bool>) -> Trade {
        Trade {
            ts: 0,
            price,
            size,
            buyer_initiated: bi,
        }
    }

    #[test]
    fn explicit_side_wins_over_tick_rule() {
        // Цена падает, но биржа явно пометила покупку — берём её.
        let sides = classify_sides(&[trade(100.0, 1.0, None), trade(99.0, 1.0, Some(true))]);
        assert_eq!(sides, vec![Side::Buy, Side::Buy]);
    }

    #[test]
    fn tick_rule_classifies_by_price_move() {
        let sides = classify_sides(&[
            trade(100.0, 1.0, None), // старт ⇒ Buy
            trade(101.0, 1.0, None), // вверх ⇒ Buy
            trade(100.5, 1.0, None), // вниз ⇒ Sell
            trade(100.5, 1.0, None), // равна ⇒ наследует Sell
        ]);
        assert_eq!(sides, vec![Side::Buy, Side::Buy, Side::Sell, Side::Sell]);
    }

    #[test]
    fn tape_stats_split_and_cvd() {
        let trades = [
            trade(100.0, 10.0, Some(true)), // buy 10
            trade(101.0, 4.0, Some(false)), // sell 4
            trade(102.0, 6.0, Some(true)),  // buy 6
        ];
        let s = tape_stats(&trades);
        assert_eq!(s.trades, 3);
        assert_eq!(s.buy_volume, 16.0);
        assert_eq!(s.sell_volume, 4.0);
        assert_eq!(s.cvd(), 12.0);
        assert_eq!(s.volume(), 20.0);
        assert_eq!(s.last_price, Some(102.0));
    }

    #[test]
    fn vwap_weights_by_volume() {
        let trades = [trade(100.0, 1.0, Some(true)), trade(200.0, 3.0, Some(true))];
        let s = tape_stats(&trades);
        // (100*1 + 200*3) / 4 = 175
        assert_eq!(s.vwap(), Some(175.0));
    }

    #[test]
    fn vwap_none_on_empty() {
        let s = tape_stats(&[]);
        assert_eq!(s.vwap(), None);
        assert_eq!(s.last_price, None);
    }
}
