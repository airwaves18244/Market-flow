//! Точка входа десктопного терминала.
//!
//! ## Статус: smoke-каркас (Фазы 0–2)
//!
//! В фазе UI здесь поднимается Tauri-приложение: регистрируются IPC-команды
//! (запрос снимков и временных рядов), события (live-push котировок во фронт) и
//! асинхронный планировщик батч-ингеста под лимиты API. Пока это консольная
//! точка входа, которая прогоняет реальный путь данных через слои
//! `domain`/`storage`: миграция → ингест баров → снимок оборота → запрос.

use domain::{AssetClass, Bar, TimeFrame};
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("market terminal — каркас (Фаза 1: хранилище и ингест)");
    println!("Классы активов: {:?}", AssetClass::ALL);
    println!("Эндпоинт API: {}", finam_proto::ENDPOINT);
    println!("Таблиц в схеме DuckDB: {}", schema::ALL_DDL.len());

    // Демонстрация связности слоёв на in-memory хранилище. В продакшен-сборке
    // вместо MemStore подключается storage::DuckStore (фича `duckdb`).
    let mut store = MemStore::new();
    store.migrate()?;
    println!("Версия схемы: {:?}", store.schema_version()?);

    let symbol = "SBER@MISX";
    let bars = [
        demo_bar(1, 300.0, 305.0, 1_000.0),
        demo_bar(2, 305.0, 303.0, 800.0),
        demo_bar(3, 303.0, 310.0, 1_500.0),
    ];

    let mut writer = Writer::new(&mut store);
    let n = writer.bars(symbol, TimeFrame::D1, &bars)?;
    writer.snapshot_from_bars(symbol, &bars, 3)?;

    println!("Записано баров {symbol}: {n}");
    let stored = store.bars(symbol, TimeFrame::D1, 0, i64::MAX)?;
    println!("Прочитано баров из хранилища: {}", stored.len());
    if let Some(snap) = store.snapshots(symbol, 0, i64::MAX)?.first() {
        println!(
            "Снимок оборота: turnover={:.0} net_flow={:.0} change={:+.2}%",
            snap.turnover,
            snap.net_flow,
            snap.change * 100.0
        );
    }

    Ok(())
}
