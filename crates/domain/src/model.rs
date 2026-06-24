//! Базовые доменные типы.
//!
//! Денежные величины и объёмы хранятся как `f64` (рыночная аналитика — это
//! статистика и доли, а не бухгалтерия копейка-в-копейку). Время — UNIX-секунды
//! UTC (`i64`), как отдаёт Finam Trade API.

use serde::{Deserialize, Serialize};

/// Класс актива. Соответствует четырём представлениям терминала плюс
/// агрегирующее «сумма всех».
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetClass {
    /// Акции (MISX и пр.).
    Equity,
    /// Фьючерсы (FORTS / RTSX).
    Future,
    /// Облигации (ОФЗ, корпоративные).
    Bond,
}

impl AssetClass {
    /// Все классы активов в стабильном порядке (для итерации в дашборде).
    pub const ALL: [AssetClass; 3] = [AssetClass::Equity, AssetClass::Future, AssetClass::Bond];

    /// Короткий машинный код класса.
    pub fn code(self) -> &'static str {
        match self {
            AssetClass::Equity => "equity",
            AssetClass::Future => "future",
            AssetClass::Bond => "bond",
        }
    }
}

/// Описание торгового инструмента (из `AssetsService`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Instrument {
    /// Стабильный идентификатор `ticker@mic`, напр. `SBER@MISX`.
    pub symbol: String,
    /// Тикер, напр. `SBER`.
    pub ticker: String,
    /// Человекочитаемое имя.
    pub name: String,
    /// Класс актива.
    pub asset_class: AssetClass,
    /// Сектор (для акций/облигаций). Заполняется из таблицы классификации;
    /// у фьючерсов обычно `None`.
    pub sector: Option<String>,
    /// Размер лота.
    pub lot_size: u32,
    /// ISIN, если есть.
    pub isin: Option<String>,
}

/// Свеча (бар) котировок за период.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bar {
    /// Время начала бара, UNIX-секунды UTC.
    pub ts: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    /// Объём в штуках (или контрактах) за бар.
    pub volume: f64,
}

impl Bar {
    /// Типичная цена `(H + L + C) / 3` — основа для оборота и money flow.
    pub fn typical_price(&self) -> f64 {
        (self.high + self.low + self.close) / 3.0
    }

    /// Денежный оборот бара ≈ типичная цена × объём.
    pub fn turnover(&self) -> f64 {
        self.typical_price() * self.volume
    }

    /// Изменение за бар: `close - open`. Знак задаёт направление потока.
    pub fn change(&self) -> f64 {
        self.close - self.open
    }
}

/// Снимок лучшей цены (из `LastQuote`/`SubscribeQuote`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub ts: i64,
    pub last: f64,
    pub bid: f64,
    pub ask: f64,
    /// Накопленный дневной объём, если предоставлен.
    pub volume: f64,
}

/// Обезличенная сделка (из `LatestTrades`/`SubscribeLatestTrades`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    pub ts: i64,
    pub price: f64,
    pub size: f64,
    /// Сторона-инициатор, если биржа отдаёт. `true` — покупка (агрессор-бид).
    pub buyer_initiated: Option<bool>,
}

impl Trade {
    /// Денежный оборот сделки.
    pub fn turnover(&self) -> f64 {
        self.price * self.size
    }
}
