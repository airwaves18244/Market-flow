//! Рыночные метрики: как из баров/котировок получить выводы о деньгах.
//!
//! Каждый подмодуль — независимая, чисто функциональная единица с юнит-тестами:
//! - [`turnover`] — оборот (денежный объём) и направленный оборот;
//! - [`flow`] — денежные потоки: net money flow, Money Flow Index (MFI), CVD;
//! - [`breadth`] — ширина рынка (растущие/падающие, A/D);
//! - [`sector`] — роллапы метрик по секторам;
//! - [`rrg`] — секторная ротация в стиле RRG (RS-Ratio / RS-Momentum);
//! - [`crossasset`] — доли оборота по классам активов и матрица перетоков.

pub mod breadth;
pub mod crossasset;
pub mod flow;
pub mod rrg;
pub mod sector;
pub mod turnover;

/// Простая скользящая средняя по последним `period` точкам каждой позиции.
///
/// Возвращает вектор той же длины; первые `period - 1` значений — `None`
/// (окно ещё не заполнено). Утилита переиспользуется в [`flow`] и [`rrg`].
pub(crate) fn sma(values: &[f64], period: usize) -> Vec<Option<f64>> {
    assert!(period > 0, "period must be > 0");
    let mut out = Vec::with_capacity(values.len());
    let mut sum = 0.0;
    for i in 0..values.len() {
        sum += values[i];
        if i >= period {
            sum -= values[i - period];
        }
        if i + 1 >= period {
            out.push(Some(sum / period as f64));
        } else {
            out.push(None);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::sma;

    #[test]
    fn sma_basic_window() {
        let v = [1.0, 2.0, 3.0, 4.0, 5.0];
        let s = sma(&v, 3);
        assert_eq!(s[0], None);
        assert_eq!(s[1], None);
        assert_eq!(s[2], Some(2.0)); // (1+2+3)/3
        assert_eq!(s[3], Some(3.0)); // (2+3+4)/3
        assert_eq!(s[4], Some(4.0)); // (3+4+5)/3
    }

    #[test]
    fn sma_period_one_is_identity() {
        let v = [10.0, 20.0, 30.0];
        let s = sma(&v, 1);
        assert_eq!(s, vec![Some(10.0), Some(20.0), Some(30.0)]);
    }
}
