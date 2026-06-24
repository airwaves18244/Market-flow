//! Интеграционные тесты слоя хранилища против реального движка DuckDB
//! (в памяти). Покрывают идемпотентность ингеста, классификацию секторов и
//! аналитические запросы (§ 1.1–1.3, § 1.5, § 1.7).

use domain::{AssetClass, Bar, Instrument};
use storage::ingest::{SymbolSnapshot, TurnoverSnapshot};
use storage::{Db, TimeFrame};

fn instrument(symbol: &str, ticker: &str, class: AssetClass, isin: Option<&str>) -> Instrument {
    Instrument {
        symbol: symbol.to_string(),
        ticker: ticker.to_string(),
        name: format!("{ticker} name"),
        asset_class: class,
        sector: None,
        lot_size: 10,
        isin: isin.map(str::to_string),
    }
}

fn bar(ts: i64, close: f64, volume: f64) -> Bar {
    Bar {
        ts,
        open: close - 1.0,
        high: close + 1.0,
        low: close - 2.0,
        close,
        volume,
    }
}

fn seed() -> Db {
    let db = Db::open_in_memory().unwrap();
    let instruments = [
        instrument("SBER@MISX", "SBER", AssetClass::Equity, Some("RU0009029540")),
        instrument("GAZP@MISX", "GAZP", AssetClass::Equity, None),
        instrument("SiM5@RTSX", "SiM5", AssetClass::Future, None),
    ];
    assert_eq!(db.upsert_instruments(&instruments).unwrap(), 3);
    db
}

#[test]
fn instrument_upsert_is_idempotent() {
    let db = seed();
    // Повторный upsert тех же символов не плодит строки.
    let again = [instrument("SBER@MISX", "SBER", AssetClass::Equity, None)];
    db.upsert_instruments(&again).unwrap();
    let n: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM instruments", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 3);
}

#[test]
fn bars_batch_insert_is_idempotent_on_pk() {
    let db = seed();
    let bars = [bar(1000, 100.0, 5.0), bar(2000, 101.0, 6.0)];
    db.insert_bars("SBER@MISX", TimeFrame::D1, &bars).unwrap();
    // Повторная вставка тех же баров (PK symbol+timeframe+ts) — без дублей,
    // значения обновляются.
    let updated = [bar(1000, 150.0, 9.0), bar(2000, 101.0, 6.0)];
    db.insert_bars("SBER@MISX", TimeFrame::D1, &updated).unwrap();

    let n: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM bars", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 2);

    let close: f64 = db
        .conn()
        .query_row(
            "SELECT close FROM bars WHERE symbol='SBER@MISX' AND ts=1000",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(close, 150.0);
}

#[test]
fn sector_map_classifies_by_ticker_and_isin() {
    let db = seed();
    // SBER — по тикеру; GAZP — нет совпадения; Газпром-ISIN не используем здесь.
    db.upsert_sector_map([("SBER", "Финансы"), ("RU0007661625", "Энергетика")])
        .unwrap();
    let updated = db.apply_sectors_to_instruments().unwrap();
    assert!(updated >= 1);

    let sber: Option<String> = db
        .conn()
        .query_row(
            "SELECT sector FROM instruments WHERE symbol='SBER@MISX'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(sber.as_deref(), Some("Финансы"));

    // У GAZP нет совпадения ни по тикеру, ни по ISIN — сектор остаётся пустым.
    let gazp: Option<String> = db
        .conn()
        .query_row(
            "SELECT sector FROM instruments WHERE symbol='GAZP@MISX'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(gazp, None);
}

#[test]
fn ticker_match_takes_priority_over_isin() {
    let db = seed();
    // Для SBER зададим и тикер, и его ISIN с разными секторами.
    db.upsert_sector_map([("SBER", "Тикер-сектор"), ("RU0009029540", "ISIN-сектор")])
        .unwrap();
    db.apply_sectors_to_instruments().unwrap();
    let sber: Option<String> = db
        .conn()
        .query_row(
            "SELECT sector FROM instruments WHERE symbol='SBER@MISX'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(sber.as_deref(), Some("Тикер-сектор"));
}

#[test]
fn analytical_queries_aggregate_snapshots() {
    let db = seed();
    db.upsert_sector_map([("SBER", "Финансы"), ("GAZP", "Нефтегаз")])
        .unwrap();
    db.apply_sectors_to_instruments().unwrap();

    let snaps = [
        SymbolSnapshot {
            symbol: "SBER@MISX".into(),
            snapshot: TurnoverSnapshot::new(1000, 500.0, 120.0, 0.012),
        },
        SymbolSnapshot {
            symbol: "SBER@MISX".into(),
            snapshot: TurnoverSnapshot::new(2000, 800.0, -50.0, -0.004),
        },
        SymbolSnapshot {
            symbol: "GAZP@MISX".into(),
            snapshot: TurnoverSnapshot::new(1000, 300.0, 30.0, 0.030),
        },
    ];
    assert_eq!(db.insert_turnover_snapshots(&snaps).unwrap(), 3);

    // Оборот по секторам за весь период.
    let by_sector = db.turnover_by_sector(0, 10_000).unwrap();
    let fin = by_sector.iter().find(|s| s.sector == "Финансы").unwrap();
    assert_eq!(fin.turnover, 1300.0); // 500 + 800
    assert_eq!(fin.net_flow, 70.0); // 120 - 50
    // Финансы (1300) идут раньше Нефтегаза (300) — сортировка по обороту.
    assert_eq!(by_sector[0].sector, "Финансы");

    // Топ-движения: берётся последний снимок инструмента в периоде.
    let movers = db.top_movers(0, 10_000, 10).unwrap();
    let sber = movers.iter().find(|m| m.symbol == "SBER@MISX").unwrap();
    assert_eq!(sber.change, -0.004); // последний снимок (ts=2000)
    // По модулю изменения GAZP (0.030) лидирует над SBER (0.004).
    assert_eq!(movers[0].symbol, "GAZP@MISX");

    // Временной ряд нетто-потока SBER — по возрастанию времени.
    let series = db.net_flow_series("SBER@MISX", 0, 10_000).unwrap();
    assert_eq!(series.len(), 2);
    assert_eq!(series[0].ts, 1000);
    assert_eq!(series[0].net_flow, 120.0);
    assert_eq!(series[1].ts, 2000);
}
