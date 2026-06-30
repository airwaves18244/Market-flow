//! Footprint / дельта по ленте сделок.
//!
//! Footprint раскладывает объём бара по ценовым уровням на «по биду» (агрессор —
//! продавец) и «по аску» (агрессор — покупатель). Дельта бара = Σ ask − Σ bid;
//! накопленная дельта (CVD) — нарастающий итог по барам.
//!
//! Сторона-агрессор берётся из [`Trade::buyer_initiated`]; сделки без стороны
//! (`None`) в footprint не учитываются (как и в [`crate::metrics::flow`] CVD).

use std::collections::BTreeMap;

use crate::metrics::flow::cumulative_volume_delta;
use crate::model::Trade;

/// Ячейка footprint: объём на одном ценовом уровне по сторонам агрессора.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FootprintCell {
    pub price: f64,
    /// Объём, исполненный по биду (агрессор-продавец).
    pub bid_volume: f64,
    /// Объём, исполненный по аску (агрессор-покупатель).
    pub ask_volume: f64,
}

impl FootprintCell {
    /// Дельта уровня: `ask − bid`.
    pub fn delta(&self) -> f64 {
        self.ask_volume - self.bid_volume
    }
}

/// Footprint одного бара: ячейки по ценам + агрегаты.
#[derive(Debug, Clone, PartialEq)]
pub struct FootprintBar {
    /// Время начала бара, UNIX-секунды UTC.
    pub ts: i64,
    /// Ячейки по возрастанию цены.
    pub cells: Vec<FootprintCell>,
    pub bid_total: f64,
    pub ask_total: f64,
    /// Дельта бара: `ask_total − bid_total`.
    pub delta: f64,
    /// Накопленная дельта (CVD) к концу этого бара включительно.
    pub cumulative_delta: f64,
}

/// Округлить цену к ближайшему шагу `tick` (бакет ценового уровня).
fn bucket(price: f64, tick: f64) -> f64 {
    if tick <= 0.0 {
        price
    } else {
        (price / tick).round() * tick
    }
}

/// Построить footprint по барам.
///
/// `bar_starts` — времена начала баров (по возрастанию), каждый бар покрывает
/// `[start, start + bar_seconds)`. Сделки распределяются по барам и ценовым
/// уровням (шаг `tick_size`). Бары без сделок дают пустые ячейки и нулевую
/// дельту, но сохраняются (выравнивание с ценовым графиком).
pub fn footprint(
    trades: &[Trade],
    bar_starts: &[i64],
    bar_seconds: i64,
    tick_size: f64,
) -> Vec<FootprintBar> {
    let mut out = Vec::with_capacity(bar_starts.len());
    let mut cum = 0.0;

    for &start in bar_starts {
        let end = start + bar_seconds;
        // price-bucket → (bid, ask)
        let mut levels: BTreeMap<i64, (f64, f64)> = BTreeMap::new();
        let mut in_bar: Vec<Trade> = Vec::new();

        for t in trades {
            if t.ts >= start && t.ts < end {
                in_bar.push(*t);
                let p = bucket(t.price, tick_size);
                // ключ бакета как целое в шагах тика, чтобы f64 не ломал порядок
                let key = if tick_size > 0.0 {
                    (p / tick_size).round() as i64
                } else {
                    p.to_bits() as i64
                };
                let cell = levels.entry(key).or_insert((0.0, 0.0));
                match t.buyer_initiated {
                    Some(true) => cell.1 += t.size,  // ask (покупка-агрессор)
                    Some(false) => cell.0 += t.size, // bid (продажа-агрессор)
                    None => {}
                }
            }
        }

        let cells: Vec<FootprintCell> = levels
            .into_iter()
            .map(|(key, (bid, ask))| FootprintCell {
                price: if tick_size > 0.0 {
                    key as f64 * tick_size
                } else {
                    f64::from_bits(key as u64)
                },
                bid_volume: bid,
                ask_volume: ask,
            })
            .collect();

        let bid_total: f64 = cells.iter().map(|c| c.bid_volume).sum();
        let ask_total: f64 = cells.iter().map(|c| c.ask_volume).sum();
        // Дельта бара = CVD по его сделкам (переиспользуем metrics::flow).
        let delta = cumulative_volume_delta(&in_bar);
        cum += delta;

        out.push(FootprintBar {
            ts: start,
            cells,
            bid_total,
            ask_total,
            delta,
            cumulative_delta: cum,
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(ts: i64, price: f64, size: f64, buy: Option<bool>) -> Trade {
        Trade {
            ts,
            price,
            size,
            buyer_initiated: buy,
        }
    }

    #[test]
    fn splits_volume_by_aggressor_and_price() {
        // Один бар [0,60): покупка 5 @100, продажа 3 @100, покупка 2 @101.
        let trades = [
            t(1, 100.0, 5.0, Some(true)),
            t(2, 100.0, 3.0, Some(false)),
            t(3, 101.0, 2.0, Some(true)),
        ];
        let fp = footprint(&trades, &[0], 60, 1.0);
        assert_eq!(fp.len(), 1);
        let bar = &fp[0];
        assert_eq!(bar.cells.len(), 2); // уровни 100 и 101
        let c100 = bar.cells.iter().find(|c| c.price == 100.0).unwrap();
        assert_eq!(c100.ask_volume, 5.0);
        assert_eq!(c100.bid_volume, 3.0);
        assert_eq!(c100.delta(), 2.0);
        // Дельта бара = (5+2) − 3 = 4
        assert_eq!(bar.delta, 4.0);
        assert_eq!(bar.cumulative_delta, 4.0);
    }

    #[test]
    fn cumulative_delta_runs_across_bars() {
        // Бар0 [0,60): дельта +4; бар1 [60,120): дельта −1.
        let trades = [
            t(1, 100.0, 4.0, Some(true)),
            t(70, 100.0, 1.0, Some(false)),
        ];
        let fp = footprint(&trades, &[0, 60], 60, 1.0);
        assert_eq!(fp[0].cumulative_delta, 4.0);
        assert_eq!(fp[1].delta, -1.0);
        assert_eq!(fp[1].cumulative_delta, 3.0);
    }

    #[test]
    fn empty_bars_are_kept_with_zero_delta() {
        let fp = footprint(&[], &[0, 60], 60, 1.0);
        assert_eq!(fp.len(), 2);
        assert!(fp.iter().all(|b| b.delta == 0.0 && b.cells.is_empty()));
    }

    #[test]
    fn trades_bucket_to_nearest_tick() {
        // tick=0.5: 100.24→100.0, 100.26→100.5
        let trades = [
            t(1, 100.24, 1.0, Some(true)),
            t(2, 100.26, 1.0, Some(true)),
        ];
        let fp = footprint(&trades, &[0], 60, 0.5);
        let prices: Vec<f64> = fp[0].cells.iter().map(|c| c.price).collect();
        assert_eq!(prices, vec![100.0, 100.5]);
    }
}
