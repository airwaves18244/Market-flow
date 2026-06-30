//! Типы заявок симулятора исполнения.

use crate::model::Side;

/// Тип заявки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    /// Рыночная: исполняется немедленно по встречной стороне стакана.
    Market,
    /// Лимитная: исполняется по цене не хуже лимита.
    Limit,
    /// Стоп: при достижении стоп-цены превращается в рыночную.
    Stop,
}

/// Время жизни заявки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInForce {
    /// До отмены.
    Gtc,
    /// На текущую сессию (в симуляторе эквивалентно GTC).
    Day,
    /// Исполнить немедленно (частично) и отменить остаток.
    Ioc,
}

/// Состояние заявки в её жизненном цикле.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
}

impl OrderStatus {
    /// Код состояния для DTO/UI.
    pub fn code(self) -> &'static str {
        match self {
            OrderStatus::New => "new",
            OrderStatus::PartiallyFilled => "partially_filled",
            OrderStatus::Filled => "filled",
            OrderStatus::Cancelled => "cancelled",
            OrderStatus::Rejected => "rejected",
        }
    }

    /// Заявка ещё «живёт» в книге (может исполняться/отменяться).
    pub fn is_open(self) -> bool {
        matches!(self, OrderStatus::New | OrderStatus::PartiallyFilled)
    }
}

/// Заявка симулятора.
#[derive(Debug, Clone, PartialEq)]
pub struct Order {
    pub id: u64,
    pub symbol: String,
    pub side: Side,
    /// Полный объём заявки (в единицах/лотах).
    pub qty: f64,
    /// Уже исполненный объём.
    pub filled: f64,
    /// Цена для лимитной/стоп-заявки; `None` для рыночной.
    pub price: Option<f64>,
    pub kind: OrderType,
    pub tif: TimeInForce,
    pub status: OrderStatus,
}

impl Order {
    /// Остаток к исполнению.
    pub fn remaining(&self) -> f64 {
        (self.qty - self.filled).max(0.0)
    }
}

/// Факт исполнения (часть заявки) — для блоттера и события `fill:tick`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fill {
    pub order_id: u64,
    pub ts: i64,
    pub side: Side,
    pub qty: f64,
    pub price: f64,
    /// Реализованный этим исполнением P&L (0, если открывает/наращивает позицию).
    pub realized_pnl: f64,
}
