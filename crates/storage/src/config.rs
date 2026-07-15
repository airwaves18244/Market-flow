//! Конфигурация путей хранилища: директория данных для файловой БД истории
//! (фаза 11.2.5).
//!
//! По умолчанию — ОС-директория данных (переопределяемая через окружение или
//! явным аргументом из настроек приложения). Без внешних зависимостей: путь
//! резолвится по переменным окружения платформы (`XDG_DATA_HOME`/`APPDATA`/
//! `HOME`), а не через крейт `directories`, чтобы `storage` оставался
//! кросс-платформенно собираемым без сетевых/системных зависимостей.

use std::path::{Path, PathBuf};

/// Имя файла основной БД DuckDB в директории данных.
pub const DB_FILE_NAME: &str = "market-flow.duckdb";

/// Имя подкаталога приложения в ОС-директории данных. Единый нейминг с
/// каталогом настроек (`app::settings`, `APP_DIR_NAME = "market-terminal"`).
const APP_DIR: &str = "market-terminal";

/// Переменная окружения для явного переопределения директории данных. Парная к
/// `MARKET_TERMINAL_CONFIG_DIR` из `app::settings`.
pub const DATA_DIR_ENV: &str = "MARKET_TERMINAL_DATA_DIR";

/// Директория данных по умолчанию.
///
/// Приоритет: `MARKET_TERMINAL_DATA_DIR` → ОС-директория данных пользователя
/// (`%APPDATA%`/`$XDG_DATA_HOME`/`~/Library/Application Support`/
/// `~/.local/share`) с подкаталогом `market-terminal` → относительный
/// `market-terminal-data` как крайний фолбэк, если окружение не задано.
pub fn default_data_dir() -> PathBuf {
    if let Some(explicit) = std::env::var_os(DATA_DIR_ENV) {
        if !explicit.is_empty() {
            return PathBuf::from(explicit);
        }
    }
    if let Some(dir) = os_data_dir() {
        return dir.join(APP_DIR);
    }
    PathBuf::from("market-terminal-data")
}

/// ОС-директория данных пользователя (без подкаталога приложения).
#[cfg(target_os = "windows")]
fn os_data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

/// ОС-директория данных пользователя (без подкаталога приложения).
#[cfg(not(target_os = "windows"))]
fn os_data_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(xdg));
    }
    let home = std::env::var_os("HOME").filter(|s| !s.is_empty())?;
    let home = PathBuf::from(home);
    #[cfg(target_os = "macos")]
    {
        Some(home.join("Library").join("Application Support"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Some(home.join(".local").join("share"))
    }
}

/// Разрешить директорию данных: явный путь из настроек либо
/// [`default_data_dir`].
pub fn resolve_data_dir(explicit: Option<PathBuf>) -> PathBuf {
    explicit
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(default_data_dir)
}

/// Путь к файлу БД в заданной директории данных.
pub fn db_path_in(dir: impl AsRef<Path>) -> PathBuf {
    dir.as_ref().join(DB_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_dir_wins_over_default() {
        let explicit = PathBuf::from("/custom/data");
        assert_eq!(resolve_data_dir(Some(explicit.clone())), explicit);
    }

    #[test]
    fn empty_explicit_falls_back_to_default() {
        assert_eq!(resolve_data_dir(Some(PathBuf::new())), default_data_dir());
    }

    #[test]
    fn default_dir_is_non_empty() {
        assert!(!default_data_dir().as_os_str().is_empty());
    }

    #[test]
    fn db_path_appends_file_name() {
        let p = db_path_in("/data");
        assert!(p.ends_with(DB_FILE_NAME));
        assert_eq!(p, Path::new("/data").join(DB_FILE_NAME));
    }
}
