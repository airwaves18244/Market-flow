//! Резолвер директории данных терминала (фаза 11.2).
//!
//! Аналог `app::settings::resolve_config_dir_with`, но для данных (файл БД
//! DuckDB, Parquet-экспорты): чистая функция над источником переменных окружения
//! (внедряется параметром, поэтому тесты не трогают реальный `env`). Путь можно
//! переопределить целиком переменной [`DATA_DIR_ENV`]
//! (`MARKET_TERMINAL_DATA_DIR`) — для портейбл-режима, тестов и CI.

use std::path::PathBuf;

/// env-переменная для переопределения директории данных целиком.
pub const DATA_DIR_ENV: &str = "MARKET_TERMINAL_DATA_DIR";
/// Имя приложения — сегмент пути в стандартных ОС-каталогах.
const APP_DIR_NAME: &str = "market-terminal";
/// Имя файла БД DuckDB в директории данных.
const DB_FILE_NAME: &str = "history.duckdb";

/// Резолвер директории данных — чистая функция над источником переменных
/// окружения.
///
/// Приоритет: [`DATA_DIR_ENV`] (если задан и непуст) → стандартный ОС-каталог
/// данных для текущей платформы (Windows — `LOCALAPPDATA`/`APPDATA`; macOS —
/// `~/Library/Application Support`; Linux/unix — XDG `XDG_DATA_HOME` или
/// `~/.local/share`). Всегда тотальна: при отсутствии всех переменных
/// возвращает относительный фолбэк.
pub fn resolve_data_dir_with(get_env: impl Fn(&str) -> Option<String>) -> PathBuf {
    if let Some(dir) = get_env(DATA_DIR_ENV).filter(|s| !s.is_empty()) {
        return PathBuf::from(dir);
    }
    if cfg!(target_os = "windows") {
        for var in ["LOCALAPPDATA", "APPDATA"] {
            if let Some(base) = get_env(var).filter(|s| !s.is_empty()) {
                return PathBuf::from(base).join(APP_DIR_NAME);
            }
        }
        return PathBuf::from(APP_DIR_NAME);
    }
    if cfg!(target_os = "macos") {
        if let Some(home) = get_env("HOME").filter(|s| !s.is_empty()) {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join(APP_DIR_NAME);
        }
        return PathBuf::from(APP_DIR_NAME);
    }
    // Linux и прочие unix-подобные — XDG Base Directory Specification.
    if let Some(xdg) = get_env("XDG_DATA_HOME").filter(|s| !s.is_empty()) {
        return PathBuf::from(xdg).join(APP_DIR_NAME);
    }
    if let Some(home) = get_env("HOME").filter(|s| !s.is_empty()) {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join(APP_DIR_NAME);
    }
    PathBuf::from(APP_DIR_NAME)
}

/// Резолвер директории данных поверх реального `std::env` (боевой режим).
pub fn default_data_dir() -> PathBuf {
    resolve_data_dir_with(|k| std::env::var(k).ok())
}

/// Путь к файлу БД DuckDB в разрешённой директории данных (чистый вариант).
pub fn resolve_db_path_with(get_env: impl Fn(&str) -> Option<String>) -> PathBuf {
    resolve_data_dir_with(get_env).join(DB_FILE_NAME)
}

/// Путь к файлу БД DuckDB поверх реального `std::env` (боевой режим).
pub fn default_db_path() -> PathBuf {
    default_data_dir().join(DB_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| map.get(k).cloned()
    }

    #[test]
    fn override_env_wins_and_sets_db_file() {
        let dir = resolve_data_dir_with(env(&[(DATA_DIR_ENV, "/custom/data")]));
        assert_eq!(dir, PathBuf::from("/custom/data"));
        let db = resolve_db_path_with(env(&[(DATA_DIR_ENV, "/custom/data")]));
        assert_eq!(db, PathBuf::from("/custom/data/history.duckdb"));
    }

    #[test]
    fn empty_override_is_ignored() {
        // Пустое переопределение не считается заданным — уходим в ОС-каталог.
        let dir = resolve_data_dir_with(env(&[(DATA_DIR_ENV, "")]));
        assert!(dir.ends_with(APP_DIR_NAME));
    }

    #[test]
    fn resolver_is_total_without_any_env() {
        let dir = resolve_data_dir_with(|_| None);
        assert!(dir.ends_with(APP_DIR_NAME));
    }
}
