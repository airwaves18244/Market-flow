//! Super Candles (датасет ALGOPACK `tradestats`): расширенная 5-минутная свеча
//! с разбивкой потока на покупки/продажи, и аналитика поверх неё.
//!
//! Поля соответствуют колонкам `tradestats` MOEX ALGOPACK. Аналитика чистая:
//! агрегация 5-мин свечей в произвольный тайм-фрейм, VWAP-полоса, индекс
//! агрессии покупателей (buy-pressure из `disb`) и аномальный объём (z-score).

use serde::{Deserialize, Serialize};

use super::rolling_zscore;

/// Расширенная свеча Super Candles за 5-минутный интервал.
///
/// Суффиксы `_b`/`_s` — покупки (buy) и продажи (sell). `disb` — дисбаланс
/// потока (−1..1): доля перевеса покупок над продажами по объёму.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuperCandle {
    /// Идентификатор инструмента (SECID).
    pub secid: String,
    /// Метка времени начала интервала (UNIX-секунды UTC).
    pub ts: i64,
    pub pr_open: f64,
    pub pr_high: f64,
    pub pr_low: f64,
    pub pr_close: f64,
    /// Стандартное отклонение цены внутри интервала.
    pub pr_std: f64,
    /// Объём (в штуках/контрактах).
    pub vol: f64,
    /// Оборот (денежный объём).
    pub val: f64,
    /// Число сделок.
    pub trades: f64,
    /// VWAP за интервал.
    pub pr_vwap: f64,
    /// Изменение цены за интервал (доля).
    pub pr_change: f64,
    /// Объём покупок/продаж.
    pub vol_b: f64,
    pub vol_s: f64,
    /// Оборот покупок/продаж.
    pub val_b: f64,
    pub val_s: f64,
    /// Сделки покупок/продаж.
    pub trades_b: f64,
    pub trades_s: f64,
    /// Дисбаланс потока (−1..1).
    pub disb: f64,
    /// VWAP покупок/продаж.
    pub pr_vwap_b: f64,
    pub pr_vwap_s: f64,
}

impl SuperCandle {
    /// Индекс агрессии покупателей (buy-pressure) — доля объёма покупок в общем
    /// объёме (0..1). При нулевом объёме возвращает `0.5` (нейтрально).
    pub fn buy_pressure(&self) -> f64 {
        let total = self.vol_b + self.vol_s;
        if total <= 0.0 {
            0.5
        } else {
            self.vol_b / total
        }
    }
}

/// Полоса VWAP: середина и границы `±k·σ` от VWAP по серии свечей.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VwapBand {
    pub mid: f64,
    pub upper: f64,
    pub lower: f64,
}

/// VWAP-полоса для свечи: центр — `pr_vwap`, ширина — `k` стандартных отклонений
/// цены (`pr_std`).
pub fn vwap_band(candle: &SuperCandle, k: f64) -> VwapBand {
    VwapBand {
        mid: candle.pr_vwap,
        upper: candle.pr_vwap + k * candle.pr_std,
        lower: candle.pr_vwap - k * candle.pr_std,
    }
}

/// Агрегировать последовательность 5-минутных свечей в свечи укрупнённого
/// тайм-фрейма по `group` исходных свечей в одну.
///
/// OHLC берётся из границ группы, объёмы/обороты/сделки суммируются, VWAP
/// пересчитывается как `Σval/Σvol`, дисбаланс — из суммарных покупок/продаж.
/// Свечи должны быть одного инструмента и отсортированы по времени; неполная
/// последняя группа агрегируется как есть.
pub fn aggregate(candles: &[SuperCandle], group: usize) -> Vec<SuperCandle> {
    assert!(group > 0, "group must be > 0");
    candles.chunks(group).filter_map(fold_group).collect()
}

fn fold_group(chunk: &[SuperCandle]) -> Option<SuperCandle> {
    let first = chunk.first()?;
    let last = chunk.last()?;
    let mut high = f64::NEG_INFINITY;
    let mut low = f64::INFINITY;
    let (mut vol, mut val, mut trades) = (0.0, 0.0, 0.0);
    let (mut vol_b, mut vol_s) = (0.0, 0.0);
    let (mut val_b, mut val_s) = (0.0, 0.0);
    let (mut trades_b, mut trades_s) = (0.0, 0.0);
    for c in chunk {
        high = high.max(c.pr_high);
        low = low.min(c.pr_low);
        vol += c.vol;
        val += c.val;
        trades += c.trades;
        vol_b += c.vol_b;
        vol_s += c.vol_s;
        val_b += c.val_b;
        val_s += c.val_s;
        trades_b += c.trades_b;
        trades_s += c.trades_s;
    }
    let vwap = if vol > 0.0 { val / vol } else { first.pr_vwap };
    let vwap_b = if vol_b > 0.0 { val_b / vol_b } else { 0.0 };
    let vwap_s = if vol_s > 0.0 { val_s / vol_s } else { 0.0 };
    let total_flow = vol_b + vol_s;
    let disb = if total_flow > 0.0 {
        (vol_b - vol_s) / total_flow
    } else {
        0.0
    };
    let pr_change = if first.pr_open != 0.0 {
        (last.pr_close - first.pr_open) / first.pr_open
    } else {
        0.0
    };
    // Стандартное отклонение цены группы оцениваем как RMS внутригрупповых σ.
    let pr_std =
        (chunk.iter().map(|c| c.pr_std * c.pr_std).sum::<f64>() / chunk.len() as f64).sqrt();
    Some(SuperCandle {
        secid: first.secid.clone(),
        ts: first.ts,
        pr_open: first.pr_open,
        pr_high: high,
        pr_low: low,
        pr_close: last.pr_close,
        pr_std,
        vol,
        val,
        trades,
        pr_vwap: vwap,
        pr_change,
        vol_b,
        vol_s,
        val_b,
        val_s,
        trades_b,
        trades_s,
        disb,
        pr_vwap_b: vwap_b,
        pr_vwap_s: vwap_s,
    })
}

/// z-score объёма свечи `idx` относительно предыдущих `window` свечей.
/// `None`, если истории недостаточно или объём постоянен.
pub fn volume_zscore(candles: &[SuperCandle], idx: usize, window: usize) -> Option<f64> {
    let vols: Vec<f64> = candles.iter().map(|c| c.vol).collect();
    rolling_zscore(&vols, idx, window)
}

/// Аномальный объём: индексы свечей, где z-score объёма ≥ `threshold`.
pub fn anomalous_volume(candles: &[SuperCandle], window: usize, threshold: f64) -> Vec<usize> {
    (0..candles.len())
        .filter(|&i| volume_zscore(candles, i, window).is_some_and(|z| z >= threshold))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn candle(ts: i64, o: f64, h: f64, l: f64, c: f64, vol: f64, vb: f64, vs: f64) -> SuperCandle {
        let val = c * vol;
        SuperCandle {
            secid: "SBER".into(),
            ts,
            pr_open: o,
            pr_high: h,
            pr_low: l,
            pr_close: c,
            pr_std: 0.5,
            vol,
            val,
            trades: 100.0,
            pr_vwap: c,
            pr_change: 0.0,
            vol_b: vb,
            vol_s: vs,
            val_b: c * vb,
            val_s: c * vs,
            trades_b: 60.0,
            trades_s: 40.0,
            disb: (vb - vs) / (vb + vs),
            pr_vwap_b: c,
            pr_vwap_s: c,
        }
    }

    #[test]
    fn buy_pressure_ratio() {
        let c = candle(0, 100.0, 101.0, 99.0, 100.0, 100.0, 70.0, 30.0);
        assert!((c.buy_pressure() - 0.7).abs() < 1e-12);
    }

    #[test]
    fn buy_pressure_neutral_on_zero_volume() {
        let c = candle(0, 100.0, 100.0, 100.0, 100.0, 0.0, 0.0, 0.0);
        assert!((c.buy_pressure() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn vwap_band_widths() {
        let c = candle(0, 100.0, 101.0, 99.0, 100.0, 100.0, 50.0, 50.0);
        let b = vwap_band(&c, 2.0);
        assert!((b.upper - 101.0).abs() < 1e-12); // 100 + 2*0.5
        assert!((b.lower - 99.0).abs() < 1e-12);
    }

    #[test]
    fn aggregate_two_candles() {
        let cs = vec![
            candle(0, 100.0, 105.0, 98.0, 102.0, 100.0, 60.0, 40.0),
            candle(300, 102.0, 110.0, 101.0, 108.0, 200.0, 150.0, 50.0),
        ];
        let agg = aggregate(&cs, 2);
        assert_eq!(agg.len(), 1);
        let a = &agg[0];
        assert_eq!(a.ts, 0);
        assert_eq!(a.pr_open, 100.0);
        assert_eq!(a.pr_close, 108.0);
        assert_eq!(a.pr_high, 110.0);
        assert_eq!(a.pr_low, 98.0);
        assert_eq!(a.vol, 300.0);
        // VWAP = Σval/Σvol.
        let expected_vwap = (102.0 * 100.0 + 108.0 * 200.0) / 300.0;
        assert!((a.pr_vwap - expected_vwap).abs() < 1e-9);
        // disb = (210 − 90)/300 = 0.4.
        assert!((a.disb - 0.4).abs() < 1e-12);
    }

    #[test]
    fn aggregate_leaves_partial_last_group() {
        let cs = vec![
            candle(0, 100.0, 101.0, 99.0, 100.0, 10.0, 5.0, 5.0),
            candle(300, 100.0, 101.0, 99.0, 100.0, 10.0, 5.0, 5.0),
            candle(600, 100.0, 101.0, 99.0, 100.0, 10.0, 5.0, 5.0),
        ];
        let agg = aggregate(&cs, 2);
        assert_eq!(agg.len(), 2); // 2 + 1
        assert_eq!(agg[1].vol, 10.0);
    }

    #[test]
    fn anomalous_volume_flags_spike() {
        let mut cs = Vec::new();
        // Слегка варьирующийся базовый объём (ненулевая дисперсия).
        for (i, &v) in [95.0, 105.0, 100.0, 98.0, 102.0].iter().enumerate() {
            cs.push(candle(
                i as i64 * 300,
                100.0,
                101.0,
                99.0,
                100.0,
                v,
                v / 2.0,
                v / 2.0,
            ));
        }
        cs.push(candle(1500, 100.0, 101.0, 99.0, 100.0, 500.0, 250.0, 250.0));
        let flagged = anomalous_volume(&cs, 5, 3.0);
        assert_eq!(flagged, vec![5]);
    }
}
