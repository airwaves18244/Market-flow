//! Счёт и позиции симулятора: учёт средней цены, реализованного и
//! нереализованного P&L. Чистая арифметика, тестируемая отдельно.

use std::collections::BTreeMap;

use crate::model::Side;

/// Позиция по инструменту: знаковый объём и средняя цена входа.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    /// Знаковый объём: `+` лонг, `−` шорт, `0` — нет позиции.
    pub qty: f64,
    /// Средняя цена открытой части позиции (0, если позиции нет).
    pub avg_price: f64,
}

impl Position {
    /// Пустая позиция.
    pub fn flat() -> Self {
        Self {
            qty: 0.0,
            avg_price: 0.0,
        }
    }

    /// Нереализованный P&L при цене `mark`.
    pub fn unrealized(&self, mark: f64) -> f64 {
        if self.qty == 0.0 {
            0.0
        } else {
            (mark - self.avg_price) * self.qty
        }
    }

    /// Применить исполнение (`side`, `qty`, `price`). Возвращает реализованный
    /// этой сделкой P&L (0 при открытии/наращивании, ненулевой при
    /// сокращении/закрытии/перевороте).
    pub fn apply_fill(&mut self, side: Side, qty: f64, price: f64) -> f64 {
        let signed = side.sign() * qty;
        let mut realized = 0.0;

        if self.qty == 0.0 || self.qty.signum() == signed.signum() {
            // Открытие или наращивание: средняя цена — взвешенная.
            let new_qty = self.qty + signed;
            self.avg_price = (self.avg_price * self.qty.abs() + price * qty) / new_qty.abs();
            self.qty = new_qty;
        } else {
            // Сокращение/закрытие (возможно с переворотом).
            let closing = qty.min(self.qty.abs());
            realized = closing * (price - self.avg_price) * self.qty.signum();
            let new_qty = self.qty + signed;
            self.qty = new_qty;
            if self.qty == 0.0 {
                self.avg_price = 0.0;
            } else if self.qty.signum() == signed.signum() {
                // Переворот за ноль: остаток открывает позицию по цене сделки.
                self.avg_price = price;
            }
        }
        realized
    }
}

/// Торговый счёт: наличность, реализованный P&L и позиции по инструментам.
#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    pub cash: f64,
    pub realized_pnl: f64,
    pub positions: BTreeMap<String, Position>,
}

impl Account {
    /// Новый счёт со стартовой наличностью.
    pub fn new(initial_cash: f64) -> Self {
        Self {
            cash: initial_cash,
            realized_pnl: 0.0,
            positions: BTreeMap::new(),
        }
    }

    /// Позиция по инструменту (пустая, если её нет).
    pub fn position(&self, symbol: &str) -> Position {
        self.positions.get(symbol).copied().unwrap_or_else(Position::flat)
    }

    /// Применить исполнение к счёту: денежный поток + позиция + реализованный P&L.
    pub fn apply_fill(&mut self, symbol: &str, side: Side, qty: f64, price: f64) -> f64 {
        // Покупка тратит наличность, продажа — пополняет.
        self.cash -= side.sign() * qty * price;
        let pos = self
            .positions
            .entry(symbol.to_string())
            .or_insert_with(Position::flat);
        let realized = pos.apply_fill(side, qty, price);
        self.realized_pnl += realized;
        realized
    }

    /// Капитал mark-to-market: наличность + рыночная стоимость позиций по
    /// переданным ценам (`symbol → mark`). Позиции без цены берутся по средней.
    pub fn equity(&self, marks: &BTreeMap<String, f64>) -> f64 {
        let mut eq = self.cash;
        for (sym, pos) in &self.positions {
            let mark = marks.get(sym).copied().unwrap_or(pos.avg_price);
            eq += pos.qty * mark;
        }
        eq
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_round_trip_books_pnl_and_cash() {
        let mut acc = Account::new(1_000.0);
        // Купить 1 @100, продать 1 @110.
        let r1 = acc.apply_fill("SBER", Side::Buy, 1.0, 100.0);
        assert_eq!(r1, 0.0);
        assert_eq!(acc.cash, 900.0);
        assert_eq!(acc.position("SBER").qty, 1.0);
        let r2 = acc.apply_fill("SBER", Side::Sell, 1.0, 110.0);
        assert!((r2 - 10.0).abs() < 1e-9);
        assert!((acc.realized_pnl - 10.0).abs() < 1e-9);
        assert_eq!(acc.position("SBER").qty, 0.0);
        assert!((acc.cash - 1_010.0).abs() < 1e-9);
    }

    #[test]
    fn averaging_in_updates_avg_price() {
        let mut pos = Position::flat();
        pos.apply_fill(Side::Buy, 1.0, 100.0);
        pos.apply_fill(Side::Buy, 1.0, 102.0);
        assert_eq!(pos.qty, 2.0);
        assert!((pos.avg_price - 101.0).abs() < 1e-9);
        assert!((pos.unrealized(105.0) - 8.0).abs() < 1e-9); // (105-101)*2
    }

    #[test]
    fn reversal_through_zero_resets_basis() {
        let mut pos = Position::flat();
        pos.apply_fill(Side::Buy, 1.0, 100.0); // long 1 @100
        let realized = pos.apply_fill(Side::Sell, 3.0, 110.0); // close 1 (+10), short 2 @110
        assert!((realized - 10.0).abs() < 1e-9);
        assert_eq!(pos.qty, -2.0);
        assert!((pos.avg_price - 110.0).abs() < 1e-9);
    }
}
