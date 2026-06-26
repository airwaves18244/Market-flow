//! Обработчики IPC-запросов (снимки + временные ряды).
//!
//! Чистые функции над [`storage::Store`]: читают данные, считают доменные
//! агрегаты и отдают [`crate::dto`]-структуры. Они не знают про Tauri, поэтому
//! полностью тестируются на `MemStore`; тонкие `#[tauri::command]`-обёртки
//! (фича `tauri`) лишь вызывают эти функции.

use domain::metrics::sector::{rollup_by_sector, InstrumentMetric};
use domain::TimeFrame;
use storage::{StorageError, Store};

use crate::dto::{BarPoint, InstrumentDto, SectorEntryDto, SectorRow, TurnoverPoint};

/// Метка сектора для инструментов без классификации.
const UNKNOWN_SECTOR: &str = "Прочее";

/// Справочник инструментов, отсортированный по символу.
pub fn instruments(store: &dyn Store) -> Result<Vec<InstrumentDto>, StorageError> {
    let mut out: Vec<InstrumentDto> = store.instruments()?.iter().map(InstrumentDto::from).collect();
    out.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    Ok(out)
}

/// Свечи инструмента в `[from_ts, to_ts]` для свечного графика.
pub fn bars(
    store: &dyn Store,
    symbol: &str,
    tf: TimeFrame,
    from_ts: i64,
    to_ts: i64,
) -> Result<Vec<BarPoint>, StorageError> {
    Ok(store
        .bars(symbol, tf, from_ts, to_ts)?
        .into_iter()
        .map(|b| BarPoint {
            ts: b.ts,
            open: b.open,
            high: b.high,
            low: b.low,
            close: b.close,
            volume: b.volume,
        })
        .collect())
}

/// Временной ряд оборота/потока инструмента в `[from_ts, to_ts]`.
pub fn turnover_series(
    store: &dyn Store,
    symbol: &str,
    from_ts: i64,
    to_ts: i64,
) -> Result<Vec<TurnoverPoint>, StorageError> {
    Ok(store
        .snapshots(symbol, from_ts, to_ts)?
        .iter()
        .map(TurnoverPoint::from)
        .collect())
}

/// Записи таблицы классификации секторов.
pub fn sector_map(store: &dyn Store) -> Result<Vec<SectorEntryDto>, StorageError> {
    Ok(store.sector_map()?.iter().map(SectorEntryDto::from).collect())
}

/// Секторный роллап для treemap/heatmap: по каждому инструменту берём
/// последний снимок оборота в окне `[from_ts, to_ts]`, относим его к сектору
/// инструмента и агрегируем (взвешивая изменение по обороту). Строки
/// отсортированы по убыванию оборота — крупнейшие плитки первыми.
pub fn sector_rollup(
    store: &dyn Store,
    from_ts: i64,
    to_ts: i64,
) -> Result<Vec<SectorRow>, StorageError> {
    let instruments = store.instruments()?;

    // (сектор, метрика) по инструментам, у которых есть снимок в окне.
    let mut items: Vec<(Option<String>, InstrumentMetric)> = Vec::new();
    for inst in &instruments {
        if let Some(last) = store.snapshots(&inst.symbol, from_ts, to_ts)?.last() {
            items.push((
                inst.sector.clone(),
                InstrumentMetric {
                    turnover: last.turnover,
                    net_flow: last.net_flow,
                    change: last.change,
                },
            ));
        }
    }

    let rolled = rollup_by_sector(
        items.iter().map(|(s, m)| (s.as_deref(), *m)),
        UNKNOWN_SECTOR,
    );

    let mut rows: Vec<SectorRow> = rolled
        .into_iter()
        .map(|(sector, agg)| SectorRow {
            sector,
            instruments: agg.instruments,
            turnover: agg.turnover,
            net_flow: agg.net_flow,
            weighted_change: agg.weighted_change,
        })
        .collect();
    rows.sort_by(|a, b| b.turnover.total_cmp(&a.turnover));
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{AssetClass, Bar, Instrument};
    use storage::ingest::Writer;
    use storage::MemStore;

    fn inst(symbol: &str, sector: Option<&str>) -> Instrument {
        Instrument {
            symbol: symbol.into(),
            ticker: symbol.split('@').next().unwrap().into(),
            name: symbol.into(),
            asset_class: AssetClass::Equity,
            sector: sector.map(str::to_string),
            lot_size: 1,
            isin: None,
        }
    }

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

    fn seeded() -> MemStore {
        let mut store = MemStore::new();
        store.migrate().unwrap();
        store
            .upsert_instruments(&[
                inst("SBER@MISX", Some("Финансы")),
                inst("LKOH@MISX", Some("Нефтегаз")),
                inst("GAZP@MISX", Some("Нефтегаз")),
                inst("XXXX@MISX", None),
            ])
            .unwrap();

        let mut w = Writer::new(&mut store);
        // бары + снимок на ts=3 для каждого
        for (sym, base) in [("SBER@MISX", 100.0), ("LKOH@MISX", 50.0), ("GAZP@MISX", 200.0)] {
            let bars = [
                bar(1, base, base * 1.01, 1_000.0),
                bar(2, base * 1.01, base * 1.02, 1_000.0),
                bar(3, base * 1.02, base * 1.03, 1_000.0),
            ];
            w.bars(sym, TimeFrame::D1, &bars).unwrap();
            w.snapshot_from_bars(sym, &bars, 3).unwrap();
        }
        store
    }

    #[test]
    fn instruments_are_sorted() {
        let store = seeded();
        let got = instruments(&store).unwrap();
        let syms: Vec<&str> = got.iter().map(|i| i.symbol.as_str()).collect();
        assert_eq!(syms, ["GAZP@MISX", "LKOH@MISX", "SBER@MISX", "XXXX@MISX"]);
        assert_eq!(got[0].asset_class, "equity");
    }

    #[test]
    fn bars_map_through() {
        let store = seeded();
        let got = bars(&store, "SBER@MISX", TimeFrame::D1, 0, 9).unwrap();
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].ts, 1);
        assert!(got[2].close > got[2].open);
    }

    #[test]
    fn turnover_series_returns_snapshot() {
        let store = seeded();
        let got = turnover_series(&store, "SBER@MISX", 0, 9).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].ts, 3);
        assert!(got[0].turnover > 0.0);
    }

    #[test]
    fn sector_rollup_aggregates_and_sorts_by_turnover() {
        let store = seeded();
        let rows = sector_rollup(&store, 0, 9).unwrap();
        // Нефтегаз (LKOH+GAZP) и Финансы (SBER); XXXX без снимка-сектора → его
        // снимок отсутствует, значит "Прочее" не появляется здесь.
        let sectors: Vec<&str> = rows.iter().map(|r| r.sector.as_str()).collect();
        assert!(sectors.contains(&"Нефтегаз"));
        assert!(sectors.contains(&"Финансы"));
        // Нефтегаз = 2 инструмента
        let og = rows.iter().find(|r| r.sector == "Нефтегаз").unwrap();
        assert_eq!(og.instruments, 2);
        // отсортировано по убыванию оборота
        assert!(rows.windows(2).all(|w| w[0].turnover >= w[1].turnover));
    }

    #[test]
    fn sector_rollup_uses_unknown_label_for_unclassified() {
        let mut store = MemStore::new();
        store.migrate().unwrap();
        store.upsert_instruments(&[inst("ZZZZ@MISX", None)]).unwrap();
        let mut w = Writer::new(&mut store);
        let bars = [bar(1, 10.0, 11.0, 100.0)];
        w.bars("ZZZZ@MISX", TimeFrame::D1, &bars).unwrap();
        w.snapshot_from_bars("ZZZZ@MISX", &bars, 1).unwrap();

        let rows = sector_rollup(&store, 0, 9).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].sector, UNKNOWN_SECTOR);
    }
}
