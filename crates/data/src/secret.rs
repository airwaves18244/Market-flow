//! Хранилище секрета API (долгоживущий ключ Finam).
//!
//! Долгоживущий API-секрет обменивается на короткоживущий JWT (см.
//! [`TokenState`](crate::TokenState)) и не должен попадать в репозиторий или
//! логи. Контракт [`SecretStore`] абстрагирует место хранения; здесь же —
//! кросс-платформенная in-memory реализация [`MemSecretStore`] для тестов и
//! headless-запуска.
//!
//! Боевое хранилище — ОС-keyring (Keychain/Credential Manager/ядро Linux) —
//! доступно за фичей `keyring` ([`KeyringSecretStore`]); платформенные
//! зависимости не тянутся в кросс-платформенный CI, где фича выключена.

use std::sync::Mutex;

use crate::DataError;

/// Источник/приёмник секрета API. Реализация может быть как in-memory, так и
/// поверх ОС-keyring.
pub trait SecretStore {
    /// Прочитать секрет. `Ok(None)` — секрет ещё не задан (не ошибка).
    fn load(&self) -> Result<Option<String>, DataError>;

    /// Сохранить (перезаписать) секрет.
    fn store(&self, secret: &str) -> Result<(), DataError>;

    /// Удалить секрет. Удаление отсутствующего — не ошибка (идемпотентно).
    fn delete(&self) -> Result<(), DataError>;
}

/// In-memory реализация [`SecretStore`]: потокобезопасная, ничего не пишет на
/// диск. Подходит для тестов и запуска, где секрет подаётся из окружения.
#[derive(Debug, Default)]
pub struct MemSecretStore {
    secret: Mutex<Option<String>>,
}

impl MemSecretStore {
    /// Пустое хранилище.
    pub fn new() -> Self {
        Self::default()
    }

    /// Хранилище с заранее заданным секретом (например, из переменной окружения).
    pub fn with_secret(secret: impl Into<String>) -> Self {
        Self {
            secret: Mutex::new(Some(secret.into())),
        }
    }
}

impl SecretStore for MemSecretStore {
    fn load(&self) -> Result<Option<String>, DataError> {
        Ok(self.secret.lock().expect("secret mutex отравлен").clone())
    }

    fn store(&self, secret: &str) -> Result<(), DataError> {
        *self.secret.lock().expect("secret mutex отравлен") = Some(secret.to_owned());
        Ok(())
    }

    fn delete(&self) -> Result<(), DataError> {
        *self.secret.lock().expect("secret mutex отравлен") = None;
        Ok(())
    }
}

/// ОС-keyring реализация [`SecretStore`] (фича `keyring`).
///
/// Кладёт долгоживущий API-секрет в нативное защищённое хранилище ОС:
/// Credential Manager (Windows), Keychain (macOS), ключи ядра/keyutils (Linux).
/// Платформенный бэкенд выбирается крейтом `keyring` на этапе сборки, поэтому
/// в кросс-платформенном CI фича выключена и зависимость не подтягивается.
///
/// Секрет адресуется парой «сервис + пользователь»; по умолчанию —
/// [`DEFAULT_SERVICE`](Self::DEFAULT_SERVICE) /
/// [`DEFAULT_USER`](Self::DEFAULT_USER).
#[cfg(feature = "keyring")]
#[derive(Debug, Clone)]
pub struct KeyringSecretStore {
    service: String,
    user: String,
}

#[cfg(feature = "keyring")]
impl KeyringSecretStore {
    /// Имя сервиса в keyring по умолчанию.
    pub const DEFAULT_SERVICE: &'static str = "market-terminal";
    /// Имя «пользователя» (ключа записи) по умолчанию.
    pub const DEFAULT_USER: &'static str = "finam-api-secret";

    /// Хранилище с адресацией по умолчанию.
    pub fn new() -> Self {
        Self::with_target(Self::DEFAULT_SERVICE, Self::DEFAULT_USER)
    }

    /// Хранилище с заданными сервисом и пользователем (для тестов/нескольких
    /// окружений в одной ОС).
    pub fn with_target(service: impl Into<String>, user: impl Into<String>) -> Self {
        Self {
            service: service.into(),
            user: user.into(),
        }
    }

    fn entry(&self) -> Result<keyring::Entry, DataError> {
        keyring::Entry::new(&self.service, &self.user)
            .map_err(|e| DataError::Other(format!("keyring: {e}")))
    }
}

#[cfg(feature = "keyring")]
impl Default for KeyringSecretStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "keyring")]
impl SecretStore for KeyringSecretStore {
    fn load(&self) -> Result<Option<String>, DataError> {
        match self.entry()?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            // Отсутствие записи — не ошибка: секрет ещё не задан.
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(DataError::Other(format!("keyring load: {e}"))),
        }
    }

    fn store(&self, secret: &str) -> Result<(), DataError> {
        self.entry()?
            .set_password(secret)
            .map_err(|e| DataError::Other(format!("keyring store: {e}")))
    }

    fn delete(&self) -> Result<(), DataError> {
        match self.entry()?.delete_credential() {
            // Удаление отсутствующего секрета идемпотентно.
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(DataError::Other(format!("keyring delete: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_store_loads_none() {
        let s = MemSecretStore::new();
        assert_eq!(s.load().unwrap(), None);
    }

    #[test]
    fn store_then_load_roundtrips() {
        let s = MemSecretStore::new();
        s.store("api-secret-xyz").unwrap();
        assert_eq!(s.load().unwrap().as_deref(), Some("api-secret-xyz"));
    }

    #[test]
    fn store_overwrites_previous() {
        let s = MemSecretStore::with_secret("old");
        s.store("new").unwrap();
        assert_eq!(s.load().unwrap().as_deref(), Some("new"));
    }

    #[test]
    fn delete_is_idempotent() {
        let s = MemSecretStore::with_secret("k");
        s.delete().unwrap();
        assert_eq!(s.load().unwrap(), None);
        // Повторное удаление отсутствующего секрета — не ошибка.
        s.delete().unwrap();
        assert_eq!(s.load().unwrap(), None);
    }

    #[test]
    fn with_secret_seeds_initial_value() {
        let s = MemSecretStore::with_secret("seed");
        assert_eq!(s.load().unwrap().as_deref(), Some("seed"));
    }

    /// Контракт используется через динамический объект (как сделает `app`).
    #[test]
    fn usable_as_trait_object() {
        let s: Box<dyn SecretStore> = Box::new(MemSecretStore::new());
        s.store("via-dyn").unwrap();
        assert_eq!(s.load().unwrap().as_deref(), Some("via-dyn"));
    }

    /// Keyring-реализация удовлетворяет тому же контракту (проверка на этапе
    /// компиляции; рантайм-тесты ниже требуют реального ОС-keyring).
    #[cfg(feature = "keyring")]
    #[test]
    fn keyring_store_is_secret_store() {
        let s = KeyringSecretStore::with_target("market-terminal-test", "unit");
        let _dyn: Box<dyn SecretStore> = Box::new(s);
    }

    /// Полный цикл поверх настоящего ОС-keyring. По умолчанию игнорируется:
    /// в headless-CI нет keyring-сессии. Запуск вручную:
    /// `cargo test -p data --features keyring -- --ignored`.
    #[cfg(feature = "keyring")]
    #[test]
    #[ignore = "требует настоящий ОС-keyring (Keychain/Credential Manager/keyutils)"]
    fn keyring_roundtrip() {
        let s = KeyringSecretStore::with_target("market-terminal-test", "roundtrip");
        // Чистый старт.
        s.delete().unwrap();
        assert_eq!(s.load().unwrap(), None);

        s.store("api-secret-xyz").unwrap();
        assert_eq!(s.load().unwrap().as_deref(), Some("api-secret-xyz"));

        s.store("api-secret-2").unwrap();
        assert_eq!(s.load().unwrap().as_deref(), Some("api-secret-2"));

        s.delete().unwrap();
        assert_eq!(s.load().unwrap(), None);
        // Повторное удаление идемпотентно.
        s.delete().unwrap();
    }
}
