//! Симулятор исполнения (paper trading): принимает заявки, исполняет их по
//! стакану/ленте и ведёт счёт. Это V2-движок исполнения — чистый, без сети;
//! живой роутинг в Finam подключается позже за фичей (тот же контракт заявок).
//!
//! Модель исполнения:
//! - **Market** — немедленно проходит встречную сторону стакана (`last_book`),
//!   при недостатке глубины добивает остаток по последнему уровню;
//! - **Limit** — при постановке исполняется маркетабельная часть по стакану,
//!   остаток встаёт в книгу и исполняется на ленте (`on_trade`) при касании цены;
//! - **Stop** — ждёт пробоя стоп-цены на ленте, затем исполняется как рыночная.

use crate::model::{BookLevel, OrderBook, Side, Trade};

use super::account::Account;
use super::order::{Fill, Order, OrderStatus, OrderType, TimeInForce};
use super::risk::{RejectReason, RiskLimits};

/// Заявка на постановку (вход симулятора).
#[derive(Debug, Clone, PartialEq)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: Side,
    pub qty: f64,
    pub kind: OrderType,
    /// Цена для лимитной/стоп-заявки; игнорируется для рыночной.
    pub price: Option<f64>,
    pub tif: TimeInForce,
}

/// Симулятор брокера: счёт, активные заявки и последний стакан.
#[derive(Debug, Clone)]
pub struct SimBroker {
    next_id: u64,
    account: Account,
    working: Vec<Order>,
    last_book: Option<OrderBook>,
    limits: RiskLimits,
}

impl SimBroker {
    /// Новый симулятор со стартовой наличностью и лимитами риска.
    pub fn new(initial_cash: f64, limits: RiskLimits) -> Self {
        Self {
            next_id: 0,
            account: Account::new(initial_cash),
            working: Vec::new(),
            last_book: None,
            limits,
        }
    }

    /// Текущий счёт.
    pub fn account(&self) -> &Account {
        &self.account
    }

    /// Активные (живые) заявки в книге.
    pub fn working_orders(&self) -> &[Order] {
        &self.working
    }

    /// Поставить заявку. Возвращает итоговое состояние заявки и список
    /// исполнений, либо причину отклонения (риск/нет ликвидности).
    pub fn submit(&mut self, req: OrderRequest, ts: i64) -> Result<(Order, Vec<Fill>), RejectReason> {
        if matches!(req.kind, OrderType::Limit | OrderType::Stop) && req.price.is_none() {
            return Err(RejectReason::MissingPrice);
        }
        let pos = self.account.position(&req.symbol).qty;
        self.limits.check(pos, req.side, req.qty)?;

        self.next_id += 1;
        let mut order = Order {
            id: self.next_id,
            symbol: req.symbol,
            side: req.side,
            qty: req.qty,
            filled: 0.0,
            price: req.price,
            kind: req.kind,
            tif: req.tif,
            status: OrderStatus::New,
        };

        let mut fills = Vec::new();
        match req.kind {
            OrderType::Market => {
                let book = self.last_book.clone().ok_or(RejectReason::NoLiquidity)?;
                let opp_empty = match order.side {
                    Side::Buy => book.asks.is_empty(),
                    Side::Sell => book.bids.is_empty(),
                };
                if opp_empty {
                    return Err(RejectReason::NoLiquidity);
                }
                fills = fill_against_book(&mut self.account, &mut order, &book, ts, true);
            }
            OrderType::Limit => {
                if let Some(book) = self.last_book.clone() {
                    fills = fill_against_book(&mut self.account, &mut order, &book, ts, false);
                }
                if order.remaining() > f64::EPSILON {
                    if req.tif == TimeInForce::Ioc {
                        // Остаток IOC отменяется (не встаёт в книгу).
                        if order.filled <= 0.0 {
                            order.status = OrderStatus::Cancelled;
                        }
                    } else {
                        self.working.push(order.clone());
                    }
                }
            }
            OrderType::Stop => {
                // Стоп ждёт пробоя на ленте — просто встаёт в книгу.
                self.working.push(order.clone());
            }
        }
        Ok((order, fills))
    }

    /// Отменить активную заявку по id.
    pub fn cancel(&mut self, id: u64) -> Result<Order, RejectReason> {
        match self.working.iter().position(|o| o.id == id) {
            Some(pos) => {
                let mut o = self.working.remove(pos);
                o.status = OrderStatus::Cancelled;
                Ok(o)
            }
            None => Err(RejectReason::NotFound),
        }
    }

    /// Обработать печать сделки на ленте: исполнить касающиеся её лимитки и
    /// сработавшие стопы. Возвращает произведённые исполнения.
    pub fn on_trade(&mut self, trade: &Trade) -> Vec<Fill> {
        let Self {
            account, working, ..
        } = self;
        let mut fills = Vec::new();
        let mut i = 0;
        while i < working.len() {
            let order = &mut working[i];
            let fill_price = match (order.kind, order.side) {
                (OrderType::Limit, Side::Buy) => (trade.price <= order.price.unwrap()).then(|| order.price.unwrap()),
                (OrderType::Limit, Side::Sell) => (trade.price >= order.price.unwrap()).then(|| order.price.unwrap()),
                (OrderType::Stop, Side::Buy) => (trade.price >= order.price.unwrap()).then_some(trade.price),
                (OrderType::Stop, Side::Sell) => (trade.price <= order.price.unwrap()).then_some(trade.price),
                _ => None,
            };
            if let Some(price) = fill_price {
                let q = order.remaining().min(trade.size);
                if q > f64::EPSILON {
                    fills.push(execute(account, order, q, price, trade.ts));
                }
            }
            if working[i].status.is_open() {
                i += 1;
            } else {
                working.remove(i);
            }
        }
        fills
    }

    /// Обновить стакан и доисполнить ставшие маркетабельными лимитки.
    pub fn on_book(&mut self, book: &OrderBook) -> Vec<Fill> {
        self.last_book = Some(book.clone());
        let Self {
            account, working, ..
        } = self;
        let mut fills = Vec::new();
        let mut i = 0;
        while i < working.len() {
            if working[i].kind == OrderType::Limit {
                let f = fill_against_book(account, &mut working[i], book, book.ts, false);
                fills.extend(f);
            }
            if working[i].status.is_open() {
                i += 1;
            } else {
                working.remove(i);
            }
        }
        fills
    }
}

/// Исполнить часть заявки и обновить счёт. Возвращает факт исполнения.
fn execute(account: &mut Account, order: &mut Order, qty: f64, price: f64, ts: i64) -> Fill {
    let realized = account.apply_fill(&order.symbol, order.side, qty, price);
    order.filled += qty;
    order.status = if order.remaining() <= f64::EPSILON {
        OrderStatus::Filled
    } else {
        OrderStatus::PartiallyFilled
    };
    Fill {
        order_id: order.id,
        ts,
        side: order.side,
        qty,
        price,
        realized_pnl: realized,
    }
}

/// Провести заявку по встречной стороне стакана. `market` — рыночная (берёт
/// любые уровни и добивает остаток по последнему); иначе лимитная (только
/// уровни не хуже её цены). Уровни идут «лучший первый», поэтому при первом
/// неприемлемом уровне обход прекращается.
fn fill_against_book(
    account: &mut Account,
    order: &mut Order,
    book: &OrderBook,
    ts: i64,
    market: bool,
) -> Vec<Fill> {
    let levels: &[BookLevel] = match order.side {
        Side::Buy => &book.asks,
        Side::Sell => &book.bids,
    };
    let mut fills = Vec::new();
    let mut last_price = None;
    for lvl in levels {
        if order.remaining() <= f64::EPSILON {
            break;
        }
        let acceptable = market
            || match order.side {
                Side::Buy => lvl.price <= order.price.unwrap_or(f64::INFINITY),
                Side::Sell => lvl.price >= order.price.unwrap_or(f64::NEG_INFINITY),
            };
        if !acceptable {
            break;
        }
        let q = order.remaining().min(lvl.size);
        if q > f64::EPSILON {
            fills.push(execute(account, order, q, lvl.price, ts));
            last_price = Some(lvl.price);
        }
    }
    // Рыночная заявка добивает остаток по последнему пройденному уровню.
    if market && order.remaining() > f64::EPSILON {
        if let Some(p) = last_price {
            fills.push(execute(account, order, order.remaining(), p, ts));
        }
    }
    fills
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> OrderBook {
        OrderBook {
            ts: 1,
            bids: vec![
                BookLevel { price: 99.0, size: 5.0 },
                BookLevel { price: 98.0, size: 10.0 },
            ],
            asks: vec![
                BookLevel { price: 100.0, size: 5.0 },
                BookLevel { price: 101.0, size: 10.0 },
            ],
        }
    }

    fn broker() -> SimBroker {
        let mut b = SimBroker::new(1_000_000.0, RiskLimits::default());
        b.on_book(&book());
        b
    }

    fn req(side: Side, qty: f64, kind: OrderType, price: Option<f64>) -> OrderRequest {
        OrderRequest {
            symbol: "SBER@MISX".into(),
            side,
            qty,
            kind,
            price,
            tif: TimeInForce::Gtc,
        }
    }

    #[test]
    fn market_buy_walks_the_book() {
        let mut b = broker();
        // Купить 8: 5 @100 + 3 @101.
        let (order, fills) = b
            .submit(req(Side::Buy, 8.0, OrderType::Market, None), 10)
            .unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(fills.len(), 2);
        assert!((fills[0].price - 100.0).abs() < 1e-9 && fills[0].qty == 5.0);
        assert!((fills[1].price - 101.0).abs() < 1e-9 && fills[1].qty == 3.0);
        assert_eq!(b.account().position("SBER@MISX").qty, 8.0);
    }

    #[test]
    fn market_without_book_is_rejected() {
        let mut b = SimBroker::new(1_000.0, RiskLimits::default());
        let err = b.submit(req(Side::Buy, 1.0, OrderType::Market, None), 1);
        assert_eq!(err, Err(RejectReason::NoLiquidity));
    }

    #[test]
    fn limit_buy_rests_then_fills_on_tape() {
        let mut b = broker();
        // Лимит на покупку по 97 (ниже рынка) — встаёт в книгу.
        let (order, fills) = b
            .submit(req(Side::Buy, 4.0, OrderType::Limit, Some(97.0)), 5)
            .unwrap();
        assert!(fills.is_empty());
        assert_eq!(order.status, OrderStatus::New);
        assert_eq!(b.working_orders().len(), 1);
        // Печать по 96 (≤97) исполняет лимитку.
        let fills = b.on_trade(&Trade {
            ts: 6,
            price: 96.0,
            size: 10.0,
            buyer_initiated: Some(false),
        });
        assert_eq!(fills.len(), 1);
        assert!((fills[0].price - 97.0).abs() < 1e-9); // по цене лимита
        assert!(b.working_orders().is_empty()); // полностью исполнена
        assert_eq!(b.account().position("SBER@MISX").qty, 4.0);
    }

    #[test]
    fn marketable_limit_fills_immediately() {
        let mut b = broker();
        // Лимит на покупку по 100.5 ≥ best ask 100 → исполняется сразу.
        let (order, fills) = b
            .submit(req(Side::Buy, 5.0, OrderType::Limit, Some(100.5)), 7)
            .unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(fills.len(), 1);
        assert!((fills[0].price - 100.0).abs() < 1e-9);
    }

    #[test]
    fn stop_sell_triggers_on_break() {
        let mut b = broker();
        let (order, fills) = b
            .submit(req(Side::Sell, 3.0, OrderType::Stop, Some(95.0)), 8)
            .unwrap();
        assert!(fills.is_empty());
        assert_eq!(order.status, OrderStatus::New);
        // Печать по 94 (≤95) пробивает стоп → рыночное исполнение по 94.
        let fills = b.on_trade(&Trade {
            ts: 9,
            price: 94.0,
            size: 5.0,
            buyer_initiated: Some(false),
        });
        assert_eq!(fills.len(), 1);
        assert!((fills[0].price - 94.0).abs() < 1e-9);
        assert_eq!(b.account().position("SBER@MISX").qty, -3.0);
    }

    #[test]
    fn cancel_removes_working_order() {
        let mut b = broker();
        let (order, _) = b
            .submit(req(Side::Buy, 4.0, OrderType::Limit, Some(97.0)), 5)
            .unwrap();
        assert!(b.cancel(order.id).is_ok());
        assert!(b.working_orders().is_empty());
        assert_eq!(b.cancel(order.id), Err(RejectReason::NotFound));
    }

    #[test]
    fn risk_limit_rejects_oversized() {
        let mut b = SimBroker::new(
            1_000.0,
            RiskLimits {
                max_order_qty: 2.0,
                max_position: 5.0,
            },
        );
        b.on_book(&book());
        assert_eq!(
            b.submit(req(Side::Buy, 3.0, OrderType::Market, None), 1),
            Err(RejectReason::MaxOrderQty)
        );
    }
}
