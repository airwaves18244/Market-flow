//! Базовые доменные типы.
//!
//! Денежные величины и объёмы хранятся как `f64` (рыночная аналитика — это
//! статистика и доли, а не бухгалтерия копейка-в-копейку). Время — UNIX-секунды
//! UTC (`i64`), как отдаёт Finam Trade API.

use serde::{Deserialize, Serialize};

/// Класс актива. Соответствует представлениям терминала плюс агрегирующее
/// «сумма всех».
///
/// `Fx` (валютный спот: USD/RUB, CNY/RUB, EUR/RUB) добавлен для кросс-актив
/// анализа потоков (вкладка «Сводка» / Summary): «куда уходят большие деньги».
/// Ингест FX-инструментов из Finam (`MarketDataService`, борд `CETS`) — задача
/// слоя `data` (см. `ROADMAP.md`); доменный тип готов уже сейчас, поэтому
/// аналитика и DTO считают FX наравне с остальными классами.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetClass {
    /// Акции (MISX и пр.).
    Equity,
    /// Фьючерсы (FORTS / RTSX).
    Future,
    /// Облигации (ОФЗ, корпоративные).
    Bond,
    /// Валютный спот (FX): USD/RUB, CNY/RUB, EUR/RUB (борд `CETS`).
    Fx,
}

impl AssetClass {
    /// Все классы активов в стабильном порядке (для итерации в дашборде).
    pub const ALL: [AssetClass; 4] = [
        AssetClass::Equity,
        AssetClass::Future,
        AssetClass::Bond,
        AssetClass::Fx,
    ];

    /// Короткий машинный код класса.
    pub fn code(self) -> &'static str {
        match self {
            AssetClass::Equity => "equity",
            AssetClass::Future => "future",
            AssetClass::Bond => "bond",
            AssetClass::Fx => "fx",
        }
    }

    /// Разобрать класс актива из кода (`equity|future|bond|fx`).
    pub fn from_code(code: &str) -> Option<AssetClass> {
        Some(match code {
            "equity" => AssetClass::Equity,
            "future" => AssetClass::Future,
            "bond" => AssetClass::Bond,
            "fx" => AssetClass::Fx,
            _ => return None,
        })
    }
}

/// Тайм-фрейм бара. Соответствует `TimeFrame` в Finam Trade API.
///
/// Это чистый доменный тип: код для хранения (`code`/`from_code`, колонка
/// `bars.timeframe`) и шаг в секундах (`seconds`) для бэкфилла и разбивки
/// исторических диапазонов. Адаптер `data` переэкспортирует его как
/// `data::TimeFrame`, чтобы сетевой слой и хранилище говорили на одном типе.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeFrame {
    M1,
    M5,
    M15,
    H1,
    D1,
}

impl TimeFrame {
    /// Все тайм-фреймы в порядке возрастания периода.
    pub const ALL: [TimeFrame; 5] = [
        TimeFrame::M1,
        TimeFrame::M5,
        TimeFrame::M15,
        TimeFrame::H1,
        TimeFrame::D1,
    ];

    /// Короткий машинный код (значение колонки `bars.timeframe`).
    pub fn code(self) -> &'static str {
        match self {
            TimeFrame::M1 => "m1",
            TimeFrame::M5 => "m5",
            TimeFrame::M15 => "m15",
            TimeFrame::H1 => "h1",
            TimeFrame::D1 => "d1",
        }
    }

    /// Разобрать тайм-фрейм из кода (`m1|m5|m15|h1|d1`).
    pub fn from_code(code: &str) -> Option<TimeFrame> {
        Some(match code {
            "m1" => TimeFrame::M1,
            "m5" => TimeFrame::M5,
            "m15" => TimeFrame::M15,
            "h1" => TimeFrame::H1,
            "d1" => TimeFrame::D1,
            _ => return None,
        })
    }

    /// Длительность одного бара в секундах. Дневной бар считаем равным
    /// календарным суткам (86 400 с) — этого достаточно для планирования
    /// бэкфилла и пагинации запросов.
    pub fn seconds(self) -> i64 {
        match self {
            TimeFrame::M1 => 60,
            TimeFrame::M5 => 5 * 60,
            TimeFrame::M15 => 15 * 60,
            TimeFrame::H1 => 60 * 60,
            TimeFrame::D1 => 24 * 60 * 60,
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

/// Уровень стакана: цена и совокупный объём на ней.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BookLevel {
    pub price: f64,
    pub size: f64,
}

/// Снимок стакана (DOM, из `OrderBook`/`SubscribeOrderBook`).
///
/// `bids` отсортированы по убыванию цены (лучший бид — первый), `asks` — по
/// возрастанию (лучший аск — первый).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBook {
    /// Момент снимка, UNIX-секунды UTC.
    pub ts: i64,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
}

impl OrderBook {
    /// Лучший бид (наивысшая цена покупки), если есть.
    pub fn best_bid(&self) -> Option<&BookLevel> {
        self.bids.first()
    }

    /// Лучший аск (наименьшая цена продажи), если есть.
    pub fn best_ask(&self) -> Option<&BookLevel> {
        self.asks.first()
    }

    /// Спред «лучший аск − лучший бид», если обе стороны присутствуют.
    pub fn spread(&self) -> Option<f64> {
        Some(self.best_ask()?.price - self.best_bid()?.price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_class_code_roundtrips() {
        for ac in AssetClass::ALL {
            assert_eq!(AssetClass::from_code(ac.code()), Some(ac));
        }
        assert_eq!(AssetClass::from_code("bogus"), None);
    }

    #[test]
    fn timeframe_code_roundtrips() {
        for tf in TimeFrame::ALL {
            assert_eq!(TimeFrame::from_code(tf.code()), Some(tf));
        }
        assert_eq!(TimeFrame::from_code("bogus"), None);
    }

    #[test]
    fn timeframe_seconds_are_ordered() {
        assert_eq!(TimeFrame::M1.seconds(), 60);
        assert_eq!(TimeFrame::H1.seconds(), 3600);
        assert_eq!(TimeFrame::D1.seconds(), 86_400);
        // период строго возрастает по списку ALL
        let secs: Vec<i64> = TimeFrame::ALL.iter().map(|t| t.seconds()).collect();
        assert!(secs.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn bar_turnover_uses_typical_price() {
        let b = Bar {
            ts: 0,
            open: 9.0,
            high: 11.0,
            low: 10.0,
            close: 12.0,
            volume: 2.0,
        };
        // typical = (11 + 10 + 12) / 3 = 11; turnover = 11 * 2 = 22
        assert!((b.turnover() - 22.0).abs() < 1e-12);
    }
}
