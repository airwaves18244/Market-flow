//! Стакан котировок (DOM — Depth of Market).
//!
//! Чистая модель книги заявок: уровни bid/ask, лучшие цены, спред, средняя
//! цена, дисбаланс спроса/предложения и кумулятивная глубина (лесенка). Сетевой
//! стрим стакана (`SubscribeOrderBook`) живёт выше, в `app`; сюда приходят уже
//! разобранные уровни.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

/// Уровень стакана: цена и доступный на ней объём (штуки/контракты).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Level {
    pub price: f64,
    pub size: f64,
}

/// Стакан заявок: bids (покупка) и asks (продажа).
///
/// По соглашению `bids` отсортированы по убыванию цены (лучшая — первая),
/// `asks` — по возрастанию (лучшая — первая). [`OrderBook::new`] наводит этот
/// порядок сам, поэтому вход может быть в любом порядке.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrderBook {
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
}

impl OrderBook {
    /// Собрать стакан, упорядочив стороны (bids ↓, asks ↑).
    pub fn new(mut bids: Vec<Level>, mut asks: Vec<Level>) -> Self {
        bids.sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap_or(Ordering::Equal));
        asks.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(Ordering::Equal));
        Self { bids, asks }
    }

    /// Лучшая заявка на покупку (наибольшая цена bid).
    pub fn best_bid(&self) -> Option<Level> {
        self.bids.first().copied()
    }

    /// Лучшая заявка на продажу (наименьшая цена ask).
    pub fn best_ask(&self) -> Option<Level> {
        self.asks.first().copied()
    }

    /// Средняя цена `(bid + ask) / 2`. `None`, если одной из сторон нет.
    pub fn mid(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(b), Some(a)) => Some((b.price + a.price) / 2.0),
            _ => None,
        }
    }

    /// Спред `ask − bid`. `None`, если одной из сторон нет.
    pub fn spread(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(b), Some(a)) => Some(a.price - b.price),
            _ => None,
        }
    }

    fn side_volume(levels: &[Level]) -> f64 {
        levels.iter().map(|l| l.size).sum()
    }

    /// Суммарный объём на стороне покупки.
    pub fn bid_volume(&self) -> f64 {
        Self::side_volume(&self.bids)
    }

    /// Суммарный объём на стороне продажи.
    pub fn ask_volume(&self) -> f64 {
        Self::side_volume(&self.asks)
    }

    /// Дисбаланс спроса/предложения в диапазоне `-1..1`:
    /// `(bidVol − askVol) / (bidVol + askVol)`. Положителен при перевесе спроса.
    /// `None`, если книга пуста.
    pub fn imbalance(&self) -> Option<f64> {
        let total = self.bid_volume() + self.ask_volume();
        if total <= 0.0 {
            None
        } else {
            Some((self.bid_volume() - self.ask_volume()) / total)
        }
    }

    /// Кумулятивная глубина по bids: для каждого уровня — он сам и накопленный
    /// объём от лучшей цены вглубь.
    pub fn cumulative_bids(&self) -> Vec<(Level, f64)> {
        cumulate(&self.bids)
    }

    /// Кумулятивная глубина по asks (от лучшей цены вглубь).
    pub fn cumulative_asks(&self) -> Vec<(Level, f64)> {
        cumulate(&self.asks)
    }
}

fn cumulate(levels: &[Level]) -> Vec<(Level, f64)> {
    let mut acc = 0.0;
    levels
        .iter()
        .map(|l| {
            acc += l.size;
            (*l, acc)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lvl(price: f64, size: f64) -> Level {
        Level { price, size }
    }

    #[test]
    fn new_orders_sides() {
        // Вход вперемешку — конструктор наводит порядок.
        let b = OrderBook::new(
            vec![lvl(99.0, 1.0), lvl(100.0, 2.0), lvl(98.0, 3.0)],
            vec![lvl(102.0, 1.0), lvl(101.0, 2.0)],
        );
        assert_eq!(b.best_bid(), Some(lvl(100.0, 2.0)));
        assert_eq!(b.best_ask(), Some(lvl(101.0, 2.0)));
        assert_eq!(b.bids.first().unwrap().price, 100.0);
        assert_eq!(b.asks.first().unwrap().price, 101.0);
    }

    #[test]
    fn mid_and_spread() {
        let b = OrderBook::new(vec![lvl(100.0, 1.0)], vec![lvl(102.0, 1.0)]);
        assert_eq!(b.mid(), Some(101.0));
        assert_eq!(b.spread(), Some(2.0));
    }

    #[test]
    fn mid_spread_none_when_one_side_empty() {
        let b = OrderBook::new(vec![lvl(100.0, 1.0)], vec![]);
        assert_eq!(b.mid(), None);
        assert_eq!(b.spread(), None);
    }

    #[test]
    fn imbalance_signs_with_demand() {
        // Спрос (15) > предложение (5) ⇒ положительный дисбаланс.
        let b = OrderBook::new(
            vec![lvl(100.0, 10.0), lvl(99.0, 5.0)],
            vec![lvl(101.0, 5.0)],
        );
        let imb = b.imbalance().unwrap();
        assert!((imb - (15.0 - 5.0) / 20.0).abs() < 1e-12);
        assert!(imb > 0.0);
    }

    #[test]
    fn imbalance_none_when_empty() {
        assert_eq!(OrderBook::default().imbalance(), None);
    }

    #[test]
    fn cumulative_depth_accumulates() {
        let b = OrderBook::new(
            vec![lvl(100.0, 2.0), lvl(99.0, 3.0), lvl(98.0, 5.0)],
            vec![],
        );
        let cum = b.cumulative_bids();
        assert_eq!(cum[0].1, 2.0);
        assert_eq!(cum[1].1, 5.0);
        assert_eq!(cum[2].1, 10.0);
    }
}
