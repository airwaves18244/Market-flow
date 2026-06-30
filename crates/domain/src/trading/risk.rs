//! Предторговые проверки риска (pre-trade risk).

use crate::model::Side;

/// Причина отклонения заявки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// Объём заявки превышает лимит на заявку.
    MaxOrderQty,
    /// Заявка вывела бы позицию за лимит.
    MaxPosition,
    /// Неположительный объём.
    InvalidQty,
    /// У лимитной/стоп-заявки не задана цена.
    MissingPrice,
    /// Нет ликвидности (стакана) для рыночной заявки.
    NoLiquidity,
    /// Заявка не найдена (отмена несуществующей).
    NotFound,
}

impl RejectReason {
    /// Человекочитаемое сообщение (для UI/блоттера).
    pub fn message(self) -> &'static str {
        match self {
            RejectReason::MaxOrderQty => "объём превышает лимит на заявку",
            RejectReason::MaxPosition => "превышен лимит позиции",
            RejectReason::InvalidQty => "объём должен быть положительным",
            RejectReason::MissingPrice => "не задана цена заявки",
            RejectReason::NoLiquidity => "нет ликвидности для рыночной заявки",
            RejectReason::NotFound => "заявка не найдена",
        }
    }

    /// Код причины для DTO.
    pub fn code(self) -> &'static str {
        match self {
            RejectReason::MaxOrderQty => "max_order_qty",
            RejectReason::MaxPosition => "max_position",
            RejectReason::InvalidQty => "invalid_qty",
            RejectReason::MissingPrice => "missing_price",
            RejectReason::NoLiquidity => "no_liquidity",
            RejectReason::NotFound => "not_found",
        }
    }
}

/// Лимиты риска счёта.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiskLimits {
    /// Максимальный объём одной заявки.
    pub max_order_qty: f64,
    /// Максимальная абсолютная позиция по инструменту.
    pub max_position: f64,
}

impl Default for RiskLimits {
    fn default() -> Self {
        Self {
            max_order_qty: 1_000.0,
            max_position: 5_000.0,
        }
    }
}

impl RiskLimits {
    /// Проверить заявку до постановки: объём и проекция позиции в пределах лимитов.
    pub fn check(
        &self,
        current_position: f64,
        side: Side,
        qty: f64,
    ) -> Result<(), RejectReason> {
        if qty <= 0.0 {
            return Err(RejectReason::InvalidQty);
        }
        if qty > self.max_order_qty {
            return Err(RejectReason::MaxOrderQty);
        }
        let projected = (current_position + side.sign() * qty).abs();
        if projected > self.max_position {
            return Err(RejectReason::MaxPosition);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_oversized_order_and_position() {
        let limits = RiskLimits {
            max_order_qty: 10.0,
            max_position: 15.0,
        };
        assert_eq!(limits.check(0.0, Side::Buy, 0.0), Err(RejectReason::InvalidQty));
        assert_eq!(
            limits.check(0.0, Side::Buy, 11.0),
            Err(RejectReason::MaxOrderQty)
        );
        // позиция 10 + заявка 10 = 20 > 15
        assert_eq!(
            limits.check(10.0, Side::Buy, 10.0),
            Err(RejectReason::MaxPosition)
        );
        // в пределах
        assert!(limits.check(10.0, Side::Buy, 5.0).is_ok());
        // продажа сокращает позицию — допустимо
        assert!(limits.check(10.0, Side::Sell, 10.0).is_ok());
    }
}
