//! Симулятор исполнения (CQG-подобный paper trading): заявки, исполнение по
//! стакану/ленте, позиции и P&L.
//!
//! - [`order`] — типы заявок (тип/TIF/состояние) и факт исполнения;
//! - [`account`] — счёт и позиции (средняя цена, реализованный/нереализованный P&L);
//! - [`risk`] — предторговые проверки (лимиты объёма/позиции);
//! - [`sim`] — движок `SimBroker`: приём заявок и матчинг.
//!
//! Всё чисто и тестируемо без сети. Живой роутинг в Finam подключается за
//! отдельной фичей в слое `data` (тот же контракт заявок).

pub mod account;
pub mod order;
pub mod risk;
pub mod sim;

pub use account::{Account, Position};
pub use order::{Fill, Order, OrderStatus, OrderType, TimeInForce};
pub use risk::{RejectReason, RiskLimits};
pub use sim::{OrderRequest, SimBroker};
