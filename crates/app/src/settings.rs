//! Персист пользовательских настроек и правил Key Activity в JSON-файл
//! ОС-config-директории (10.5.3 / S.2.2 / 10.8.* / 11.6.1 / 12.8.1).
//!
//! Раньше конфигурация терминала (отображение, паспорт MOEX ALGO, LLM,
//! источник данных, опционы, пользовательские правила Key Activity) жила
//! только в localStorage браузерного вебвью: терялась при переустановке и не
//! была видна другим окнам/процессам. Здесь — единый источник истины на
//! стороне ядра: один JSON-файл `settings.json` в стандартной ОС-директории
//! конфигурации.
//!
//! Секреты (ключ LLM-провайдера, токен ALGOPACK) сюда НЕ попадают — только
//! несекретные поля и флаг «секрет задан» ([`crate::dto::SettingsDto::llm_has_key`]).
//! Сами секреты живут в ОС-keyring/`.env` (`data::SecretStore`).
//!
//! Путь резолвится вручную на `std` (без `dirs`/`directories` — в проекте
//! дисциплина минимальных зависимостей):
//! - Windows: `%APPDATA%\market-terminal\`;
//! - macOS: `~/Library/Application Support/market-terminal/`;
//! - Linux/прочие unix: `$XDG_CONFIG_HOME/market-terminal/` (фолбэк —
//!   `~/.config/market-terminal/`).
//!
//! Переопределяется переменной окружения [`CONFIG_DIR_ENV`] (тесты, портейбл-
//! режим). Резолвер [`resolve_config_dir_with`] — чистая функция: принимает
//! источник переменных окружения параметром и не трогает диск, поэтому тесты
//! не мутируют реальный `std::env` процесса.
//!
//! Запись атомарная: документ пишется во временный файл рядом и
//! переименовывается (`rename` на одной файловой системе — атомарная операция
//! ОС), поэтому падение процесса посреди записи не может испортить уже
//! существующий файл — читатель либо увидит старую версию, либо новую целиком.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use domain::keyactivity::Rule as KeyActivityRule;

use crate::dto::SettingsDto;

/// Имя файла конфигурации в config-директории.
const SETTINGS_FILE: &str = "settings.json";
/// Имя приложения — сегмент пути в стандартных ОС-каталогах конфигурации.
const APP_DIR_NAME: &str = "market-terminal";
/// env-переменная для переопределения config-директории целиком (тесты/CI/портейбл-режим).
pub const CONFIG_DIR_ENV: &str = "MARKET_TERMINAL_CONFIG_DIR";

/// Резолвер config-директории — чистая функция над источником переменных
/// окружения (внедряется параметром, поэтому тесты не трогают реальный `env`).
///
/// Приоритет: [`CONFIG_DIR_ENV`] (если задан и непуст) → стандартный
/// ОС-каталог для текущей целевой платформы.
pub fn resolve_config_dir_with(get_env: impl Fn(&str) -> Option<String>) -> PathBuf {
    if let Some(dir) = get_env(CONFIG_DIR_ENV).filter(|s| !s.is_empty()) {
        return PathBuf::from(dir);
    }
    if cfg!(target_os = "windows") {
        if let Some(appdata) = get_env("APPDATA").filter(|s| !s.is_empty()) {
            return PathBuf::from(appdata).join(APP_DIR_NAME);
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
    if let Some(xdg) = get_env("XDG_CONFIG_HOME").filter(|s| !s.is_empty()) {
        return PathBuf::from(xdg).join(APP_DIR_NAME);
    }
    if let Some(home) = get_env("HOME").filter(|s| !s.is_empty()) {
        return PathBuf::from(home).join(".config").join(APP_DIR_NAME);
    }
    // Нет ни XDG_CONFIG_HOME, ни HOME (нетипично) — относительный фолбэк,
    // чтобы резолвер оставался тотальной функцией.
    PathBuf::from(APP_DIR_NAME)
}

/// Резолвер поверх реального `std::env` (боевой режим).
pub fn default_config_dir() -> PathBuf {
    resolve_config_dir_with(|k| std::env::var(k).ok())
}

/// Содержимое файла конфигурации целиком: настройки + пользовательские
/// правила Key Activity. Раздельные сеттеры ([`SettingsStore::set_settings`]/
/// [`SettingsStore::set_key_activity_rules`]) читают файл, подменяют свою
/// секцию и пишут назад — изменение одной секции не затирает другую.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ConfigFile {
    settings: SettingsDto,
    key_activity_rules: Vec<KeyActivityRule>,
}

/// Хранилище конфигурации поверх JSON-файла в config-директории ОС.
pub struct SettingsStore {
    dir: PathBuf,
}

impl SettingsStore {
    /// Хранилище над явно заданной директорией (тесты, портейбл-режим).
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Хранилище над стандартной ОС-директорией (с учётом [`CONFIG_DIR_ENV`]).
    pub fn from_env() -> Self {
        Self::new(default_config_dir())
    }

    fn settings_path(&self) -> PathBuf {
        self.dir.join(SETTINGS_FILE)
    }

    /// Прочитать файл конфигурации. Отсутствующий файл или битый JSON — не
    /// ошибка: трактуются как дефолты (терминал не должен падать из-за
    /// отсутствующего/повреждённого файла настроек при первом запуске).
    fn read(&self) -> ConfigFile {
        fs::read_to_string(self.settings_path())
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    /// Атомарно записать файл конфигурации: временный файл рядом + `rename`.
    fn write(&self, doc: &ConfigFile) -> Result<(), String> {
        fs::create_dir_all(&self.dir)
            .map_err(|e| format!("не удалось создать директорию конфигурации: {e}"))?;
        let json = serde_json::to_string_pretty(doc)
            .map_err(|e| format!("не удалось сериализовать настройки: {e}"))?;
        let tmp_path = self.dir.join(format!("{SETTINGS_FILE}.tmp"));
        fs::write(&tmp_path, json)
            .map_err(|e| format!("не удалось записать временный файл настроек: {e}"))?;
        fs::rename(&tmp_path, self.settings_path())
            .map_err(|e| format!("не удалось завершить атомарную запись настроек: {e}"))?;
        Ok(())
    }

    /// Текущие пользовательские настройки (дефолты, если файла ещё нет).
    pub fn get_settings(&self) -> SettingsDto {
        self.read().settings
    }

    /// Сохранить настройки (уже провалидированные вызывающей стороной, см.
    /// [`crate::api::settings_set`]). Правила Key Activity не затрагиваются.
    pub fn set_settings(&self, doc: SettingsDto) -> Result<(), String> {
        let mut cfg = self.read();
        cfg.settings = doc;
        self.write(&cfg)
    }

    /// Сохранённые пользователем правила Key Activity (пусто, если
    /// пользователь ещё не сохранял свой набор — тогда UI использует
    /// встроенные дефолты, см. [`crate::api::key_activity_rules`]).
    pub fn get_key_activity_rules(&self) -> Vec<KeyActivityRule> {
        self.read().key_activity_rules
    }

    /// Сохранить уже провалидированные правила Key Activity (см.
    /// [`crate::api::key_activity_rules_set`]). Настройки не затрагиваются.
    pub fn set_key_activity_rules(&self, rules: Vec<KeyActivityRule>) -> Result<(), String> {
        let mut cfg = self.read();
        cfg.key_activity_rules = rules;
        self.write(&cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::keyactivity::{Comparator, Expr, Metric, Scope};
    use std::collections::HashMap;

    /// Изолированная временная директория для теста (не трогает реальный
    /// пользовательский config-каталог). Удаляется при drop.
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "market-terminal-settings-test-{tag}-{}-{:?}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sample_rule(id: &str, weight: f64) -> KeyActivityRule {
        KeyActivityRule {
            id: id.to_string(),
            name: format!("Правило {id}"),
            scope: Scope::Market,
            expr: Expr::cond(Metric::VolumeZScore, Comparator::Ge, 3.0),
            weight,
        }
    }

    // ── Резолвер config-пути по env ──────────────────────────────────────────

    #[test]
    fn resolve_config_dir_prefers_override_env() {
        let env: HashMap<&str, &str> = [(CONFIG_DIR_ENV, "/custom/config/dir")].into();
        let dir = resolve_config_dir_with(|k| env.get(k).map(|s| s.to_string()));
        assert_eq!(dir, PathBuf::from("/custom/config/dir"));
    }

    #[test]
    fn resolve_config_dir_ignores_empty_override() {
        let env: HashMap<&str, &str> = [(CONFIG_DIR_ENV, ""), ("HOME", "/home/u")].into();
        let dir = resolve_config_dir_with(|k| env.get(k).map(|s| s.to_string()));
        // Пустой override игнорируется — используется unix-фолбэк по HOME.
        assert!(dir.ends_with(".config/market-terminal") || dir.ends_with("market-terminal"));
    }

    #[test]
    fn resolve_config_dir_is_pure_and_total_without_any_env() {
        // Ни одна переменная не задана — резолвер не паникует, отдаёт валидный путь.
        let dir = resolve_config_dir_with(|_| None);
        assert!(dir.to_string_lossy().contains(APP_DIR_NAME));
    }

    // ── Roundtrip settings_get/set ────────────────────────────────────────────

    #[test]
    fn settings_roundtrip_get_set() {
        let tmp = TempDir::new("roundtrip");
        let store = SettingsStore::new(tmp.0.clone());

        // Нет файла — дефолты.
        assert_eq!(store.get_settings(), SettingsDto::default());

        let mut custom = SettingsDto {
            tape_limit: 250,
            llm_provider: "anthropic".to_string(),
            ..SettingsDto::default()
        };
        custom.watchlist.insert("YDEX".to_string(), true);
        store.set_settings(custom.clone()).unwrap();

        assert_eq!(store.get_settings(), custom);

        // Новый store над тем же каталогом видит записанное — файл, не память.
        let store2 = SettingsStore::new(tmp.0.clone());
        assert_eq!(store2.get_settings(), custom);
    }

    #[test]
    fn settings_and_key_activity_rules_are_independent_sections() {
        let tmp = TempDir::new("sections");
        let store = SettingsStore::new(tmp.0.clone());

        let custom = SettingsDto {
            dom_depth: 42,
            ..SettingsDto::default()
        };
        store.set_settings(custom.clone()).unwrap();

        store
            .set_key_activity_rules(vec![sample_rule("r1", 2.0)])
            .unwrap();

        // Запись правил не затёрла настройки, и наоборот.
        assert_eq!(store.get_settings(), custom);
        assert_eq!(store.get_key_activity_rules(), vec![sample_rule("r1", 2.0)]);

        let mut custom2 = custom.clone();
        custom2.dom_depth = 7;
        store.set_settings(custom2.clone()).unwrap();
        assert_eq!(store.get_key_activity_rules(), vec![sample_rule("r1", 2.0)]);
    }

    #[test]
    fn key_activity_rules_default_to_empty_until_saved() {
        let tmp = TempDir::new("ka-empty");
        let store = SettingsStore::new(tmp.0.clone());
        assert!(store.get_key_activity_rules().is_empty());
    }

    // ── Атомарность записи ────────────────────────────────────────────────────

    #[test]
    fn write_is_atomic_old_file_survives_a_failed_write() {
        let tmp = TempDir::new("atomic");
        let store = SettingsStore::new(tmp.0.clone());

        let first = SettingsDto {
            tape_limit: 111,
            ..SettingsDto::default()
        };
        store.set_settings(first.clone()).unwrap();

        // Имитируем «падение посреди записи»: временный файл создан и
        // содержит мусор, но `rename` не выполнялся — целевой файл должен
        // остаться нетронутым (старое значение), а не оказаться битым.
        let tmp_path = tmp.0.join(format!("{SETTINGS_FILE}.tmp"));
        fs::write(&tmp_path, b"{not valid json").unwrap();

        assert_eq!(store.get_settings(), first);

        // Последующая успешная запись всё равно работает (временный файл
        // перезаписывается, а не мешает).
        let mut second = first.clone();
        second.tape_limit = 222;
        store.set_settings(second.clone()).unwrap();
        assert_eq!(store.get_settings(), second);
        assert!(
            !tmp_path.exists(),
            "временный файл должен быть переименован"
        );
    }

    #[test]
    fn read_survives_missing_and_corrupt_file() {
        let tmp = TempDir::new("corrupt");
        let store = SettingsStore::new(tmp.0.clone());
        // Файла ещё нет.
        assert_eq!(store.get_settings(), SettingsDto::default());

        fs::write(store.settings_path(), b"{ this is not json").unwrap();
        assert_eq!(store.get_settings(), SettingsDto::default());
        assert!(store.get_key_activity_rules().is_empty());
    }
}
