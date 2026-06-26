//! Точка входа десктопного терминала.
//!
//! ## Статус: каркас Tauri (Фаза 3)
//!
//! Ядро IPC ([`api`]/[`dto`]/[`state`]) реализовано и протестировано на
//! `MemStore`. Привязка к Tauri (команды, события, билдер) живёт в модуле
//! [`tauri_app`] за фичей `tauri` — её сборка требует десктопного окружения
//! (webkit2gtk), поэтому по умолчанию выключена и не ломает кросс-платформенный
//! CI. Без фичи `tauri` бинарь работает как консольный smoke, прогоняющий путь
//! данных `domain` → `storage` → `dto`.

mod api;
mod dto;
mod state;

#[cfg(feature = "tauri")]
mod tauri_app;

use domain::{AssetClass, Bar, TimeFrame};
use state::AppState;
use storage::ingest::Writer;
use storage::{schema, MemStore, Store};

fn demo_bar(ts: i64, open: f64, close: f64, volume: f64) -> Bar {
    Bar {
        ts,
        open,
        high: open.max(close),
        low: open.min(close),
        close,
        volume,
    }
}

/// Наполнить хранилище демонстрационными данными (для smoke без живого API).
fn seed_demo_store() -> Result<MemStore, Box<dyn std::error::Error>> {
    use domain::Instrument;

    let mut store = MemStore::new();
    store.migrate()?;
    store.upsert_instruments(&[
        Instrument {
            symbol: "SBER@MISX".into(),
            ticker: "SBER".into(),
            name: "Сбербанк".into(),
            asset_class: AssetClass::Equity,
            sector: Some("Финансы".into()),
            lot_size: 10,
            isin: Some("RU0009029540".into()),
        },
        Instrument {
            symbol: "LKOH@MISX".into(),
            ticker: "LKOH".into(),
            name: "Лукойл".into(),
            asset_class: AssetClass::Equity,
            sector: Some("Нефтегаз".into()),
            lot_size: 1,
            isin: None,
        },
    ])?;

    let mut w = Writer::new(&mut store);
    w.load_sector_map([("SBER", "Финансы"), ("LKOH", "Нефтегаз")])?;
    for (sym, base) in [("SBER@MISX", 300.0), ("LKOH@MISX", 7000.0)] {
        let bars = [
            demo_bar(1, base, base * 1.01, 1_000.0),
            demo_bar(2, base * 1.01, base * 0.999, 900.0),
            demo_bar(3, base * 0.999, base * 1.02, 1_500.0),
        ];
        w.bars(sym, TimeFrame::D1, &bars)?;
        w.snapshot_from_bars(sym, &bars, 3)?;
    }
    Ok(store)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "tauri")]
    {
        tauri_app::run();
        return Ok(());
    }

    #[cfg(not(feature = "tauri"))]
    {
        println!("market terminal — каркас (Фаза 3: Tauri-оболочка + IPC)");
        println!("Классы активов: {:?}", AssetClass::ALL);
        println!("Эндпоинт API: {}", finam_proto::ENDPOINT);
        println!("Таблиц в схеме DuckDB: {}", schema::ALL_DDL.len());

        let store = seed_demo_store()?;
        let state = AppState::new(store);

        println!("\nIPC-команды (демо на MemStore):");
        println!("  instruments(): {}", state.instruments()?.len());
        let rows = state.sector_rollup(0, i64::MAX)?;
        println!("  sector_rollup(): {} секторов", rows.len());
        for r in &rows {
            println!(
                "    {:<10} turnover={:>12.0} change={:+.2}%",
                r.sector,
                r.turnover,
                r.weighted_change * 100.0
            );
        }
        let series = state.turnover_series("SBER@MISX", 0, i64::MAX)?;
        println!("  turnover_series(SBER@MISX): {} точек", series.len());
        let candles = state.bars("SBER@MISX", TimeFrame::D1, 0, i64::MAX)?;
        println!("  bars(SBER@MISX, d1): {} свечей", candles.len());
        println!("  sector_map(): {} записей", state.sector_map()?.len());

        Ok(())
    }
}
