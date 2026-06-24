//! Ширина рынка (breadth): сколько бумаг растёт против падающих.
//!
//! Breadth показывает, «широкий» ли рынок (растёт большинство) или рост держат
//! несколько тяжеловесов. Считается по дневным изменениям набора инструментов.

/// Сводка ширины рынка по набору изменений (в долях: `0.01` = +1%).
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Breadth {
    pub advancers: u32,
    pub decliners: u32,
    pub unchanged: u32,
}

impl Breadth {
    /// Всего инструментов в выборке.
    pub fn total(&self) -> u32 {
        self.advancers + self.decliners + self.unchanged
    }

    /// Net advances = растущие − падающие (вклад в A/D-линию).
    pub fn net_advances(&self) -> i64 {
        self.advancers as i64 - self.decliners as i64
    }

    /// Advance/Decline ratio. `None`, если падающих нет (деление на ноль).
    pub fn ad_ratio(&self) -> Option<f64> {
        if self.decliners == 0 {
            None
        } else {
            Some(self.advancers as f64 / self.decliners as f64)
        }
    }

    /// Доля растущих от всех (0..1). `None` для пустой выборки.
    pub fn pct_advancing(&self) -> Option<f64> {
        let total = self.total();
        if total == 0 {
            None
        } else {
            Some(self.advancers as f64 / total as f64)
        }
    }
}

/// Посчитать breadth по списку дневных изменений.
///
/// Изменение с модулем меньше `flat_eps` считается «без изменения» — это
/// убирает шум на бумагах, дёрнувшихся на доли процента.
pub fn breadth(changes: &[f64], flat_eps: f64) -> Breadth {
    let mut b = Breadth::default();
    for &c in changes {
        if c > flat_eps {
            b.advancers += 1;
        } else if c < -flat_eps {
            b.decliners += 1;
        } else {
            b.unchanged += 1;
        }
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_with_epsilon() {
        let changes = [0.05, -0.03, 0.0001, -0.0001, 0.02];
        let b = breadth(&changes, 0.001);
        assert_eq!(b.advancers, 2);
        assert_eq!(b.decliners, 1);
        assert_eq!(b.unchanged, 2);
        assert_eq!(b.total(), 5);
        assert_eq!(b.net_advances(), 1);
    }

    #[test]
    fn ratios_and_edge_cases() {
        let b = breadth(&[0.1, 0.1, -0.1, -0.1], 0.0);
        assert_eq!(b.ad_ratio(), Some(1.0));
        assert_eq!(b.pct_advancing(), Some(0.5));

        let empty = breadth(&[], 0.0);
        assert_eq!(empty.pct_advancing(), None);
        assert_eq!(empty.ad_ratio(), None);

        let all_up = breadth(&[0.1, 0.2], 0.0);
        assert_eq!(all_up.ad_ratio(), None); // нет падающих
    }
}
