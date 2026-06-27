//! Асинхронная оркестрация ингеста: источник → хранилище.
//!
//! Замыкает Фазу 1: связывает сетевой контракт [`data::MarketData`] с
//! персистентным [`storage::Store`]. Здесь живёт цикл опроса вотчлиста —
//! синхронизация справочника, дозагрузка «хвоста» баров и round-robin обход
//! символов под лимит запросов (через [`storage::ingest::BatchCursor`]).
//!
//! Сетевые детали (gRPC, авторизация, rate-limit) инкапсулированы в `data`;
//! сюда приходит уже абстрактный источник. Поэтому оркестрация тестируется на
//! мок-источнике и `MemStore`, без сети и реального рантайма.

use data::{DataError, MarketData, TimeFrame};
use domain::Bar;
use storage::backfill::{chunk_range, plan_backfill};
use storage::ingest::{snapshot_from_bars, BatchCursor};
use storage::{StorageError, Store};

/// Предел числа баров в одном ответе API (страница бэкфилла).
pub const DEFAULT_MAX_BARS: usize = 500;

/// Ошибка ингеста: из источника данных либо из хранилища.
#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("источник данных: {0}")]
    Data(#[from] DataError),
    #[error("хранилище: {0}")]
    Storage(#[from] StorageError),
}

/// Итог одного такта опроса: что дозагружено и какие символы дали ошибку.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct PollReport {
    /// `(symbol, число записанных баров)` по успешно дозагруженным символам.
    pub backfilled: Vec<(String, usize)>,
    /// `(symbol, текст ошибки)` по символам, упавшим в этом такте.
    pub errors: Vec<(String, String)>,
}

impl PollReport {
    /// Всего баров записано за такт.
    pub fn bars_written(&self) -> usize {
        self.backfilled.iter().map(|(_, n)| n).sum()
    }
}

/// Синхронизировать справочник инструментов по списку площадок (MIC).
///
/// Тянет `AssetsService.Assets` по каждой площадке и upsert'ит в хранилище.
/// Возвращает число записанных строк справочника.
#[tracing::instrument(skip(src, store), fields(mics = mics.len()))]
pub async fn sync_instruments<M, S>(
    src: &M,
    store: &mut S,
    mics: &[&str],
) -> Result<usize, IngestError>
where
    M: MarketData,
    S: Store,
{
    let mut all = Vec::new();
    for mic in mics {
        let items = src.assets(mic).await?;
        tracing::debug!(mic, count = items.len(), "получен справочник площадки");
        all.extend(items);
    }
    let n = store.upsert_instruments(&all)?;
    tracing::info!(instruments = n, "справочник синхронизирован");
    Ok(n)
}

/// Дозагрузить «хвост» баров инструмента в окне `[desired_from, desired_to]`.
///
/// Планирует бэкфилл поверх последнего сохранённого бара (без повторного запроса
/// уже имеющейся истории), бьёт диапазон на страницы под `max_bars`, пишет бары
/// и обновляет снимок оборота. Возвращает число записанных баров.
#[tracing::instrument(skip(src, store), fields(symbol = symbol, tf = tf.code()))]
pub async fn backfill_symbol<M, S>(
    src: &M,
    store: &mut S,
    symbol: &str,
    tf: TimeFrame,
    desired_from: i64,
    desired_to: i64,
    max_bars: usize,
) -> Result<usize, IngestError>
where
    M: MarketData,
    S: Store,
{
    let last = store.last_bar_ts(symbol, tf)?;
    let Some(range) = plan_backfill(last, desired_from, desired_to, tf) else {
        tracing::debug!("история актуальна — дозагрузка не нужна");
        return Ok(0);
    };

    let mut written = 0;
    let mut fetched: Vec<Bar> = Vec::new();
    for page in chunk_range(range, tf, max_bars) {
        let bars = src.bars(symbol, tf, page.from_ts, page.to_ts).await?;
        if !bars.is_empty() {
            written += store.insert_bars(symbol, tf, &bars)?;
            fetched.extend_from_slice(&bars);
        }
    }

    if let Some(last_bar) = fetched.last() {
        if let Some(snap) = snapshot_from_bars(&fetched, last_bar.ts) {
            store.insert_snapshot(symbol, &snap)?;
        }
    }
    tracing::info!(bars = written, "бэкфилл символа завершён");
    Ok(written)
}

/// Один такт опроса: взять следующую порцию символов из `cursor` и дозагрузить
/// каждый. Ошибка по одному символу не прерывает остальные — она собирается в
/// [`PollReport::errors`] (изоляция сбоев).
#[tracing::instrument(skip(src, store, cursor), fields(tf = tf.code()))]
pub async fn poll_cycle<M, S>(
    src: &M,
    store: &mut S,
    cursor: &mut BatchCursor,
    tf: TimeFrame,
    desired_from: i64,
    desired_to: i64,
    max_bars: usize,
) -> PollReport
where
    M: MarketData,
    S: Store,
{
    let mut report = PollReport::default();
    for symbol in cursor.next_batch() {
        match backfill_symbol(src, store, &symbol, tf, desired_from, desired_to, max_bars).await {
            Ok(n) => {
                if n > 0 {
                    report.backfilled.push((symbol, n));
                }
            }
            Err(e) => {
                tracing::warn!(symbol, error = %e, "ошибка дозагрузки символа");
                report.errors.push((symbol, e.to_string()));
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{AssetClass, Instrument, Quote, Trade};
    use std::collections::HashMap;
    use storage::MemStore;

    /// Минимальный исполнитель фьючерсов без внешних зависимостей.
    fn block_on<F: std::future::Future>(fut: F) -> F::Output {
        use std::pin::pin;
        use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

        fn noop(_: *const ()) {}
        fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);

        let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
        let mut cx = Context::from_waker(&waker);
        let mut fut = pin!(fut);
        loop {
            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    fn demo_bar(ts: i64) -> Bar {
        Bar {
            ts,
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.5,
            volume: 1_000.0,
        }
    }

    /// Мок-источник: канонический справочник + бары на символ; для заданных
    /// символов отдаёт ошибку (проверка изоляции сбоев).
    struct MockSource {
        assets: Vec<Instrument>,
        bars: HashMap<String, Vec<Bar>>,
        fail: Vec<String>,
    }

    impl MarketData for MockSource {
        async fn assets(&self, _mic: &str) -> Result<Vec<Instrument>, DataError> {
            Ok(self.assets.clone())
        }

        async fn bars(
            &self,
            symbol: &str,
            _tf: TimeFrame,
            from_ts: i64,
            to_ts: i64,
        ) -> Result<Vec<Bar>, DataError> {
            if self.fail.iter().any(|s| s == symbol) {
                return Err(DataError::Transport("симулированный обрыв".into()));
            }
            Ok(self
                .bars
                .get(symbol)
                .map(|v| {
                    v.iter()
                        .copied()
                        .filter(|b| b.ts >= from_ts && b.ts <= to_ts)
                        .collect()
                })
                .unwrap_or_default())
        }

        async fn last_quote(&self, _symbol: &str) -> Result<Quote, DataError> {
            Err(DataError::Other("не используется в тесте".into()))
        }

        async fn latest_trades(&self, _symbol: &str) -> Result<Vec<Trade>, DataError> {
            Ok(Vec::new())
        }
    }

    fn inst(symbol: &str) -> Instrument {
        Instrument {
            symbol: symbol.into(),
            ticker: symbol.split('@').next().unwrap().into(),
            name: symbol.into(),
            asset_class: AssetClass::Equity,
            sector: None,
            lot_size: 1,
            isin: None,
        }
    }

    fn source() -> MockSource {
        let mut bars = HashMap::new();
        bars.insert(
            "SBER@MISX".to_string(),
            vec![demo_bar(1), demo_bar(2), demo_bar(3)],
        );
        bars.insert("LKOH@MISX".to_string(), vec![demo_bar(1), demo_bar(2)]);
        MockSource {
            assets: vec![inst("SBER@MISX"), inst("LKOH@MISX")],
            bars,
            fail: vec!["BAD@MISX".to_string()],
        }
    }

    fn fresh_store() -> MemStore {
        let mut s = MemStore::new();
        s.migrate().unwrap();
        s
    }

    #[test]
    fn sync_instruments_upserts_reference() {
        let src = source();
        let mut store = fresh_store();
        let n = block_on(sync_instruments(&src, &mut store, &["MISX"])).unwrap();
        assert_eq!(n, 2);
        assert_eq!(store.instruments().unwrap().len(), 2);
    }

    #[test]
    fn backfill_writes_bars_and_snapshot() {
        let src = source();
        let mut store = fresh_store();
        let n = block_on(backfill_symbol(
            &src,
            &mut store,
            "SBER@MISX",
            TimeFrame::D1,
            1,
            10,
            DEFAULT_MAX_BARS,
        ))
        .unwrap();
        assert_eq!(n, 3);
        assert_eq!(
            store
                .bars("SBER@MISX", TimeFrame::D1, 0, 100)
                .unwrap()
                .len(),
            3
        );
        // снимок оборота построен
        assert_eq!(store.snapshots("SBER@MISX", 0, 100).unwrap().len(), 1);
    }

    #[test]
    fn backfill_is_idempotent_tail_only() {
        let src = source();
        let mut store = fresh_store();
        // первый прогон загружает всё
        block_on(backfill_symbol(
            &src,
            &mut store,
            "SBER@MISX",
            TimeFrame::D1,
            1,
            10,
            DEFAULT_MAX_BARS,
        ))
        .unwrap();
        // повторный прогон: история актуальна — новых баров нет
        let n2 = block_on(backfill_symbol(
            &src,
            &mut store,
            "SBER@MISX",
            TimeFrame::D1,
            1,
            10,
            DEFAULT_MAX_BARS,
        ))
        .unwrap();
        assert_eq!(n2, 0);
    }

    #[test]
    fn poll_cycle_isolates_per_symbol_errors() {
        let src = source();
        let mut store = fresh_store();
        let mut cursor = BatchCursor::new(
            vec![
                "SBER@MISX".to_string(),
                "BAD@MISX".to_string(),
                "LKOH@MISX".to_string(),
            ],
            10,
        );
        let report = block_on(poll_cycle(
            &src,
            &mut store,
            &mut cursor,
            TimeFrame::D1,
            1,
            10,
            DEFAULT_MAX_BARS,
        ));
        // два символа дозагружены, один — в ошибках, цикл не прерван
        assert_eq!(report.backfilled.len(), 2);
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].0, "BAD@MISX");
        assert_eq!(report.bars_written(), 5); // SBER:3 + LKOH:2
    }
}
