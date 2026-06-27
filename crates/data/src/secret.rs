//! Хранилище секрета API (долгоживущий ключ Finam).
//!
//! Долгоживущий API-секрет обменивается на короткоживущий JWT (см.
//! [`TokenState`](crate::TokenState)) и не должен попадать в репозиторий или
//! логи. Контракт [`SecretStore`] абстрагирует место хранения; здесь же —
//! кросс-платформенная in-memory реализация [`MemSecretStore`] для тестов и
//! headless-запуска.
//!
//! Боевое хранилище — ОС-keyring (Keychain/Credential Manager/Secret Service) —
//! подключается отдельной реализацией за фичей в фазе интеграции, чтобы не тянуть
//! платформенные зависимости в кросс-платформенный CI.

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
}
