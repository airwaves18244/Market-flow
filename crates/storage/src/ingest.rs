//! Ингест рыночных данных в хранилище.
//!
//! Здесь живёт логика, превращающая ответы источника (`data::MarketData`,
//! приведённые к доменным типам) в строки таблиц:
//! - [`Writer`] — тонкая обёртка над [`Store`] с удобными методами записи;
//! - [`snapshot_from_bars`] — построение снимка оборота из серии баров;
//! - [`BatchCursor`] — планировщик батч-поллинга (round-robin по символам),
//!   чтобы уважать лимит ~200 запросов/мин на метод Finam.
//!
//! Сам асинхронный цикл опроса (с tokio и реальным gRPC-клиентом) собирается в
//! `app`; здесь — синхронные, кросс-платформенно тестируемые кирпичики.

use domain::algo::{FutoiPoint, Hi2Point, SuperCandle};
use domain::metrics::turnover::{directional_turnover, total_turnover};
use domain::{Bar, Instrument, TimeFrame, Trade};

use crate::store::{AlgoObstatsRecord, AlgoOrderstatsRecord, SectorEntry, Store, TurnoverSnapshot};
use crate::StorageError;

/// Построить снимок оборота из серии баров за период (напр. за торговый день).
///
/// - `turnover` — суммарный денежный оборот серии;
/// - `net_flow` — направленный оборот «вверх − вниз» (см. `domain::metrics`);
/// - `change` — относительное изменение `(last.close − first.open) / first.open`.
///
/// `ts` снимка задаётся явно (обычно — время последнего бара). Возвращает
/// `None` для пустой серии.
pub fn snapshot_from_bars(bars: &[Bar], ts: i64) -> Option<TurnoverSnapshot> {
    let first = bars.first()?;
    let last = bars.last()?;
    let change = if first.open != 0.0 {
        (last.close - first.open) / first.open
    } else {
        0.0
    };
    Some(TurnoverSnapshot {
        ts,
        turnover: total_turnover(bars),
        net_flow: directional_turnover(bars).net(),
        change,
    })
}

/// Построить записи классификации секторов из пар `(ключ, сектор)`.
///
/// ISIN распознаётся по форме (12 символов, первые два — латинские буквы);
/// тикеры нормализуются в верхний регистр для устойчивого поиска.
pub fn sector_entries<I, S>(pairs: I) -> Vec<SectorEntry>
where
    I: IntoIterator<Item = (S, S)>,
    S: Into<String>,
{
    pairs
        .into_iter()
        .map(|(key, sector)| {
            let key = key.into();
            let is_isin = is_isin(&key);
            let key = if is_isin { key } else { key.to_uppercase() };
            SectorEntry {
                key,
                sector: sector.into(),
                is_isin,
            }
        })
        .collect()
}

/// Грубая проверка формата ISIN: 12 символов, первые два — латинские буквы.
fn is_isin(s: &str) -> bool {
    s.len() == 12
        && s.chars().take(2).all(|c| c.is_ascii_alphabetic())
        && s.chars().skip(2).all(|c| c.is_ascii_alphanumeric())
}

/// Обёртка над хранилищем с удобными методами ингеста.
pub struct Writer<'a, S: Store> {
    store: &'a mut S,
}

impl<'a, S: Store> Writer<'a, S> {
    pub fn new(store: &'a mut S) -> Self {
        Self { store }
    }

    /// Записать справочник инструментов.
    pub fn instruments(&mut self, items: &[Instrument]) -> Result<usize, StorageError> {
        self.store.upsert_instruments(items)
    }

    /// Записать бары инструмента в заданном тайм-фрейме.
    pub fn bars(
        &mut self,
        symbol: &str,
        tf: TimeFrame,
        bars: &[Bar],
    ) -> Result<usize, StorageError> {
        self.store.insert_bars(symbol, tf, bars)
    }

    /// Посчитать снимок оборота из серии баров и записать его. Возвращает
    /// `Ok(false)`, если серия пуста (записывать нечего).
    pub fn snapshot_from_bars(
        &mut self,
        symbol: &str,
        bars: &[Bar],
        ts: i64,
    ) -> Result<bool, StorageError> {
        match snapshot_from_bars(bars, ts) {
            Some(snap) => {
                self.store.insert_snapshot(symbol, &snap)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Дописать обезличенные сделки (тиковую ленту) инструмента. Питает
    /// footprint/дельту и симулятор исполнения. Возвращает число строк.
    pub fn trades(&mut self, symbol: &str, trades: &[Trade]) -> Result<usize, StorageError> {
        self.store.insert_trades(symbol, trades)
    }

    /// Загрузить таблицу классификации секторов из пар `(ключ, сектор)`.
    pub fn load_sector_map<I, T>(&mut self, pairs: I) -> Result<usize, StorageError>
    where
        I: IntoIterator<Item = (T, T)>,
        T: Into<String>,
    {
        let entries = sector_entries(pairs);
        self.store.upsert_sector_map(&entries)
    }

    /// Записать свечи Super Candles (датасет ALGOPACK `tradestats`) для
    /// рынка `market`. Идемпотентно по ключу (secid, ts, market) — повторный
    /// ингест той же свечи не плодит дублей.
    pub fn algo_tradestats(
        &mut self,
        market: &str,
        candles: &[SuperCandle],
    ) -> Result<usize, StorageError> {
        self.store.insert_algo_tradestats(market, candles)
    }

    /// Записать точки FUTOI (датасет ALGOPACK `futoi`, рынок `fo`).
    /// Идемпотентно по ключу (secid, ts, market, clgroup).
    pub fn algo_futoi(
        &mut self,
        market: &str,
        points: &[FutoiPoint],
    ) -> Result<usize, StorageError> {
        self.store.insert_algo_futoi(market, points)
    }

    /// Записать точки HI2 (датасет ALGOPACK `hi2`). Идемпотентно по ключу
    /// (secid, ts, market).
    pub fn algo_hi2(&mut self, market: &str, points: &[Hi2Point]) -> Result<usize, StorageError> {
        self.store.insert_algo_hi2(market, points)
    }

    /// Записать статистику стакана заявок (датасет ALGOPACK `obstats`).
    /// Идемпотентно по ключу (secid, ts, market).
    pub fn algo_obstats(&mut self, records: &[AlgoObstatsRecord]) -> Result<usize, StorageError> {
        self.store.insert_algo_obstats(records)
    }

    /// Записать статистику заявок (датасет ALGOPACK `orderstats`).
    /// Идемпотентно по ключу (secid, ts, market).
    pub fn algo_orderstats(
        &mut self,
        records: &[AlgoOrderstatsRecord],
    ) -> Result<usize, StorageError> {
        self.store.insert_algo_orderstats(records)
    }
}

/// Планировщик батч-поллинга: round-robin по списку символов.
///
/// За один такт опроса берём не больше `batch` символов, затем сдвигаем курсор.
/// Так мы равномерно обходим вотчлист, не превышая лимит запросов на метод.
#[derive(Debug, Clone)]
pub struct BatchCursor {
    symbols: Vec<String>,
    batch: usize,
    pos: usize,
}

impl BatchCursor {
    /// `batch` зажимается в `>= 1`.
    pub fn new(symbols: Vec<String>, batch: usize) -> Self {
        Self {
            symbols,
            batch: batch.max(1),
            pos: 0,
        }
    }

    /// Следующая порция символов; курсор сдвигается с переносом по кругу.
    pub fn next_batch(&mut self) -> Vec<String> {
        if self.symbols.is_empty() {
            return Vec::new();
        }
        let n = self.batch.min(self.symbols.len());
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(self.symbols[self.pos].clone());
            self.pos = (self.pos + 1) % self.symbols.len();
        }
        out
    }

    /// Сколько тактов нужно, чтобы один раз обойти весь список.
    pub fn cycles_per_sweep(&self) -> usize {
        if self.symbols.is_empty() {
            0
        } else {
            self.symbols.len().div_ceil(self.batch)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::MemStore;

    fn bar(ts: i64, open: f64, close: f64, vol: f64) -> Bar {
        Bar {
            ts,
            open,
            high: open.max(close),
            low: open.min(close),
            close,
            volume: vol,
        }
    }

    #[test]
    fn snapshot_from_bars_computes_flow_and_change() {
        let bars = [
            bar(1, 10.0, 12.0, 100.0), // up
            bar(2, 12.0, 11.0, 50.0),  // down
        ];
        let snap = snapshot_from_bars(&bars, 2).unwrap();
        assert_eq!(snap.ts, 2);
        assert!(snap.turnover > 0.0);
        // change = (11 - 10) / 10 = 0.1
        assert!((snap.change - 0.1).abs() < 1e-12);
        // up-бар крупнее по обороту → net_flow > 0
        assert!(snap.net_flow > 0.0);
    }

    #[test]
    fn snapshot_from_empty_is_none() {
        assert!(snapshot_from_bars(&[], 0).is_none());
    }

    #[test]
    fn writer_persists_bars_and_snapshot() {
        let mut store = MemStore::new();
        let bars = [bar(1, 10.0, 12.0, 100.0), bar(2, 12.0, 13.0, 80.0)];
        {
            let mut w = Writer::new(&mut store);
            assert_eq!(w.bars("SBER@MISX", TimeFrame::D1, &bars).unwrap(), 2);
            assert!(w.snapshot_from_bars("SBER@MISX", &bars, 2).unwrap());
        }
        assert_eq!(
            store.bars("SBER@MISX", TimeFrame::D1, 0, 9).unwrap().len(),
            2
        );
        assert_eq!(store.snapshots("SBER@MISX", 0, 9).unwrap().len(), 1);
    }

    #[test]
    fn writer_load_sector_map_detects_isin_and_uppercases_ticker() {
        let mut store = MemStore::new();
        {
            let mut w = Writer::new(&mut store);
            w.load_sector_map([("sber", "Финансы"), ("RU0009029540", "Финансы")])
                .unwrap();
        }
        let mut entries = store.sector_map().unwrap();
        entries.sort_by(|a, b| a.key.cmp(&b.key));
        // тикер нормализован в верхний регистр, ISIN распознан
        let ticker = entries.iter().find(|e| !e.is_isin).unwrap();
        let isin = entries.iter().find(|e| e.is_isin).unwrap();
        assert_eq!(ticker.key, "SBER");
        assert_eq!(isin.key, "RU0009029540");
    }

    #[test]
    fn writer_algo_tradestats_dedups_by_key() {
        let mut store = MemStore::new();
        let candle = |ts: i64, close: f64| SuperCandle {
            secid: "RIH5".into(),
            ts,
            pr_open: close,
            pr_high: close,
            pr_low: close,
            pr_close: close,
            pr_std: 0.1,
            vol: 10.0,
            val: close * 10.0,
            trades: 1.0,
            pr_vwap: close,
            pr_change: 0.0,
            vol_b: 6.0,
            vol_s: 4.0,
            val_b: close * 6.0,
            val_s: close * 4.0,
            trades_b: 1.0,
            trades_s: 0.0,
            disb: 0.2,
            pr_vwap_b: close,
            pr_vwap_s: close,
        };
        {
            let mut w = Writer::new(&mut store);
            w.algo_tradestats("fo", &[candle(1, 10.0), candle(2, 20.0)])
                .unwrap();
            // повторный ингест с той же ts — перезаписывает, не дублирует
            w.algo_tradestats("fo", &[candle(1, 99.0)]).unwrap();
        }
        let got = store.algo_tradestats("fo", "RIH5", 0, 9).unwrap();
        assert_eq!(got.len(), 2);
        assert!((got[0].pr_close - 99.0).abs() < 1e-12);
    }

    #[test]
    fn batch_cursor_round_robins() {
        let mut c = BatchCursor::new(vec!["A".into(), "B".into(), "C".into()], 2);
        assert_eq!(c.cycles_per_sweep(), 2); // ceil(3/2)
        assert_eq!(c.next_batch(), vec!["A", "B"]);
        assert_eq!(c.next_batch(), vec!["C", "A"]); // переносится по кругу
        assert_eq!(c.next_batch(), vec!["B", "C"]);
    }

    #[test]
    fn batch_cursor_handles_empty_and_zero_batch() {
        let mut empty = BatchCursor::new(vec![], 4);
        assert!(empty.next_batch().is_empty());
        assert_eq!(empty.cycles_per_sweep(), 0);

        let mut c = BatchCursor::new(vec!["A".into()], 0); // batch зажат до 1
        assert_eq!(c.next_batch(), vec!["A"]);
    }
}
