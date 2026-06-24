//! Версионированные миграции схемы.
//!
//! Применение идемпотентно: уже накатанные версии пропускаются. Каждая версия
//! выполняется атомарно (в одной транзакции вместе с отметкой в
//! `schema_migrations`), поэтому прерванный запуск не оставляет схему «между»
//! версиями.

use std::time::{SystemTime, UNIX_EPOCH};

use duckdb::Connection;

use crate::{schema, Result, StorageError};

/// Журнал применённых миграций.
const DDL_MIGRATIONS: &str = "\
CREATE TABLE IF NOT EXISTS schema_migrations (
    version    BIGINT PRIMARY KEY,
    applied_at BIGINT NOT NULL
);";

/// Одна миграция: версия и набор SQL-операторов в порядке выполнения.
pub struct Migration {
    pub version: u32,
    pub statements: &'static [&'static str],
}

/// Полный список миграций в порядке возрастания версий.
///
/// Версия 1 — базовая схема (`schema::ALL_DDL`). Новые версии **добавляются в
/// конец**; уже выпущенные миграции не редактируются.
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    statements: &schema::ALL_DDL,
}];

/// Применить все недостающие миграции. Возвращает число накатанных версий.
pub fn apply(conn: &Connection) -> Result<u32> {
    conn.execute_batch(DDL_MIGRATIONS)?;
    let current = current_version(conn)?;

    // Версии должны идти строго по возрастанию — страховка от опечаток в списке.
    let mut prev = 0u32;
    for m in MIGRATIONS {
        if m.version <= prev {
            return Err(StorageError::Migration(format!(
                "версии миграций не упорядочены: {} после {}",
                m.version, prev
            )));
        }
        prev = m.version;
    }

    let mut applied = 0u32;
    for m in MIGRATIONS {
        if m.version <= current {
            continue;
        }
        let mut sql = String::from("BEGIN TRANSACTION;\n");
        for stmt in m.statements {
            sql.push_str(stmt);
            sql.push('\n');
        }
        sql.push_str(&format!(
            "INSERT INTO schema_migrations(version, applied_at) VALUES ({}, {});\n",
            m.version,
            now_unix()
        ));
        sql.push_str("COMMIT;");
        conn.execute_batch(&sql)?;
        applied += 1;
    }
    Ok(applied)
}

/// Максимальная применённая версия (0, если миграций ещё не было).
pub fn current_version(conn: &Connection) -> Result<u32> {
    conn.execute_batch(DDL_MIGRATIONS)?;
    let v: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;
    Ok(v as u32)
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
