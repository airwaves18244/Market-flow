//! Точка входа десктопного терминала.
//!
//! ## Статус: smoke-каркас (Фаза 1)
//!
//! В фазе UI здесь поднимается Tauri-приложение: регистрируются IPC-команды
//! (запрос снимков и временных рядов), события (live-push котировок во фронт) и
//! планировщик батч-ингеста под лимиты API. Пока это консольная точка входа,
//! прогоняющая ингест-поток через in-memory DuckDB и подтверждающая, что слои
//! `domain`/`data`/`storage` связываются и собираются.

use domain::{AssetClass, Bar, Instrument};
use storage::{schema, Store, TurnoverSnapshot};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("market terminal — каркас (Фаза 1)");
    println!("Классы активов: {:?}", AssetClass::ALL);
    println!("Эндпоинт API: {}", finam_proto::ENDPOINT);
    println!("Таблиц в схеме DuckDB: {}", schema::ALL_DDL.len());

    // Демонстрация сквозного потока: миграции → ингест → запрос.
    let store = Store::open_in_memory()?;
    println!("Версия схемы DuckDB: v{}", store.schema_version()?);

    store.upsert_instruments(&[Instrument {
        symbol: "SBER@MISX".into(),
        ticker: "SBER".into(),
        name: "Сбербанк".into(),
        asset_class: AssetClass::Equity,
        sector: Some("Финансы".into()),
        lot_size: 10,
        isin: Some("RU0009029540".into()),
    }])?;
    store.insert_bars(
        "SBER@MISX",
        "d1",
        &[Bar { ts: 1_700_000_000, open: 270.0, high: 275.0, low: 269.0, close: 274.0, volume: 1_000.0 }],
    )?;
    store.insert_turnover_snapshot(
        "SBER@MISX",
        TurnoverSnapshot { ts: 1_700_000_000, turnover: 274_000.0, net_flow: 4_000.0, change: 0.014 },
    )?;

    let by_sector = store.turnover_by_sector(0)?;
    println!("Оборот по секторам: {by_sector:?}");

    // Планирование бэкфилла: год часовых баров окнами по 30 дней под лимит API.
    let plan = data::backfill::plan_backfill(0, 365 * 86_400, 30 * 86_400, 200);
    println!("Бэкфилл: {} окон, мин. длительность {:?}", plan.windows, plan.min_duration);
    Ok(())
}
