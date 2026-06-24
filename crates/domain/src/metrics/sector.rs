//! Роллапы метрик по секторам и классам активов.
//!
//! Сектор у Finam не приходит из API напрямую — он берётся из таблицы
//! классификации (`data::classify`). Здесь мы агрегируем уже посчитанные
//! по инструментам метрики в разрезе секторов для treemap/heatmap.

use std::collections::BTreeMap;

/// Вклад одного инструмента в секторную агрегацию.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InstrumentMetric {
    /// Оборот инструмента за период.
    pub turnover: f64,
    /// Чистый денежный поток (см. [`crate::metrics::flow`]).
    pub net_flow: f64,
    /// Дневное изменение в долях (`0.01` = +1%).
    pub change: f64,
}

/// Агрегированные метрики сектора.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct SectorAggregate {
    pub instruments: u32,
    pub turnover: f64,
    pub net_flow: f64,
    /// Средневзвешенное по обороту изменение сектора (в долях).
    pub weighted_change: f64,
}

/// Свернуть метрики инструментов в сектора.
///
/// `items` — пары `(сектор, метрика)`. Инструменты без сектора группируются
/// под ключом `unknown_label` (напр. `"—"` или `"Прочее"`).
///
/// `weighted_change` — изменение, взвешенное по обороту: крупные бумаги влияют
/// на «цвет» плитки сектора сильнее мелких. Если оборот сектора нулевой,
/// берётся простое среднее изменений.
pub fn rollup_by_sector<'a, I>(items: I, unknown_label: &str) -> BTreeMap<String, SectorAggregate>
where
    I: IntoIterator<Item = (Option<&'a str>, InstrumentMetric)>,
{
    // Накапливаем суммы и отдельно сумму change для fallback-среднего.
    struct Acc {
        agg: SectorAggregate,
        change_sum: f64,
        weighted_change_sum: f64,
    }
    let mut map: BTreeMap<String, Acc> = BTreeMap::new();

    for (sector, m) in items {
        let key = sector.unwrap_or(unknown_label).to_string();
        let e = map.entry(key).or_insert(Acc {
            agg: SectorAggregate::default(),
            change_sum: 0.0,
            weighted_change_sum: 0.0,
        });
        e.agg.instruments += 1;
        e.agg.turnover += m.turnover;
        e.agg.net_flow += m.net_flow;
        e.change_sum += m.change;
        e.weighted_change_sum += m.change * m.turnover;
    }

    map.into_iter()
        .map(|(k, mut a)| {
            a.agg.weighted_change = if a.agg.turnover > 0.0 {
                a.weighted_change_sum / a.agg.turnover
            } else if a.agg.instruments > 0 {
                a.change_sum / a.agg.instruments as f64
            } else {
                0.0
            };
            (k, a.agg)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(turnover: f64, net_flow: f64, change: f64) -> InstrumentMetric {
        InstrumentMetric {
            turnover,
            net_flow,
            change,
        }
    }

    #[test]
    fn rolls_up_and_weights_by_turnover() {
        let items = vec![
            (Some("Нефтегаз"), m(100.0, 10.0, 0.02)),
            (Some("Нефтегаз"), m(300.0, -5.0, -0.01)),
            (None, m(50.0, 1.0, 0.05)),
        ];
        let r = rollup_by_sector(items, "Прочее");

        let og = r.get("Нефтегаз").unwrap();
        assert_eq!(og.instruments, 2);
        assert_eq!(og.turnover, 400.0);
        assert_eq!(og.net_flow, 5.0);
        // weighted change = (0.02*100 + (-0.01)*300)/400 = (2 - 3)/400 = -0.0025
        assert!((og.weighted_change - (-0.0025)).abs() < 1e-12);

        let other = r.get("Прочее").unwrap();
        assert_eq!(other.instruments, 1);
        assert!((other.weighted_change - 0.05).abs() < 1e-12);
    }

    #[test]
    fn zero_turnover_falls_back_to_mean_change() {
        let items = vec![
            (Some("X"), m(0.0, 0.0, 0.10)),
            (Some("X"), m(0.0, 0.0, 0.20)),
        ];
        let r = rollup_by_sector(items, "—");
        let x = r.get("X").unwrap();
        assert!((x.weighted_change - 0.15).abs() < 1e-12);
    }
}
