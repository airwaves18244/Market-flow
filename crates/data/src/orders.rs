//! Роутинг заявок: куда уходит заявка на исполнение.
//!
//! Контракт [`OrderRouter`] отделяет «куда исполнять» от «как принимать заявки»
//! в `app`. По умолчанию используется [`SimOrderRouter`] — обёртка над
//! симулятором `domain::trading::SimBroker` (paper trading, V2). Боевой роутер
//! [`FinamOrderRouter`] (живой gRPC `OrderService`/`AccountsService`) — каркас
//! за фичей `live-trading`, по умолчанию не компилируется.
//!
//! Слой симметричен `MarketData`: реализации могут быть сетевыми, но контракт
//! чистый и тестируется на симуляторе без сети.

use domain::trading::{Fill, Order, OrderRequest, RejectReason, RiskLimits, SimBroker};
use domain::{OrderBook, Trade};

/// Ошибка роутинга заявки.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RouterError {
    /// Заявка отклонена (риск/нет ликвидности/ошибка ввода).
    #[error("заявка отклонена: {0}")]
    Rejected(&'static str),
    /// Роутер не подключён к источнику исполнения (живой режим).
    #[error("роутер не подключён")]
    NotConnected,
    /// Возможность ещё не реализована (заглушка боевого роутера).
    #[error("не реализовано")]
    Unimplemented,
}

impl From<RejectReason> for RouterError {
    fn from(r: RejectReason) -> Self {
        RouterError::Rejected(r.message())
    }
}

/// Куда уходит заявка на исполнение. Реализации: симулятор (по умолчанию) и
/// боевой Finam (за фичей).
pub trait OrderRouter {
    /// Поставить заявку. Возвращает её итоговое состояние и исполнения.
    fn submit(&mut self, req: OrderRequest) -> Result<(Order, Vec<Fill>), RouterError>;

    /// Отменить активную заявку по id.
    fn cancel(&mut self, id: u64) -> Result<Order, RouterError>;
}

/// Роутер по умолчанию: симулятор исполнения (paper trading).
///
/// Оборачивает чистый `SimBroker`. Рыночные данные (стакан/лента) докручивают
/// исполнение лимиток/стопов через [`SimOrderRouter::on_book`]/[`on_trade`].
pub struct SimOrderRouter {
    broker: SimBroker,
}

impl SimOrderRouter {
    /// Новый симуляторный роутер со стартовой наличностью и лимитами риска.
    pub fn new(initial_cash: f64, limits: RiskLimits) -> Self {
        Self {
            broker: SimBroker::new(initial_cash, limits),
        }
    }

    /// Доступ к симулятору (счёт/заявки) для запросов состояния.
    pub fn broker(&self) -> &SimBroker {
        &self.broker
    }

    /// Прокинуть снимок стакана (исполнение ставших маркетабельными лимиток).
    pub fn on_book(&mut self, book: &OrderBook) -> Vec<Fill> {
        self.broker.on_book(book)
    }

    /// Прокинуть печать сделки (исполнение лимиток/стопов на ленте).
    pub fn on_trade(&mut self, trade: &Trade) -> Vec<Fill> {
        self.broker.on_trade(trade)
    }
}

impl OrderRouter for SimOrderRouter {
    fn submit(&mut self, req: OrderRequest) -> Result<(Order, Vec<Fill>), RouterError> {
        // Время исполнения — текущее (для отметки сделок симулятора).
        let ts = unix_now();
        self.broker.submit(req, ts).map_err(RouterError::from)
    }

    fn cancel(&mut self, id: u64) -> Result<Order, RouterError> {
        self.broker.cancel(id).map_err(RouterError::from)
    }
}

/// Текущее время в UNIX-секундах UTC.
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Боевой роутер Finam — каркас (фича `live-trading`).
///
/// Здесь подключится живой gRPC `OrderService` (постановка/отмена) и
/// `AccountsService` (позиции/сделки) по тем же доменным типам заявок. Сейчас
/// это контрактная заглушка: методы возвращают [`RouterError::Unimplemented`],
/// чтобы зафиксировать интерфейс до интеграции транспорта (vendored `.proto` и
/// клиентские стабы добавляются вместе с реализацией).
#[cfg(feature = "live-trading")]
pub struct FinamOrderRouter {
    _private: (),
}

#[cfg(feature = "live-trading")]
impl FinamOrderRouter {
    /// Создать (ещё не подключённый) боевой роутер.
    pub fn new() -> Self {
        Self { _private: () }
    }
}

#[cfg(feature = "live-trading")]
impl Default for FinamOrderRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "live-trading")]
impl OrderRouter for FinamOrderRouter {
    fn submit(&mut self, _req: OrderRequest) -> Result<(Order, Vec<Fill>), RouterError> {
        Err(RouterError::Unimplemented)
    }

    fn cancel(&mut self, _id: u64) -> Result<Order, RouterError> {
        Err(RouterError::Unimplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::trading::{OrderType, TimeInForce};
    use domain::{BookLevel, OrderBook, Side};

    fn sim() -> SimOrderRouter {
        let mut r = SimOrderRouter::new(1_000_000.0, RiskLimits::default());
        r.on_book(&OrderBook {
            ts: 1,
            bids: vec![BookLevel { price: 99.0, size: 10.0 }],
            asks: vec![BookLevel { price: 100.0, size: 10.0 }],
        });
        r
    }

    #[test]
    fn sim_router_submits_and_fills_market() {
        let mut r = sim();
        let (order, fills) = r
            .submit(OrderRequest {
                symbol: "SBER@MISX".into(),
                side: Side::Buy,
                qty: 3.0,
                kind: OrderType::Market,
                price: None,
                tif: TimeInForce::Gtc,
            })
            .unwrap();
        assert_eq!(fills.len(), 1);
        assert_eq!(order.filled, 3.0);
        assert_eq!(r.broker().account().position("SBER@MISX").qty, 3.0);
    }

    #[test]
    fn sim_router_maps_reject_reason() {
        let mut r = SimOrderRouter::new(1_000.0, RiskLimits::default());
        // Нет стакана → рыночная заявка отклоняется.
        let err = r.submit(OrderRequest {
            symbol: "X".into(),
            side: Side::Buy,
            qty: 1.0,
            kind: OrderType::Market,
            price: None,
            tif: TimeInForce::Gtc,
        });
        assert!(matches!(err, Err(RouterError::Rejected(_))));
    }
}
