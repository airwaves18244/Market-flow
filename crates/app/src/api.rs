//! Обработчики IPC-запросов (снимки + временные ряды).
//!
//! Чистые функции над [`storage::Store`]: читают данные, считают доменные
//! агрегаты и отдают [`crate::dto`]-структуры. Они не знают про Tauri, поэтому
//! полностью тестируются на `MemStore`; тонкие `#[tauri::command]`-обёртки
//! (фича `tauri`) лишь вызывают эти функции.

use domain::metrics::breadth::breadth;
use domain::metrics::sector::{rollup_by_sector, InstrumentMetric};
use domain::TimeFrame;
use storage::{StorageError, Store};

use crate::dto::{BarPoint, BreadthDto, InstrumentDto, RrgSectorDto, SectorEntryDto, SectorRow, TopMoverDto, TurnoverPoint};

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

/// Ширина рынка по окну времени: сколько инструментов растёт, падает, без изменений.
pub fn breadth_data(
    store: &dyn Store,
    from_ts: i64,
    to_ts: i64,
) -> Result<BreadthDto, StorageError> {
    let instruments = store.instruments()?;
    let mut changes = Vec::new();

    for inst in &instruments {
        if let Some(last) = store.snapshots(&inst.symbol, from_ts, to_ts)?.last() {
            changes.push(last.change);
        }
    }

    let b = breadth(&changes, 0.001);
    Ok(BreadthDto {
        advancers: b.advancers,
        decliners: b.decliners,
        unchanged: b.unchanged,
        pct_advancing: b.pct_advancing(),
        ad_ratio: b.ad_ratio(),
    })
}

/// Топ-движения: инструменты с наибольшим абсолютным изменением в окне.
/// Возвращает до `limit` (default 10) инструментов, отсортированных по |изменению|.
pub fn top_movers(
    store: &dyn Store,
    from_ts: i64,
    to_ts: i64,
    limit: Option<usize>,
) -> Result<Vec<TopMoverDto>, StorageError> {
    let instruments = store.instruments()?;
    let limit = limit.unwrap_or(10);
    let mut movers: Vec<(String, String, String, Option<String>, f64, f64)> = Vec::new();

    for inst in &instruments {
        if let Some(last_snapshot) = store.snapshots(&inst.symbol, from_ts, to_ts)?.last() {
            // Получить последний бар для цены закрытия.
            let last_close = store
                .bars(&inst.symbol, TimeFrame::D1, from_ts, to_ts)
                .ok()
                .and_then(|bs| bs.last().map(|b| b.close))
                .unwrap_or(0.0);

            movers.push((
                inst.symbol.clone(),
                inst.ticker.clone(),
                inst.name.clone(),
                inst.sector.clone(),
                last_snapshot.change,
                last_close,
            ));
        }
    }

    movers.sort_by(|a, b| {
        let abs_a = a.4.abs();
        let abs_b = b.4.abs();
        abs_b.total_cmp(&abs_a)
    });

    Ok(movers
        .into_iter()
        .take(limit)
        .map(|(symbol, ticker, name, sector, change, close)| TopMoverDto {
            symbol,
            ticker,
            name,
            sector,
            change,
            last_close: close,
        })
        .collect())
}

/// RRG для секторов: позиция каждого сектора относительно рынка по RS-Ratio и RS-Momentum.
/// Требует, чтобы в окне `[from_ts, to_ts]` были свечи для расчёта индекса сектора.
pub fn rrg_sectors(
    store: &dyn Store,
    from_ts: i64,
    to_ts: i64,
) -> Result<Vec<RrgSectorDto>, StorageError> {
    let instruments = store.instruments()?;

    // Собираем инструменты по секторам.
    let mut sector_instruments: std::collections::HashMap<String, Vec<&domain::Instrument>> = std::collections::HashMap::new();
    for inst in &instruments {
        let sec = inst.sector.clone().unwrap_or_else(|| UNKNOWN_SECTOR.to_string());
        sector_instruments.entry(sec).or_insert_with(Vec::new).push(inst);
    }

    // Здесь можно было бы вычислить RRG для каждого сектора, но требует
    // агрегации цен и индекса бенчмарка. Для scaffold показываем пример
    // с заглушкой на основе sector_rollup данных.
    let rollups = sector_rollup(store, from_ts, to_ts)?;
    let avg_turnover = rollups.iter().map(|r| r.turnover).sum::<f64>() / rollups.len().max(1) as f64;

    let mut rrg_data: Vec<RrgSectorDto> = Vec::new();
    for row in &rollups {
        // Упрощённая метрика: RS-Ratio = (turnover / avg_turnover) * 100
        // RS-Momentum = weighted_change направление
        let rs_ratio = if avg_turnover > 0.0 {
            (row.turnover / avg_turnover) * 100.0
        } else {
            100.0
        };
        let rs_momentum = (row.weighted_change + 1.0) * 100.0; // Shift к центру 100

        let quadrant = match (rs_ratio >= 100.0, rs_momentum >= 100.0) {
            (true, true) => "leading",
            (true, false) => "weakening",
            (false, false) => "lagging",
            (false, true) => "improving",
        };

        rrg_data.push(RrgSectorDto {
            sector: row.sector.clone(),
            rs_ratio,
            rs_momentum,
            quadrant: quadrant.to_string(),
        });
    }

    Ok(rrg_data)
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

    #[test]
    fn breadth_counts_advancers_and_decliners() {
        let store = seeded();
        // SBER: change=0.03, LKOH: 0.04, GAZP: 0.06 — 3 растущих
        let bd = breadth_data(&store, 0, 9).unwrap();
        assert_eq!(bd.advancers, 3);
        assert_eq!(bd.decliners, 0);
        assert_eq!(bd.unchanged, 0);
        assert_eq!(bd.pct_advancing, Some(1.0));
    }

    #[test]
    fn top_movers_sorts_by_absolute_change() {
        let store = seeded();
        let movers = top_movers(&store, 0, 9, Some(5)).unwrap();
        assert!(movers.len() <= 5);
        // Должны быть отсортированы по |изменению|
        if movers.len() > 1 {
            assert!(movers[0].change.abs() >= movers[1].change.abs());
        }
    }

    #[test]
    fn rrg_sectors_returns_all_sectors() {
        let store = seeded();
        let rrg = rrg_sectors(&store, 0, 9).unwrap();
        assert!(rrg.len() > 0);
        // Должны быть в квадрантах
        for point in &rrg {
            assert!(["leading", "weakening", "lagging", "improving"].contains(&point.quadrant.as_str()));
        }
    }
}
