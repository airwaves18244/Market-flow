//! Денежные потоки: куда движутся деньги внутри инструмента.
//!
//! - **Net money flow** — Σ(положительный money flow) − Σ(отрицательный),
//!   где знак задаётся ростом/падением типичной цены бар-к-бару.
//! - **Money Flow Index (MFI)** — нормированный 0..100 осциллятор давления
//!   покупателей/продавцов (объёмный аналог RSI).
//! - **Cumulative Volume Delta (CVD)** — накопленная разница агрессивных
//!   покупок и продаж по ленте сделок.

use crate::model::{Bar, Trade};

/// Положительная и отрицательная компоненты денежного потока по серии баров.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MoneyFlow {
    pub positive: f64,
    pub negative: f64,
}

impl MoneyFlow {
    /// Чистый денежный поток: `positive - negative`.
    pub fn net(&self) -> f64 {
        self.positive - self.negative
    }
}

/// Накопить денежный поток: бар идёт в плюс, если его типичная цена выше, чем у
/// предыдущего бара, иначе в минус. Первый бар базовый и не учитывается.
pub fn money_flow(bars: &[Bar]) -> MoneyFlow {
    let mut mf = MoneyFlow::default();
    for w in bars.windows(2) {
        let prev = w[0].typical_price();
        let cur = &w[1];
        let raw = cur.turnover(); // typical * volume
        if cur.typical_price() > prev {
            mf.positive += raw;
        } else if cur.typical_price() < prev {
            mf.negative += raw;
        }
        // равенство типичных цен — money flow нулевой, пропускаем
    }
    mf
}

/// Money Flow Index за `period` (классически 14) по последним барам.
///
/// Возвращает `None`, если баров меньше `period + 1`. Диапазон 0..100:
/// >80 — перекупленность/сильный приток, <20 — перепроданность/отток.
pub fn money_flow_index(bars: &[Bar], period: usize) -> Option<f64> {
    if period == 0 || bars.len() < period + 1 {
        return None;
    }
    // Берём последние `period` переходов (т.е. period+1 баров с конца).
    let window = &bars[bars.len() - (period + 1)..];
    let mf = money_flow(window);
    if mf.negative == 0.0 {
        // Нет отрицательного потока — индекс упирается в максимум.
        return Some(100.0);
    }
    let money_ratio = mf.positive / mf.negative;
    Some(100.0 - (100.0 / (1.0 + money_ratio)))
}

/// Накопленная дельта объёма (CVD) по ленте сделок.
///
/// Покупки (`buyer_initiated == Some(true)`) прибавляют размер, продажи
/// (`Some(false)`) вычитают. Сделки без стороны (`None`) пропускаются.
pub fn cumulative_volume_delta(trades: &[Trade]) -> f64 {
    trades
        .iter()
        .map(|t| match t.buyer_initiated {
            Some(true) => t.size,
            Some(false) => -t.size,
            None => 0.0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(price: f64, volume: f64) -> Bar {
        // плоский бар: typical_price == price
        Bar {
            ts: 0,
            open: price,
            high: price,
            low: price,
            close: price,
            volume,
        }
    }

    #[test]
    fn money_flow_signs_by_typical_price() {
        let bars = [bar(10.0, 1.0), bar(11.0, 2.0), bar(9.0, 3.0)];
        let mf = money_flow(&bars);
        assert_eq!(mf.positive, 11.0 * 2.0); // рост к 11
        assert_eq!(mf.negative, 9.0 * 3.0); // падение к 9
        assert_eq!(mf.net(), 22.0 - 27.0);
    }

    #[test]
    fn mfi_all_up_is_100() {
        let bars: Vec<Bar> = (1..=15).map(|i| bar(i as f64, 1.0)).collect();
        assert_eq!(money_flow_index(&bars, 14), Some(100.0));
    }

    #[test]
    fn mfi_in_range_and_needs_history() {
        let bars = [bar(1.0, 1.0), bar(2.0, 1.0)];
        assert_eq!(money_flow_index(&bars, 14), None);

        // чередование вверх/вниз → MFI где-то между 0 и 100
        let mut bars = Vec::new();
        for i in 0..20 {
            bars.push(bar(if i % 2 == 0 { 10.0 } else { 11.0 }, 1.0));
        }
        let mfi = money_flow_index(&bars, 14).unwrap();
        assert!((0.0..=100.0).contains(&mfi));
    }

    #[test]
    fn cvd_nets_buys_and_sells() {
        let t = |size: f64, buy: Option<bool>| Trade {
            ts: 0,
            price: 100.0,
            size,
            buyer_initiated: buy,
        };
        let trades = [t(5.0, Some(true)), t(3.0, Some(false)), t(2.0, None)];
        assert_eq!(cumulative_volume_delta(&trades), 2.0);
    }
}
