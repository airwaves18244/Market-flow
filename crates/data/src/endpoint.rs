//! Канонические идентификаторы методов Finam Trade API.
//!
//! Один источник истины для имён методов: их используют как ключи
//! [`RateLimiter`](crate::RateLimiter) (лимит ~200/мин — *на метод*) и как
//! `&'static str`-метку для трейсинга и ошибки [`DataError::RateLimited`].
//!
//! [`DataError::RateLimited`]: crate::DataError::RateLimited

/// Метод Finam Trade API, частота которого ограничивается раздельно.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Method {
    /// `AuthService.Auth` — обмен API-секрета на короткоживущий JWT.
    Auth,
    /// `AssetsService.Assets` — список инструментов биржи.
    Assets,
    /// `MarketDataService.Bars` — исторические бары.
    Bars,
    /// `MarketDataService.LastQuote` — последняя котировка.
    LastQuote,
    /// `MarketDataService.LatestTrades` — последние сделки.
    LatestTrades,
}

impl Method {
    /// Все методы — для итерирования (например, прогрев/диагностика лимитов).
    pub const ALL: [Method; 5] = [
        Method::Auth,
        Method::Assets,
        Method::Bars,
        Method::LastQuote,
        Method::LatestTrades,
    ];

    /// Стабильное строковое имя метода (ключ лимитера, метка трейсинга).
    pub const fn as_str(self) -> &'static str {
        match self {
            Method::Auth => "auth",
            Method::Assets => "assets",
            Method::Bars => "bars",
            Method::LastQuote => "last_quote",
            Method::LatestTrades => "latest_trades",
        }
    }
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<Method> for &'static str {
    fn from(m: Method) -> Self {
        m.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_stable_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for m in Method::ALL {
            assert!(seen.insert(m.as_str()), "дубликат имени метода: {m}");
        }
        assert_eq!(seen.len(), Method::ALL.len());
    }

    #[test]
    fn names_match_marketdata_trait_methods() {
        // Имена должны совпадать с методами трейта `MarketData`/`AuthService`.
        assert_eq!(Method::Auth.as_str(), "auth");
        assert_eq!(Method::Assets.as_str(), "assets");
        assert_eq!(Method::Bars.as_str(), "bars");
        assert_eq!(Method::LastQuote.as_str(), "last_quote");
        assert_eq!(Method::LatestTrades.as_str(), "latest_trades");
    }

    #[test]
    fn display_and_into_str_agree_with_as_str() {
        for m in Method::ALL {
            assert_eq!(m.to_string(), m.as_str());
            let s: &'static str = m.into();
            assert_eq!(s, m.as_str());
        }
    }
}
