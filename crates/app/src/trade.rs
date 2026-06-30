//! Сессия симулированной торговли: тонкая, потокобезопасная обёртка над
//! [`domain::trading::SimBroker`].
//!
//! Живёт в [`crate::state::AppState`] рядом с хранилищем. Команды Tauri ставят и
//! отменяют заявки через неё, а live-слой (стрим сделок/стакана) докручивает
//! исполнение лимиток/стопов через [`TradeSession::on_trade`]/[`on_book`].
//! Преобразование доменных типов в DTO — здесь же, чтобы команды оставались
//! тонкими.

use std::sync::Mutex;

use domain::trading::{OrderRequest, RiskLimits, SimBroker};
use domain::{OrderBook, Trade};

use crate::dto::{
    AccountDto, FillEventDto, OrderDto, OrderInput, PositionDto, SubmitResultDto,
};

/// Стартовая наличность симулятора по умолчанию.
const DEFAULT_CASH: f64 = 1_000_000.0;

/// Потокобезопасная сессия paper-трейдинга.
pub struct TradeSession {
    broker: Mutex<SimBroker>,
}

impl TradeSession {
    /// Сессия со стартовой наличностью и лимитами по умолчанию.
    pub fn new() -> Self {
        Self {
            broker: Mutex::new(SimBroker::new(DEFAULT_CASH, RiskLimits::default())),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, SimBroker> {
        // Отравленный мьютекс маловероятен (нет паник под локом); берём данные.
        self.broker.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Поставить заявку. `Err` — человекочитаемая причина отклонения/ошибки ввода.
    pub fn submit(&self, input: &OrderInput) -> Result<SubmitResultDto, String> {
        let side = input
            .parse_side()
            .ok_or_else(|| format!("неизвестная сторона: {}", input.side))?;
        let kind = input
            .parse_kind()
            .ok_or_else(|| format!("неизвестный тип заявки: {}", input.kind))?;
        let req = OrderRequest {
            symbol: input.symbol.clone(),
            side,
            qty: input.qty,
            kind,
            price: input.price,
            tif: input.parse_tif(),
        };
        let mut broker = self.lock();
        match broker.submit(req, unix_now()) {
            Ok((order, fills)) => Ok(SubmitResultDto {
                order: OrderDto::from(&order),
                fills: fills.iter().map(FillEventDto::from).collect(),
            }),
            Err(reason) => Err(reason.message().to_string()),
        }
    }

    /// Отменить активную заявку по id.
    pub fn cancel(&self, id: u64) -> Result<OrderDto, String> {
        self.lock()
            .cancel(id)
            .map(|o| OrderDto::from(&o))
            .map_err(|r| r.message().to_string())
    }

    /// Активные заявки (блоттер).
    pub fn orders(&self) -> Vec<OrderDto> {
        self.lock().working_orders().iter().map(OrderDto::from).collect()
    }

    /// Открытые позиции (ненулевые).
    pub fn positions(&self) -> Vec<PositionDto> {
        self.lock()
            .account()
            .positions
            .iter()
            .filter(|(_, p)| p.qty != 0.0)
            .map(|(sym, p)| PositionDto::new(sym, p))
            .collect()
    }

    /// Состояние счёта.
    pub fn account(&self) -> AccountDto {
        let acc = self.lock();
        let acc = acc.account();
        AccountDto {
            cash: acc.cash,
            realized_pnl: acc.realized_pnl,
        }
    }

    /// Прокинуть печать сделки в симулятор (исполнение лимиток/стопов).
    pub fn on_trade(&self, trade: &Trade) -> Vec<FillEventDto> {
        self.lock().on_trade(trade).iter().map(FillEventDto::from).collect()
    }

    /// Обновить стакан симулятора (исполнение ставших маркетабельными лимиток).
    pub fn on_book(&self, book: &OrderBook) -> Vec<FillEventDto> {
        self.lock().on_book(book).iter().map(FillEventDto::from).collect()
    }
}

impl Default for TradeSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Текущее время в UNIX-секундах UTC (для отметки исполнений).
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{BookLevel, OrderBook};

    fn seeded_session() -> TradeSession {
        let s = TradeSession::new();
        s.on_book(&OrderBook {
            ts: 1,
            bids: vec![BookLevel { price: 99.0, size: 10.0 }],
            asks: vec![BookLevel { price: 100.0, size: 10.0 }],
        });
        s
    }

    #[test]
    fn market_order_fills_and_updates_account() {
        let s = seeded_session();
        let res = s
            .submit(&OrderInput {
                symbol: "SBER@MISX".into(),
                side: "buy".into(),
                qty: 3.0,
                kind: "market".into(),
                price: None,
                tif: None,
            })
            .unwrap();
        assert_eq!(res.order.status, "filled");
        assert_eq!(res.fills.len(), 1);
        let positions = s.positions();
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].qty, 3.0);
        // наличность уменьшилась на 3*100
        assert!((s.account().cash - (1_000_000.0 - 300.0)).abs() < 1e-6);
    }

    #[test]
    fn limit_order_rests_in_blotter_and_cancels() {
        let s = seeded_session();
        let res = s
            .submit(&OrderInput {
                symbol: "SBER@MISX".into(),
                side: "buy".into(),
                qty: 2.0,
                kind: "limit".into(),
                price: Some(95.0),
                tif: None,
            })
            .unwrap();
        assert_eq!(s.orders().len(), 1);
        assert!(s.cancel(res.order.id).is_ok());
        assert!(s.orders().is_empty());
    }

    #[test]
    fn bad_side_is_rejected() {
        let s = seeded_session();
        let err = s.submit(&OrderInput {
            symbol: "X".into(),
            side: "hodl".into(),
            qty: 1.0,
            kind: "market".into(),
            price: None,
            tif: None,
        });
        assert!(err.is_err());
    }
}
