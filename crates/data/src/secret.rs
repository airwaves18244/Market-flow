//! Хранение API-секрета в ОС-keyring (§ 0.2).
//!
//! Секрет (API-токен Finam) **не** хранится в репозитории или конфиге — только
//! в защищённом хранилище ОС. Тонкая обёртка над `keyring`.

use crate::DataError;

/// Имя сервиса в keyring по умолчанию.
pub const SERVICE: &str = "market-terminal.finam";

fn entry(account: &str) -> Result<keyring::Entry, DataError> {
    keyring::Entry::new(SERVICE, account).map_err(|e| DataError::Auth(e.to_string()))
}

/// Прочитать секрет для аккаунта из keyring.
pub fn load(account: &str) -> Result<String, DataError> {
    entry(account)?
        .get_password()
        .map_err(|e| DataError::Auth(e.to_string()))
}

/// Сохранить секрет для аккаунта в keyring.
pub fn store(account: &str, secret: &str) -> Result<(), DataError> {
    entry(account)?
        .set_password(secret)
        .map_err(|e| DataError::Auth(e.to_string()))
}
