//! Хранилище API-секрета.
//!
//! Авторизация в Finam Trade API использует долгоживущий API-секрет, из которого
//! [`crate::auth::TokenManager`] получает короткоживущие JWT. Секрет нельзя
//! держать в коде/конфиге открытым текстом — на десктопе его место в системном
//! защищённом хранилище (Keychain / Secret Service / Credential Manager).
//!
//! Здесь — абстракция [`SecretStore`] и две независимые от ОС реализации:
//! [`StaticSecretStore`] (для тестов и инъекции) и [`EnvSecretStore`] (из
//! переменной окружения). Реализация поверх `keyring` подключается в десктопной
//! сборке за отдельной фичей — см. примечание ниже; её нельзя тестировать в
//! headless-CI, поэтому контракт и логика проверяются на ОС-независимых имплах.

use crate::DataError;

/// Переменная окружения по умолчанию с API-секретом Finam.
pub const DEFAULT_ENV_VAR: &str = "FINAM_API_SECRET";

/// Источник API-секрета.
pub trait SecretStore {
    /// Получить API-секрет. Ошибка — если секрет не найден/недоступен.
    fn api_secret(&self) -> Result<String, DataError>;
}

/// Секрет, заданный значением напрямую (инъекция в тестах и обёртках).
#[derive(Debug, Clone)]
pub struct StaticSecretStore(String);

impl StaticSecretStore {
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }
}

impl SecretStore for StaticSecretStore {
    fn api_secret(&self) -> Result<String, DataError> {
        if self.0.is_empty() {
            Err(DataError::Auth("API-секрет пуст".into()))
        } else {
            Ok(self.0.clone())
        }
    }
}

/// Секрет из переменной окружения (по умолчанию [`DEFAULT_ENV_VAR`]).
#[derive(Debug, Clone)]
pub struct EnvSecretStore {
    var: String,
}

impl EnvSecretStore {
    /// Читать из переменной [`DEFAULT_ENV_VAR`].
    pub fn new() -> Self {
        Self::from_var(DEFAULT_ENV_VAR)
    }

    /// Читать из произвольной переменной окружения.
    pub fn from_var(var: impl Into<String>) -> Self {
        Self { var: var.into() }
    }
}

impl Default for EnvSecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for EnvSecretStore {
    fn api_secret(&self) -> Result<String, DataError> {
        match std::env::var(&self.var) {
            Ok(v) if !v.is_empty() => Ok(v),
            _ => Err(DataError::Auth(format!(
                "переменная окружения {} не задана",
                self.var
            ))),
        }
    }
}

// Десктопная реализация поверх системного хранилища секретов подключается за
// фичей `keyring` (зависимость `keyring` тянет ОС-бэкенды и недоступна в
// headless-CI). Эскиз контракта:
//
// ```ignore
// pub struct KeyringSecretStore { service: String, account: String }
// impl SecretStore for KeyringSecretStore {
//     fn api_secret(&self) -> Result<String, DataError> {
//         keyring::Entry::new(&self.service, &self.account)
//             .and_then(|e| e.get_password())
//             .map_err(|e| DataError::Auth(e.to_string()))
//     }
// }
// ```

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_store_returns_value() {
        let s = StaticSecretStore::new("top-secret");
        assert_eq!(s.api_secret().unwrap(), "top-secret");
    }

    #[test]
    fn static_store_rejects_empty() {
        let s = StaticSecretStore::new("");
        assert!(s.api_secret().is_err());
    }

    #[test]
    fn env_store_reads_variable() {
        // Уникальное имя переменной, чтобы не пересекаться с другими тестами.
        let var = "MARKETFLOW_TEST_SECRET_OK";
        std::env::set_var(var, "abc123");
        let s = EnvSecretStore::from_var(var);
        assert_eq!(s.api_secret().unwrap(), "abc123");
        std::env::remove_var(var);
    }

    #[test]
    fn env_store_errors_when_missing() {
        let s = EnvSecretStore::from_var("MARKETFLOW_TEST_SECRET_MISSING");
        assert!(s.api_secret().is_err());
    }
}
