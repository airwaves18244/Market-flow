//! Планирование бэкфилла исторических баров.
//!
//! Чистые функции: по уже сохранённому покрытию и желаемому диапазону считают,
//! *что именно* надо дозагрузить, и режут это на страницы под лимиты API
//! (Finam отдаёт ограниченное число баров за запрос). Сам fetch — в `app`.

use domain::history::{missing_ranges, TimeRange};
use domain::TimeFrame;

/// Диапазон времени для запроса баров, `[from_ts, to_ts]` включительно
/// (UNIX-секунды UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FetchRange {
    pub from_ts: i64,
    pub to_ts: i64,
}

impl FetchRange {
    /// Сколько баров покрывает диапазон при шаге `step_secs` (включая оба конца).
    pub fn bar_count(&self, step_secs: i64) -> i64 {
        if step_secs <= 0 || self.to_ts < self.from_ts {
            return 0;
        }
        (self.to_ts - self.from_ts) / step_secs + 1
    }
}

/// Что дозагрузить, чтобы покрыть `[desired_from, desired_to]` поверх уже
/// сохранённого `existing_last` (время последнего бара в БД, `None` — пусто).
///
/// Дозагружаем только «хвост» после последнего бара: бэкфилл идёт вперёд во
/// времени и не перезапрашивает уже имеющуюся историю. Возвращает `None`, если
/// данные уже актуальны или диапазон вырожден.
pub fn plan_backfill(
    existing_last: Option<i64>,
    desired_from: i64,
    desired_to: i64,
    tf: TimeFrame,
) -> Option<FetchRange> {
    if desired_to < desired_from {
        return None;
    }
    let step = tf.seconds();
    let from = match existing_last {
        // следующий бар после последнего сохранённого
        Some(last) => last + step,
        None => desired_from,
    };
    let from = from.max(desired_from);
    if from > desired_to {
        return None;
    }
    Some(FetchRange {
        from_ts: from,
        to_ts: desired_to,
    })
}

/// Разбить диапазон на страницы не более `max_bars` баров каждая — под предел
/// числа баров в одном ответе API. Страницы идут по возрастанию времени,
/// смежны (без «мёртвой зоны» на стыках) и не пересекаются.
///
/// Границы страниц считаем в *секундах*, а не в барах: раньше страница
/// заканчивалась на `start + span - step` (последний ожидаемый бар), а
/// следующая начиналась с `end + step`. Если фактические `ts` баров не кратны
/// шагу (Finam D1 ≈ 21:00 UTC), интервал `(end, end + step)` выпадал из обеих
/// страниц — и бар на стыке терялся. Теперь страница покрывает `span` секунд
/// подряд `[start, start + span - 1]`, а следующая продолжается ровно со
/// следующей секунды `end + 1` — дыр между страницами нет. `MarketData::bars`
/// включителен по обеим границам, поэтому бар ровно на `end + 1` попадёт только
/// в следующую страницу (без дублей); дедуп по `ts` в `data::history` — второй
/// рубеж защиты.
pub fn chunk_range(range: FetchRange, tf: TimeFrame, max_bars: usize) -> Vec<FetchRange> {
    let step = tf.seconds();
    if step <= 0 || max_bars == 0 || range.to_ts < range.from_ts {
        return Vec::new();
    }
    // Длительность страницы в секундах: окно `span` секунд вмещает не более
    // `max_bars` баров, отстоящих друг от друга не менее чем на `step`.
    let span = step * (max_bars as i64);
    let mut out = Vec::new();
    let mut start = range.from_ts;
    while start <= range.to_ts {
        // Страница покрывает секунды подряд; верхняя граница — не дальше конца
        // диапазона.
        let end = (start + span - 1).min(range.to_ts);
        out.push(FetchRange {
            from_ts: start,
            to_ts: end,
        });
        // Смежная следующая страница: без разрыва между `end` и стартом.
        start = end + 1;
    }
    out
}

/// План инкрементальной дозагрузки истории по измерениям источник+TF (фаза
/// 11.2.3): по уже покрытым диапазонам (`covered`, из каталога
/// `history_datasets`) и запросу `requested` вычислить недостающие куски
/// ([`domain::history::missing_ranges`]) и нарезать каждый на страницы под
/// лимит API (`max_bars` баров на запрос).
///
/// Диапазоны истории полуоткрыты `[from, till)`, а [`FetchRange`] включителен
/// `[from_ts, to_ts]`, поэтому верхняя граница страницы — `till - 1` (последний
/// бар строго до `till`). Страницы идут по возрастанию времени и не
/// пересекаются; пустой результат — всё покрыто или запрос вырожден.
pub fn plan_history_fetch(
    covered: &[TimeRange],
    requested: TimeRange,
    tf: TimeFrame,
    max_bars: usize,
) -> Vec<FetchRange> {
    missing_ranges(requested, covered)
        .into_iter()
        .filter(|gap| !gap.is_empty())
        .flat_map(|gap| {
            let range = FetchRange {
                from_ts: gap.from,
                to_ts: gap.till - 1,
            };
            chunk_range(range, tf, max_bars)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400;

    #[test]
    fn fresh_db_fetches_whole_range() {
        let r = plan_backfill(None, 0, 10 * DAY, TimeFrame::D1).unwrap();
        assert_eq!(
            r,
            FetchRange {
                from_ts: 0,
                to_ts: 10 * DAY
            }
        );
        assert_eq!(r.bar_count(DAY), 11);
    }

    #[test]
    fn existing_data_fetches_only_tail() {
        // есть бары до 5*DAY; шаг — сутки → грузим с 6*DAY
        let r = plan_backfill(Some(5 * DAY), 0, 10 * DAY, TimeFrame::D1).unwrap();
        assert_eq!(r.from_ts, 6 * DAY);
        assert_eq!(r.to_ts, 10 * DAY);
    }

    #[test]
    fn up_to_date_returns_none() {
        assert!(plan_backfill(Some(10 * DAY), 0, 10 * DAY, TimeFrame::D1).is_none());
        // последний бар за пределами желаемого to — тоже нечего грузить
        assert!(plan_backfill(Some(20 * DAY), 0, 10 * DAY, TimeFrame::D1).is_none());
    }

    #[test]
    fn degenerate_range_returns_none() {
        assert!(plan_backfill(None, 100, 50, TimeFrame::D1).is_none());
    }

    #[test]
    fn existing_before_desired_from_clamps_to_from() {
        // последний бар раньше начала окна → грузим с desired_from
        let r = plan_backfill(Some(DAY), 5 * DAY, 8 * DAY, TimeFrame::D1).unwrap();
        assert_eq!(r.from_ts, 5 * DAY);
    }

    #[test]
    fn chunk_splits_into_pages_without_overlap() {
        let range = FetchRange {
            from_ts: 0,
            to_ts: 9 * DAY,
        };
        let pages = chunk_range(range, TimeFrame::D1, 4);
        // 10 баров по 4 на страницу → 4 + 4 + 2.
        // Границы теперь в секундах: страница покрывает span = 4*DAY секунд
        // подряд, следующая продолжается со следующей секунды (без «мёртвой
        // зоны» на стыке).
        assert_eq!(pages.len(), 3);
        assert_eq!(
            pages[0],
            FetchRange {
                from_ts: 0,
                to_ts: 4 * DAY - 1
            }
        );
        assert_eq!(
            pages[1],
            FetchRange {
                from_ts: 4 * DAY,
                to_ts: 8 * DAY - 1
            }
        );
        assert_eq!(
            pages[2],
            FetchRange {
                from_ts: 8 * DAY,
                to_ts: 9 * DAY
            }
        );
        // Страницы смежны и не пересекаются: конец каждой + 1 = начало следующей.
        for w in pages.windows(2) {
            assert_eq!(w[0].to_ts + 1, w[1].from_ts);
        }
        // Суммарно покрыто ровно 10 баров (при барах, кратных шагу).
        let total: i64 = pages.iter().map(|p| p.bar_count(DAY)).sum();
        assert_eq!(total, 10);
    }

    #[test]
    fn chunk_covers_offset_bars_without_gaps() {
        // Регрессия: фактические `ts` баров НЕ кратны шагу (как Finam D1 ≈ 21:00
        // UTC). Раньше страница заканчивалась на последнем ожидаемом баре, а
        // следующая начиналась через `step` — интервал между ними выпадал, и бар
        // на стыке терялся. Теперь страницы смежны в секундах и покрывают все
        // бары. Пример из аудита: step=60, max_bars=3, бары со сдвигом +30.
        const STEP: i64 = 60; // TimeFrame::M1
        let bars = [30_i64, 90, 150, 210, 270, 330];
        let range = FetchRange {
            from_ts: 0,
            to_ts: *bars.last().unwrap(),
        };
        let pages = chunk_range(range, TimeFrame::M1, 3);

        // Каждый бар должен попасть ровно в одну страницу (границы включительны).
        for &ts in &bars {
            let hits = pages
                .iter()
                .filter(|p| p.from_ts <= ts && ts <= p.to_ts)
                .count();
            assert_eq!(hits, 1, "бар ts={ts} должен попасть ровно в одну страницу");
        }
        // Ни одна страница не содержит больше max_bars=3 баров реального шага.
        for p in &pages {
            assert!(p.bar_count(STEP) <= 3, "страница {p:?} превышает лимит баров");
        }
        // Страницы смежны — без разрывов на стыках.
        for w in pages.windows(2) {
            assert_eq!(w[0].to_ts + 1, w[1].from_ts);
        }
    }

    #[test]
    fn chunk_single_page_when_fits() {
        let range = FetchRange {
            from_ts: 0,
            to_ts: 2 * DAY,
        };
        let pages = chunk_range(range, TimeFrame::D1, 100);
        assert_eq!(pages, vec![range]);
    }

    #[test]
    fn chunk_zero_max_bars_is_empty() {
        let range = FetchRange {
            from_ts: 0,
            to_ts: DAY,
        };
        assert!(chunk_range(range, TimeFrame::D1, 0).is_empty());
    }

    #[test]
    fn plan_history_fetch_covers_gaps_and_paginates() {
        // Покрыто [0, 5*DAY); запрос [0, 10*DAY) → дыра [5*DAY, 10*DAY).
        let covered = [TimeRange::new(0, 5 * DAY)];
        let requested = TimeRange::new(0, 10 * DAY);
        let pages = plan_history_fetch(&covered, requested, TimeFrame::D1, 3);
        // Дыра включительно [5*DAY, 10*DAY - 1] = 5 баров по суткам → 3 + 2.
        assert!(!pages.is_empty());
        assert_eq!(pages.first().unwrap().from_ts, 5 * DAY);
        // ни одна страница не выходит за верхнюю (полуоткрытую) границу
        assert!(pages.iter().all(|p| p.to_ts < 10 * DAY));
        let total: i64 = pages.iter().map(|p| p.bar_count(DAY)).sum();
        assert_eq!(total, 5);
    }

    #[test]
    fn plan_history_fetch_empty_when_covered() {
        let covered = [TimeRange::new(0, 10 * DAY)];
        let pages = plan_history_fetch(&covered, TimeRange::new(0, 10 * DAY), TimeFrame::D1, 5);
        assert!(pages.is_empty());
    }

    #[test]
    fn plan_then_chunk_pipeline() {
        // типичный путь: спланировать хвост, затем нарезать на страницы
        let plan = plan_backfill(Some(2 * DAY), 0, 12 * DAY, TimeFrame::D1).unwrap();
        let pages = chunk_range(plan, TimeFrame::D1, 5);
        assert_eq!(pages.first().unwrap().from_ts, 3 * DAY);
        assert_eq!(pages.last().unwrap().to_ts, 12 * DAY);
    }
}
