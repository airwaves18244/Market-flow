//! Идемпотентные миграции схемы.
//!
//! «Источник правды» по структуре — DDL в [`crate::schema`]. Миграция
//! применяет таблицу версии и все DDL (каждый — `CREATE TABLE IF NOT EXISTS`,
//! т.е. безопасен к повторному запуску), затем фиксирует
//! [`schema::SCHEMA_VERSION`]. Логика версий вынесена сюда отдельно от движка,
//! чтобы её можно было тестировать без нативной библиотеки DuckDB.

use crate::schema::{self, SCHEMA_VERSION};

/// SQL-операторы миграции в порядке применения: сначала таблица версии,
/// затем таблицы данных. Все операторы идемпотентны.
pub fn statements() -> Vec<&'static str> {
    let mut out = Vec::with_capacity(schema::ALL_DDL.len() + 1);
    out.push(schema::DDL_SCHEMA_VERSION);
    out.extend_from_slice(&schema::ALL_DDL);
    out
}

/// Нужно ли применять миграцию при текущей версии БД.
///
/// `current` — версия, прочитанная из таблицы `schema_version` (`None` — БД
/// новая/пустая). Возвращает `true`, если код новее, чем БД.
pub fn pending(current: Option<i32>) -> bool {
    match current {
        Some(v) => v < SCHEMA_VERSION,
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_start_with_version_table_then_all_ddl() {
        let s = statements();
        assert_eq!(s.len(), schema::ALL_DDL.len() + 1);
        assert!(s[0].contains("schema_version"));
        assert!(s[1..]
            .iter()
            .all(|q| q.contains("CREATE TABLE IF NOT EXISTS")));
    }

    #[test]
    fn pending_is_true_for_fresh_or_older_db() {
        assert!(pending(None));
        assert!(pending(Some(SCHEMA_VERSION - 1)));
        assert!(!pending(Some(SCHEMA_VERSION)));
        // БД новее кода (откат версии приложения) — миграцию не навязываем
        assert!(!pending(Some(SCHEMA_VERSION + 1)));
    }
}
