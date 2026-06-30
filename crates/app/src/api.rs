//! Обработчики IPC-запросов (снимки + временные ряды).
//!
//! Чистые функции над [`storage::Store`]: читают данные, считают доменные
//! агрегаты и отдают [`crate::dto`]-структуры. Они не знают про Tauri, поэтому
//! полностью тестируются на `MemStore`; тонкие `#[tauri::command]`-обёртки
//! (фича `tauri`) лишь вызывают эти функции.

use std::collections::BTreeMap;

use domain::backtest::{
    descriptors, run_backtest as domain_run_backtest, strategy_from_id, StrategyParams,
};
use domain::metrics::alerts::{AlertEngine, Observation};
use domain::metrics::breadth::breadth;
use domain::metrics::crossasset::{flow_matrix, turnover_shares, TurnoverShares};
use domain::metrics::sector::{rollup_by_sector, InstrumentMetric};
use domain::{AssetClass, TimeFrame};
use storage::{StorageError, Store};

use crate::dto::{
    AlertEventDto, AlertRuleInput, AssetClassShareDto, BacktestConfigInput, BacktestReportDto,
    BarPoint, BondIssuerDto, BreadthDto, CrossAssetSummaryDto, FlowEdgeDto, FutureGroupDto,
    InstrumentDto, RrgSectorDto, SectorEntryDto, SectorRow, StrategyDescriptorDto, TopMoverDto,
    TurnoverByClassPoint, TurnoverPoint, YieldCurvePoint,
};

/// Метка сектора для инструментов без классификации.
const UNKNOWN_SECTOR: &str = "Прочее";

/// Префикс из первых `n` символов (не байт) в верхнем регистре.
///
/// Группировка по символам, а не байтам, важна для кириллических тикеров:
/// байтовый срез `s[..n]` на UTF-8 паникует, если режет середину символа.
fn char_prefix(s: &str, n: usize) -> String {
    s.chars().take(n).collect::<String>().to_uppercase()
}

/// Справочник инструментов, отсортированный по символу.
pub fn instruments(store: &dyn Store) -> Result<Vec<InstrumentDto>, StorageError> {
    let mut out: Vec<InstrumentDto> = store
        .instruments()?
        .iter()
        .map(InstrumentDto::from)
        .collect();
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
    Ok(store
        .sector_map()?
        .iter()
        .map(SectorEntryDto::from)
        .collect())
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
        .map(
            |(symbol, ticker, name, sector, change, close)| TopMoverDto {
                symbol,
                ticker,
                name,
                sector,
                change,
                last_close: close,
            },
        )
        .collect())
}

/// RRG для секторов: позиция каждого сектора на плоскости RS-Ratio / RS-Momentum.
///
/// Упрощённая реализация (scaffold): относительную силу оцениваем по доле оборота
/// сектора (turnover / средний по секторам), а импульс — по средневзвешенному
/// изменению. Полноценный RRG (`domain::metrics::rrg`) требует выровненных по
/// времени ценовых серий сектор/бенчмарк — это задача фазы интеграции данных.
pub fn rrg_sectors(
    store: &dyn Store,
    from_ts: i64,
    to_ts: i64,
) -> Result<Vec<RrgSectorDto>, StorageError> {
    let rollups = sector_rollup(store, from_ts, to_ts)?;
    if rollups.is_empty() {
        return Ok(Vec::new());
    }
    let avg_turnover = rollups.iter().map(|r| r.turnover).sum::<f64>() / rollups.len() as f64;

    let rrg_data = rollups
        .iter()
        .map(|row| {
            // RS-Ratio: оборот сектора относительно среднего, масштаб к 100.
            let rs_ratio = if avg_turnover > 0.0 {
                (row.turnover / avg_turnover) * 100.0
            } else {
                100.0
            };
            // RS-Momentum: изменение в долях, сдвинутое к центру 100
            // (+1% → 101, −1% → 99).
            let rs_momentum = (row.weighted_change + 1.0) * 100.0;

            let quadrant = match (rs_ratio >= 100.0, rs_momentum >= 100.0) {
                (true, true) => "leading",
                (true, false) => "weakening",
                (false, false) => "lagging",
                (false, true) => "improving",
            };

            RrgSectorDto {
                sector: row.sector.clone(),
                rs_ratio,
                rs_momentum,
                quadrant: quadrant.to_string(),
            }
        })
        .collect();

    Ok(rrg_data)
}

/// Агрегация фьючерсов по группам (базовая):
/// собираются инструменты класса "future", группируются по 2-символьному
/// префиксу тикера (корень контракта, напр. `Si`, `RI`), в каждой группе
/// считаются обороты и потоки.
pub fn futures_rollup(
    store: &dyn Store,
    from_ts: i64,
    to_ts: i64,
) -> Result<Vec<FutureGroupDto>, StorageError> {
    let futures = store.instruments_by_asset_class("future")?;

    // Группируем по 2-символьному корню тикера: Si, RI, ED, GD и т.д.
    let mut groups: std::collections::HashMap<String, Vec<InstrumentMetric>> =
        std::collections::HashMap::new();

    for fut in &futures {
        let group = char_prefix(&fut.ticker, 2);

        if let Some(last) = store.snapshots(&fut.symbol, from_ts, to_ts)?.last() {
            groups.entry(group).or_default().push(InstrumentMetric {
                turnover: last.turnover,
                net_flow: last.net_flow,
                change: last.change,
            });
        }
    }

    let mut rows: Vec<FutureGroupDto> = groups
        .into_iter()
        .map(|(group, metrics)| {
            let total_turnover = metrics.iter().map(|m| m.turnover).sum::<f64>();
            let total_flow = metrics.iter().map(|m| m.net_flow).sum::<f64>();
            let weighted_change = if total_turnover > 0.0 {
                metrics.iter().map(|m| m.change * m.turnover).sum::<f64>() / total_turnover
            } else {
                0.0
            };

            FutureGroupDto {
                group,
                contracts: metrics.len() as u32,
                turnover: total_turnover,
                net_flow: total_flow,
                weighted_change,
                open_interest: 0.0, // Placeholder: требует отдельных данных
            }
        })
        .collect();

    rows.sort_by(|a, b| b.turnover.total_cmp(&a.turnover));
    Ok(rows)
}

/// Агрегация облигаций по эмитентам (базовая):
/// собираются инструменты класса "bond", группируются по 3-символьному
/// префиксу тикера (эмитент), в каждой группе считаются обороты и потоки.
///
/// `avg_yield`/`weighted_duration` сейчас 0.0: доходность и дюрация требуют
/// отдельного источника данных (купоны/погашение), которого пока нет в
/// хранилище — поля добавлены под интеграцию, чтобы не фабриковать значения.
pub fn bonds_rollup(
    store: &dyn Store,
    from_ts: i64,
    to_ts: i64,
) -> Result<Vec<BondIssuerDto>, StorageError> {
    let bonds = store.instruments_by_asset_class("bond")?;

    // Группируем по 3-символьному префиксу эмитента: OFZ, GAZ, LUK и т.д.
    let mut issuers: std::collections::HashMap<String, Vec<InstrumentMetric>> =
        std::collections::HashMap::new();

    for bond in &bonds {
        let issuer = char_prefix(&bond.ticker, 3);

        if let Some(last) = store.snapshots(&bond.symbol, from_ts, to_ts)?.last() {
            issuers.entry(issuer).or_default().push(InstrumentMetric {
                turnover: last.turnover,
                net_flow: last.net_flow,
                change: last.change,
            });
        }
    }

    let mut rows: Vec<BondIssuerDto> = issuers
        .into_iter()
        .map(|(issuer, metrics)| {
            let total_turnover = metrics.iter().map(|m| m.turnover).sum::<f64>();
            let total_flow = metrics.iter().map(|m| m.net_flow).sum::<f64>();

            BondIssuerDto {
                issuer,
                bonds: metrics.len() as u32,
                turnover: total_turnover,
                net_flow: total_flow,
                avg_yield: 0.0,         // требует источника купонов/погашения
                weighted_duration: 0.0, // требует источника купонов/погашения
            }
        })
        .collect();

    rows.sort_by(|a, b| b.turnover.total_cmp(&a.turnover));
    Ok(rows)
}

/// Кривая доходности облигаций.
///
/// Сейчас — статическая иллюстративная кривая (scaffold): реальная кривая
/// строится по доходностям выпусков, которых пока нет в хранилище. Вынесена в
/// отдельную команду, чтобы фронт подключился к контракту до интеграции данных.
pub fn yield_curve() -> Result<Vec<YieldCurvePoint>, StorageError> {
    Ok(vec![
        YieldCurvePoint {
            maturity_years: 0.25,
            yield_pct: 4.5,
        },
        YieldCurvePoint {
            maturity_years: 0.5,
            yield_pct: 4.7,
        },
        YieldCurvePoint {
            maturity_years: 1.0,
            yield_pct: 5.1,
        },
        YieldCurvePoint {
            maturity_years: 2.0,
            yield_pct: 5.6,
        },
        YieldCurvePoint {
            maturity_years: 3.0,
            yield_pct: 5.9,
        },
        YieldCurvePoint {
            maturity_years: 5.0,
            yield_pct: 6.2,
        },
        YieldCurvePoint {
            maturity_years: 7.0,
            yield_pct: 6.4,
        },
        YieldCurvePoint {
            maturity_years: 10.0,
            yield_pct: 6.5,
        },
    ])
}

// ── Фаза 6 — представление «Сумма всех» (кросс-актив) ──────────────────────

/// Доли классов активов: оборот по классам из последних снимков в окне.
///
/// Берём последний снимок каждого инструмента (как `sector_rollup`/`breadth`),
/// суммируем оборот по классам и считаем доли. Питает gauge общего оборота и
/// donut долей.
fn class_turnover(
    store: &dyn Store,
    from_ts: i64,
    to_ts: i64,
) -> Result<BTreeMap<AssetClass, f64>, StorageError> {
    let mut by_class: BTreeMap<AssetClass, f64> = BTreeMap::new();
    for inst in store.instruments()? {
        if let Some(last) = store.snapshots(&inst.symbol, from_ts, to_ts)?.last() {
            *by_class.entry(inst.asset_class).or_default() += last.turnover;
        }
    }
    Ok(by_class)
}

/// Сводка по классам активов: общий оборот + доли (gauge + donut).
pub fn cross_asset_summary(
    store: &dyn Store,
    from_ts: i64,
    to_ts: i64,
) -> Result<CrossAssetSummaryDto, StorageError> {
    let by_class = class_turnover(store, from_ts, to_ts)?;
    let shares = turnover_shares(&by_class);

    let rows = AssetClass::ALL
        .iter()
        .map(|&c| AssetClassShareDto {
            asset_class: c.code().to_string(),
            turnover: by_class.get(&c).copied().unwrap_or(0.0),
            share: shares.share_of(c),
        })
        .collect();

    Ok(CrossAssetSummaryDto {
        total: shares.total,
        shares: rows,
    })
}

/// Временной ряд оборота по классам активов (stacked area).
///
/// Группирует все снимки в окне по `ts` и классу актива, суммируя оборот.
pub fn turnover_timeline(
    store: &dyn Store,
    from_ts: i64,
    to_ts: i64,
) -> Result<Vec<TurnoverByClassPoint>, StorageError> {
    // ts → (класс → суммарный оборот)
    let mut timeline: BTreeMap<i64, BTreeMap<AssetClass, f64>> = BTreeMap::new();
    for inst in store.instruments()? {
        for snap in store.snapshots(&inst.symbol, from_ts, to_ts)? {
            *timeline
                .entry(snap.ts)
                .or_default()
                .entry(inst.asset_class)
                .or_default() += snap.turnover;
        }
    }

    Ok(timeline
        .into_iter()
        .map(|(ts, m)| TurnoverByClassPoint {
            ts,
            equity: m.get(&AssetClass::Equity).copied().unwrap_or(0.0),
            future: m.get(&AssetClass::Future).copied().unwrap_or(0.0),
            bond: m.get(&AssetClass::Bond).copied().unwrap_or(0.0),
        })
        .collect())
}

/// Перетоки долей между классами активов (Sankey).
///
/// Сравнивает распределение долей оборота в первой и последней точках окна
/// (`domain::metrics::crossasset::flow_matrix`). `< 2` точек → нет рёбер.
pub fn flow_sankey(
    store: &dyn Store,
    from_ts: i64,
    to_ts: i64,
) -> Result<Vec<FlowEdgeDto>, StorageError> {
    let timeline = turnover_timeline(store, from_ts, to_ts)?;
    if timeline.len() < 2 {
        return Ok(Vec::new());
    }

    let to_shares = |p: &TurnoverByClassPoint| -> TurnoverShares {
        let mut m = BTreeMap::new();
        m.insert(AssetClass::Equity, p.equity);
        m.insert(AssetClass::Future, p.future);
        m.insert(AssetClass::Bond, p.bond);
        turnover_shares(&m)
    };

    let prev = to_shares(timeline.first().unwrap());
    let curr = to_shares(timeline.last().unwrap());

    Ok(flow_matrix(&prev, &curr, 1e-6)
        .into_iter()
        .map(|e| FlowEdgeDto {
            from: e.from.code().to_string(),
            to: e.to.code().to_string(),
            weight: e.weight,
        })
        .collect())
}

// ── Фаза 7 — алёрты по сохранённым данным ──────────────────────────────────

/// Прогон правил алёртов по сохранённым барам.
///
/// Для каждого правила берём дневные бары соответствующего инструмента в окне
/// `[from_ts, to_ts]`, строим наблюдения (`price` = закрытие бара, `change` =
/// `(close − open) / open` — дневное изменение в долях) и пропускаем их через
/// edge-triggered [`AlertEngine`]. Наблюдения по всем инструментам
/// упорядочиваются по времени, чтобы движок видел согласованную хронологию.
/// Возвращает события в порядке срабатывания — основа для replay-проверки
/// правил и для панели алёртов без живого подключения.
pub fn alerts_scan(
    store: &dyn Store,
    rules: &[AlertRuleInput],
    from_ts: i64,
    to_ts: i64,
) -> Result<Vec<AlertEventDto>, StorageError> {
    let domain_rules: Vec<_> = rules.iter().filter_map(AlertRuleInput::to_rule).collect();
    if domain_rules.is_empty() {
        return Ok(Vec::new());
    }

    // Уникальные символы из правил → наблюдения по дневным барам.
    let mut symbols: Vec<&str> = domain_rules.iter().map(|r| r.symbol.as_str()).collect();
    symbols.sort_unstable();
    symbols.dedup();

    // (ts, symbol, observation) по всем инструментам, далее сортируем по ts.
    let mut feed: Vec<(i64, String, Observation)> = Vec::new();
    for sym in symbols {
        for bar in store.bars(sym, TimeFrame::D1, from_ts, to_ts)? {
            let change = if bar.open != 0.0 {
                (bar.close - bar.open) / bar.open
            } else {
                0.0
            };
            feed.push((
                bar.ts,
                sym.to_string(),
                Observation {
                    ts: bar.ts,
                    price: bar.close,
                    change,
                },
            ));
        }
    }
    feed.sort_by_key(|a| a.0);

    let mut engine = AlertEngine::new(domain_rules);
    let mut out = Vec::new();
    for (_, sym, obs) in &feed {
        for ev in engine.observe(sym, obs) {
            out.push(AlertEventDto::from(&ev));
        }
    }
    Ok(out)
}

// ── V2 / Бэктестер ─────────────────────────────────────────────────────────

/// Каталог встроенных стратегий с их параметрами (для пикера в UI).
pub fn list_strategies() -> Vec<StrategyDescriptorDto> {
    descriptors().iter().map(StrategyDescriptorDto::from).collect()
}

/// Прогнать бэктест стратегии по сохранённым барам инструмента.
///
/// Грузит бары через [`Store::bars`], собирает стратегию из библиотеки по `id`
/// и параметрам, прогоняет чистый движок [`domain::backtest::run_backtest`] и
/// возвращает отчёт (сделки/кривая капитала/метрики). Неизвестный `id` →
/// ошибка.
#[allow(clippy::too_many_arguments)]
pub fn run_backtest(
    store: &dyn Store,
    symbol: &str,
    tf: TimeFrame,
    from_ts: i64,
    to_ts: i64,
    strategy_id: &str,
    params: &StrategyParams,
    config: &BacktestConfigInput,
) -> Result<BacktestReportDto, StorageError> {
    let bars = store.bars(symbol, tf, from_ts, to_ts)?;
    let mut strategy = strategy_from_id(strategy_id, params)
        .ok_or_else(|| StorageError::Db(format!("неизвестная стратегия: {strategy_id}")))?;
    let report = domain_run_backtest(&bars, strategy.as_mut(), config.to_config());
    Ok(BacktestReportDto::from(&report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{AssetClass, Bar, Instrument};
    use storage::ingest::Writer;
    use storage::MemStore;

    fn inst(symbol: &str, sector: Option<&str>) -> Instrument {
        inst_of(symbol, sector, AssetClass::Equity)
    }

    fn inst_of(symbol: &str, sector: Option<&str>, asset_class: AssetClass) -> Instrument {
        Instrument {
            symbol: symbol.into(),
            ticker: symbol.split('@').next().unwrap().into(),
            name: symbol.into(),
            asset_class,
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
        for (sym, base) in [
            ("SBER@MISX", 100.0),
            ("LKOH@MISX", 50.0),
            ("GAZP@MISX", 200.0),
        ] {
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
        store
            .upsert_instruments(&[inst("ZZZZ@MISX", None)])
            .unwrap();
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
    fn top_movers_sorts_by_absolute_change_and_respects_limit() {
        let store = seeded();
        let all = top_movers(&store, 0, 9, None).unwrap();
        // 3 инструмента со снимками (XXXX без снимка не попадает).
        assert_eq!(all.len(), 3);
        // Отсортировано по убыванию |изменения|.
        assert!(all
            .windows(2)
            .all(|w| w[0].change.abs() >= w[1].change.abs()));
        // Цена закрытия проброшена из последнего бара.
        assert!(all.iter().all(|m| m.last_close > 0.0));

        // limit усекает выдачу.
        let top1 = top_movers(&store, 0, 9, Some(1)).unwrap();
        assert_eq!(top1.len(), 1);
        assert_eq!(top1[0].symbol, all[0].symbol);
    }

    #[test]
    fn rrg_sectors_assigns_valid_quadrants() {
        let store = seeded();
        let rrg = rrg_sectors(&store, 0, 9).unwrap();
        assert!(!rrg.is_empty());
        for point in &rrg {
            assert!(
                ["leading", "weakening", "lagging", "improving"].contains(&point.quadrant.as_str())
            );
        }
    }

    #[test]
    fn rrg_sectors_empty_when_no_snapshots() {
        let mut store = MemStore::new();
        store.migrate().unwrap();
        assert!(rrg_sectors(&store, 0, 9).unwrap().is_empty());
    }

    /// Хранилище с одним фьючерсом и одной облигацией (плюс снимки).
    fn seeded_mixed() -> MemStore {
        let mut store = MemStore::new();
        store.migrate().unwrap();
        store
            .upsert_instruments(&[
                inst("SBER@MISX", Some("Финансы")),
                inst_of("SiH5@RTSX", None, AssetClass::Future),
                inst_of("SiM5@RTSX", None, AssetClass::Future),
                inst_of("SU26240@MISX", None, AssetClass::Bond),
            ])
            .unwrap();

        let mut w = Writer::new(&mut store);
        for (sym, base) in [
            ("SBER@MISX", 100.0),
            ("SiH5@RTSX", 90_000.0),
            ("SiM5@RTSX", 91_000.0),
            ("SU26240@MISX", 800.0),
        ] {
            let bars = [
                bar(1, base, base * 1.01, 1_000.0),
                bar(2, base * 1.01, base * 1.02, 1_000.0),
            ];
            w.bars(sym, TimeFrame::D1, &bars).unwrap();
            w.snapshot_from_bars(sym, &bars, 2).unwrap();
        }
        store
    }

    #[test]
    fn instruments_by_asset_class_filters() {
        let store = seeded_mixed();
        assert_eq!(store.instruments_by_asset_class("future").unwrap().len(), 2);
        assert_eq!(store.instruments_by_asset_class("bond").unwrap().len(), 1);
        assert_eq!(store.instruments_by_asset_class("equity").unwrap().len(), 1);
    }

    #[test]
    fn futures_rollup_groups_by_prefix_and_excludes_other_classes() {
        let store = seeded_mixed();
        let rows = futures_rollup(&store, 0, 9).unwrap();
        // SiH5 + SiM5 → одна группа "SI"; облигация/акция не попадают.
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].group, "SI");
        assert_eq!(rows[0].contracts, 2);
        assert!(rows[0].turnover > 0.0);
    }

    #[test]
    fn bonds_rollup_groups_by_issuer_with_no_fabricated_yield() {
        let store = seeded_mixed();
        let rows = bonds_rollup(&store, 0, 9).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].issuer, "SU2"); // 3-символьный префикс SU26240
        assert_eq!(rows[0].bonds, 1);
        // Доходность/дюрация не фабрикуются (нет источника данных).
        assert_eq!(rows[0].avg_yield, 0.0);
        assert_eq!(rows[0].weighted_duration, 0.0);
    }

    #[test]
    fn yield_curve_is_sorted_by_maturity() {
        let curve = yield_curve().unwrap();
        assert!(!curve.is_empty());
        assert!(curve
            .windows(2)
            .all(|w| w[0].maturity_years < w[1].maturity_years));
    }

    #[test]
    fn char_prefix_handles_multibyte_without_panic() {
        // Кириллица: байтовый срез [..3] разрезал бы середину символа и паниковал.
        assert_eq!(char_prefix("ОФЗ26240", 3), "ОФЗ");
        assert_eq!(char_prefix("Si", 5), "SI"); // короче запрошенного — без паники
    }

    // ── Фаза 6 — «Сумма всех» ──────────────────────────────────────────────

    #[test]
    fn cross_asset_summary_aggregates_all_classes() {
        let store = seeded_mixed();
        let s = cross_asset_summary(&store, 0, 9).unwrap();
        // Все три класса присутствуют (с нулевыми долями допустимо).
        assert_eq!(s.shares.len(), 3);
        let codes: Vec<&str> = s.shares.iter().map(|r| r.asset_class.as_str()).collect();
        assert!(codes.contains(&"equity"));
        assert!(codes.contains(&"future"));
        assert!(codes.contains(&"bond"));
        // Общий оборот = сумма оборотов классов; доли суммируются в 1.
        assert!(s.total > 0.0);
        let share_sum: f64 = s.shares.iter().map(|r| r.share).sum();
        assert!((share_sum - 1.0).abs() < 1e-9);
        // Фьючерсы (два контракта) дают наибольший оборот.
        let fut = s.shares.iter().find(|r| r.asset_class == "future").unwrap();
        assert!(fut.share > 0.0);
    }

    #[test]
    fn turnover_timeline_groups_by_ts_and_class() {
        let store = seeded_mixed();
        let tl = turnover_timeline(&store, 0, 9).unwrap();
        // Снимки только на ts=2 → одна точка.
        assert_eq!(tl.len(), 1);
        assert_eq!(tl[0].ts, 2);
        assert!(tl[0].equity > 0.0);
        assert!(tl[0].future > 0.0);
        assert!(tl[0].bond > 0.0);
    }

    #[test]
    fn alerts_scan_fires_on_stored_bars() {
        use crate::dto::AlertRuleInput;
        let store = seeded(); // SBER close растёт 101→102→103 от base=100
        let rules = vec![AlertRuleInput {
            symbol: "SBER@MISX".into(),
            kind: "priceAbove".into(),
            threshold: 102.0,
        }];
        let events = alerts_scan(&store, &rules, 0, 9).unwrap();
        // Бар ts=3 закрывается на 103 (>102) → одно срабатывание по фронту.
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].symbol, "SBER@MISX");
        assert!(events[0].message.contains("выше"));
    }

    #[test]
    fn alerts_scan_ignores_unknown_kind_and_empty_rules() {
        use crate::dto::AlertRuleInput;
        let store = seeded();
        assert!(alerts_scan(&store, &[], 0, 9).unwrap().is_empty());
        let bogus = vec![AlertRuleInput {
            symbol: "SBER@MISX".into(),
            kind: "nonsense".into(),
            threshold: 1.0,
        }];
        assert!(alerts_scan(&store, &bogus, 0, 9).unwrap().is_empty());
    }

    #[test]
    fn list_strategies_exposes_builtin_library() {
        let strategies = list_strategies();
        assert!(!strategies.is_empty());
        let ids: Vec<&str> = strategies.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"ma_cross"));
        assert!(ids.contains(&"same_lot"));
        // у каждой стратегии есть параметры со значениями по умолчанию
        assert!(strategies.iter().all(|s| !s.params.is_empty()));
    }

    #[test]
    fn run_backtest_reports_trades_and_rejects_unknown_strategy() {
        let store = seeded(); // SBER/LKOH/GAZP с дневными барами
        let cfg = BacktestConfigInput {
            initial_capital: 100_000.0,
            commission: 0.0,
            slippage: 0.0,
            fill_timing: None,
        };
        let params = StrategyParams::new();
        let report = run_backtest(
            &store,
            "SBER@MISX",
            TimeFrame::D1,
            0,
            i64::MAX,
            "same_lot",
            &params,
            &cfg,
        )
        .unwrap();
        // кривая капитала строится по барам инструмента
        assert!(!report.equity_curve.is_empty());

        // неизвестная стратегия → ошибка
        assert!(run_backtest(
            &store,
            "SBER@MISX",
            TimeFrame::D1,
            0,
            i64::MAX,
            "does_not_exist",
            &params,
            &cfg,
        )
        .is_err());
    }

    #[test]
    fn flow_sankey_empty_with_single_period() {
        let store = seeded_mixed();
        assert!(flow_sankey(&store, 0, 9).unwrap().is_empty());
    }

    #[test]
    fn flow_sankey_detects_share_shift_between_periods() {
        use storage::store::TurnoverSnapshot;

        // Два периода: доля смещается из акций в фьючерсы.
        let mut store = MemStore::new();
        store.migrate().unwrap();
        store
            .upsert_instruments(&[
                inst("SBER@MISX", Some("Финансы")),
                inst_of("SiH5@RTSX", None, AssetClass::Future),
            ])
            .unwrap();

        let snap = |ts, turnover| TurnoverSnapshot {
            ts,
            turnover,
            net_flow: 0.0,
            change: 0.0,
        };
        // Период 1 (ts=1): акции доминируют (0.9 доли).
        store.insert_snapshot("SBER@MISX", &snap(1, 900.0)).unwrap();
        store.insert_snapshot("SiH5@RTSX", &snap(1, 100.0)).unwrap();
        // Период 2 (ts=2): фьючерс перетянул долю (0.9).
        store.insert_snapshot("SBER@MISX", &snap(2, 100.0)).unwrap();
        store.insert_snapshot("SiH5@RTSX", &snap(2, 900.0)).unwrap();

        let edges = flow_sankey(&store, 0, 9).unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from, "equity");
        assert_eq!(edges[0].to, "future");
        // Доля сместилась на 0.8 (с 0.9 до 0.1 у акций).
        assert!((edges[0].weight - 0.8).abs() < 1e-9);
    }
}
