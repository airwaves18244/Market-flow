//! Классификация инструментов по секторам.
//!
//! Finam Trade API не отдаёт отраслевую принадлежность, поэтому сектор берём из
//! редактируемой таблицы соответствия (составы отраслевых индексов MOEX +
//! ручные правки). Поиск — сначала по тикеру, затем по ISIN.
//!
//! Это уже рабочая, тестируемая логика; источник данных (файл/БД) подключается
//! на уровне `storage`.

use std::collections::HashMap;

/// Таблица соответствия «инструмент → сектор».
#[derive(Debug, Clone, Default)]
pub struct SectorMap {
    by_ticker: HashMap<String, String>,
    by_isin: HashMap<String, String>,
}

impl SectorMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Построить таблицу из пар `(ключ, сектор)`, где ключ — тикер или ISIN.
    /// ISIN определяется по форме: 12 символов, первые два — буквы.
    pub fn from_pairs<I, S>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (S, S)>,
        S: Into<String>,
    {
        let mut map = SectorMap::new();
        for (key, sector) in pairs {
            let key = key.into();
            let sector = sector.into();
            if is_isin(&key) {
                map.by_isin.insert(key, sector);
            } else {
                map.by_ticker.insert(key.to_uppercase(), sector);
            }
        }
        map
    }

    /// Добавить/переопределить соответствие по тикеру.
    pub fn set_ticker(&mut self, ticker: impl Into<String>, sector: impl Into<String>) {
        self.by_ticker
            .insert(ticker.into().to_uppercase(), sector.into());
    }

    /// Найти сектор: приоритет у тикера, затем ISIN.
    pub fn lookup(&self, ticker: &str, isin: Option<&str>) -> Option<&str> {
        if let Some(s) = self.by_ticker.get(&ticker.to_uppercase()) {
            return Some(s.as_str());
        }
        if let Some(isin) = isin {
            if let Some(s) = self.by_isin.get(isin) {
                return Some(s.as_str());
            }
        }
        None
    }

    pub fn len(&self) -> usize {
        self.by_ticker.len() + self.by_isin.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_ticker.is_empty() && self.by_isin.is_empty()
    }
}

/// Грубая проверка формата ISIN: 12 символов, первые два — латинские буквы.
fn is_isin(s: &str) -> bool {
    s.len() == 12
        && s.chars().take(2).all(|c| c.is_ascii_alphabetic())
        && s.chars().skip(2).all(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticker_lookup_is_case_insensitive() {
        let m = SectorMap::from_pairs([("SBER", "Финансы"), ("GAZP", "Нефтегаз")]);
        assert_eq!(m.lookup("sber", None), Some("Финансы"));
        assert_eq!(m.lookup("GAZP", None), Some("Нефтегаз"));
        assert_eq!(m.lookup("UNKNOWN", None), None);
    }

    #[test]
    fn isin_is_detected_and_used_as_fallback() {
        let m = SectorMap::from_pairs([("RU0009029540", "Финансы")]);
        assert!(m.by_isin.contains_key("RU0009029540"));
        // по тикеру нет, по ISIN — есть
        assert_eq!(m.lookup("SBER", Some("RU0009029540")), Some("Финансы"));
    }

    #[test]
    fn ticker_takes_priority_over_isin() {
        let mut m = SectorMap::from_pairs([("RU0009029540", "ISIN-сектор")]);
        m.set_ticker("SBER", "Тикер-сектор");
        assert_eq!(m.lookup("SBER", Some("RU0009029540")), Some("Тикер-сектор"));
    }

    #[test]
    fn isin_format_detection() {
        assert!(is_isin("RU0009029540"));
        assert!(!is_isin("SBER"));
        assert!(!is_isin("12009029540X")); // первые два не буквы
    }
}
