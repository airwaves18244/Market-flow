//! Точка входа десктопного терминала.
//!
//! ## Статус: smoke-каркас (Фаза 0)
//!
//! В фазе UI здесь поднимается Tauri-приложение: регистрируются IPC-команды
//! (запрос снимков и временных рядов), события (live-push котировок во фронт) и
//! планировщик батч-ингеста под лимиты API. Пока это консольная точка входа,
//! подтверждающая, что слои `domain`/`data`/`storage` связываются и собираются.

use domain::AssetClass;
use storage::schema;

fn main() {
    println!("market terminal — каркас (Фаза 0)");
    println!("Классы активов: {:?}", AssetClass::ALL);
    println!("Эндпоинт API: {}", finam_proto::ENDPOINT);
    println!("Таблиц в схеме DuckDB: {}", schema::ALL_DDL.len());
}
