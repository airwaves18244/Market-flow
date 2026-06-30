//! Опционы: ценообразование, греки, подразумеваемая волатильность, модели
//! улыбки и конструктор стратегий (фаза 12).
//!
//! Весь модуль — чистая математика без сети, БД и UI: типы и функции
//! детерминированы и покрыты юнит-тестами (известные эталоны, паритет
//! put-call, согласованность греков с конечными разностями). Сетевой слой
//! (опционная доска MOEX) и IPC подключаются в `data`/`app`.
//!
//! Подмодули:
//! - [`pricing`] — модели цены опциона: Блэк-76 и Башелье (нормальная),
//!   аналитические греки, решатель IV;
//! - [`smile`] — модели улыбки волатильности: MOEX-параметрическая, SABR
//!   (Hagan), SVI (raw, Gatheral), Каленкович; общий калибратор;
//! - [`strategy`] — конструктор опционных стратегий: ноги, шаблоны, payoff,
//!   агрегированные греки, точки безубытка, max profit/loss.

pub mod pricing;
pub mod smile;
pub mod strategy;

pub use pricing::{greeks, implied_vol, price, Greeks, OptionType, PriceInputs, PriceModel};
pub use smile::{KalenkovichSmile, MoexSmile, SabrParams, SmileModel, SmilePoint, SviParams};
pub use strategy::{Leg, LegKind, Side, Strategy, StrategyResult};
