//! Контракт стратегии бэктестера и параметры.
//!
//! Стратегия — чистая функция истории: на каждом баре она видит окно баров до
//! текущего включительно и текущую позицию, и возвращает желаемую **целевую
//! позицию** (`Signal`). Целевая модель (а не «купи/продай N») делает движок
//! однозначным: он сам торгует разницу между текущей и целевой позицией.

use std::collections::BTreeMap;

use crate::model::Bar;

/// Параметры стратегии: имя → числовое значение. Плоский набор `f64` удобно
/// прокинуть в UI как форму и сериализовать в DTO.
pub type StrategyParams = BTreeMap<String, f64>;

/// Прочитать параметр с фолбэком на значение по умолчанию.
pub fn param(params: &StrategyParams, key: &str, default: f64) -> f64 {
    params.get(key).copied().unwrap_or(default)
}

/// Сигнал стратегии: желаемая знаковая позиция после исполнения.
///
/// `+` — длинная позиция (лонг), `−` — короткая (шорт), `0` — вне рынка.
/// Движок исполняет разницу `target − current` рыночной заявкой.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Signal {
    pub target_position: f64,
}

impl Signal {
    /// Целевая знаковая позиция.
    pub fn target(position: f64) -> Self {
        Self {
            target_position: position,
        }
    }

    /// Выйти из рынка (целевая позиция 0).
    pub fn flat() -> Self {
        Self::target(0.0)
    }
}

/// Контекст бара, передаваемый стратегии.
#[derive(Debug, Clone, Copy)]
pub struct BarContext<'a> {
    /// Бары с начала серии до текущего включительно (`bars.last()` — текущий).
    pub bars: &'a [Bar],
    /// Индекс текущего бара в полной серии.
    pub index: usize,
    /// Текущая знаковая позиция стратегии (в единицах/лотах).
    pub position: f64,
}

impl BarContext<'_> {
    /// Текущий бар.
    pub fn current(&self) -> &Bar {
        // Контекст всегда строится с непустым окном (см. engine).
        self.bars.last().expect("BarContext: пустое окно баров")
    }

    /// Цены закрытия видимого окна (для индикаторов).
    pub fn closes(&self) -> Vec<f64> {
        self.bars.iter().map(|b| b.close).collect()
    }
}

/// Торговая стратегия бэктестера.
pub trait Strategy {
    /// Стабильный машинный идентификатор стратегии (ключ в библиотеке/UI).
    fn id(&self) -> &'static str;

    /// Решение на текущем баре. `None` — оставить позицию без изменений.
    fn on_bar(&mut self, ctx: &BarContext) -> Option<Signal>;
}
