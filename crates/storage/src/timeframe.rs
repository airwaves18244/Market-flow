//! Тайм-фрейм бара и его строковый код в колонке `bars.timeframe`.
//!
//! Намеренно дублирует множество значений `data::TimeFrame`, но не тянет
//! зависимость от сетевого слоя: хранилище опирается только на `domain`.
//! Код (`m1|m5|m15|h1|d1`) совпадает с тем, что записывается в DDL `bars`.

/// Тайм-фрейм бара.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeFrame {
    M1,
    M5,
    M15,
    H1,
    D1,
}

impl TimeFrame {
    /// Все значения в стабильном порядке (от мелкого к крупному).
    pub const ALL: [TimeFrame; 5] = [
        TimeFrame::M1,
        TimeFrame::M5,
        TimeFrame::M15,
        TimeFrame::H1,
        TimeFrame::D1,
    ];

    /// Машинный код для хранения в БД.
    pub fn code(self) -> &'static str {
        match self {
            TimeFrame::M1 => "m1",
            TimeFrame::M5 => "m5",
            TimeFrame::M15 => "m15",
            TimeFrame::H1 => "h1",
            TimeFrame::D1 => "d1",
        }
    }

    /// Разбор кода из БД. Регистр игнорируется.
    pub fn from_code(code: &str) -> Option<TimeFrame> {
        match code.to_ascii_lowercase().as_str() {
            "m1" => Some(TimeFrame::M1),
            "m5" => Some(TimeFrame::M5),
            "m15" => Some(TimeFrame::M15),
            "h1" => Some(TimeFrame::H1),
            "d1" => Some(TimeFrame::D1),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_roundtrips() {
        for tf in TimeFrame::ALL {
            assert_eq!(TimeFrame::from_code(tf.code()), Some(tf));
        }
    }

    #[test]
    fn from_code_is_case_insensitive_and_rejects_unknown() {
        assert_eq!(TimeFrame::from_code("D1"), Some(TimeFrame::D1));
        assert_eq!(TimeFrame::from_code("M15"), Some(TimeFrame::M15));
        assert_eq!(TimeFrame::from_code("w1"), None);
    }
}
