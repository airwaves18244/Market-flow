//! Планирование инкрементальной дозагрузки истории (фаза 11.2).
//!
//! Чистые функции поверх [`domain::history::missing_ranges`] и каталога: по уже
//! сохранённому покрытию и желаемому диапазону считают, *что именно* надо
//! докачать для ключа `{source, secid, tf}`. Реальный fetch (через
//! `data::HistorySource`) и запись — в оркестраторе `app` (фаза 11.3).

use domain::history::{missing_ranges, Catalog, DataSource, TimeRange};
use domain::TimeFrame;

/// Свести отсортированные метки баров в непрерывные покрытые диапазоны.
///
/// Бар с меткой `ts` покрывает полуоткрытый интервал `[ts, ts + step)`; соседние
/// бары, отстоящие ровно на шаг тайм-фрейма, сливаются в один диапазон, разрыв
/// (пропущенный бар) начинает новый. Метки дедуплицируются и сортируются внутри,
/// поэтому вход может быть в любом порядке. Так «сырое» содержимое `history_bars`
/// превращается в покрытие для [`plan_history_fetch`], включая внутренние дыры.
pub fn covered_ranges(bar_ts: &[i64], tf: TimeFrame) -> Vec<TimeRange> {
    let step = tf.seconds();
    if step <= 0 || bar_ts.is_empty() {
        return Vec::new();
    }
    let mut ts: Vec<i64> = bar_ts.to_vec();
    ts.sort_unstable();
    ts.dedup();

    let mut out = Vec::new();
    let mut start = ts[0];
    let mut prev = ts[0];
    for &t in &ts[1..] {
        if t == prev + step {
            prev = t;
        } else {
            out.push(TimeRange::new(start, prev + step));
            start = t;
            prev = t;
        }
    }
    out.push(TimeRange::new(start, prev + step));
    out
}

/// Что докачать, чтобы покрыть `desired` поверх уже имеющегося покрытия
/// `covered`. Тонкая обёртка над [`missing_ranges`]: результат —
/// непересекающиеся диапазоны по возрастанию (внутренние дыры + хвост), пустой,
/// если всё покрыто. `covered` нормализуется внутри, поэтому может содержать
/// перекрытия/несортированность.
pub fn plan_history_fetch(desired: TimeRange, covered: &[TimeRange]) -> Vec<TimeRange> {
    missing_ranges(desired, covered)
}

/// Что докачать для ключа `{source, secid, tf}` по каталогу датасетов.
///
/// Покрытие берётся из [`DatasetMeta::range`](domain::history::DatasetMeta) —
/// одного непрерывного диапазона на ключ (каталог сливает смежные при upsert).
/// Для планирования по фактическому содержимому с внутренними дырами используйте
/// [`covered_ranges`] над метками баров и [`plan_history_fetch`].
pub fn plan_from_catalog(
    catalog: &Catalog,
    source: DataSource,
    secid: &str,
    tf: TimeFrame,
    desired: TimeRange,
) -> Vec<TimeRange> {
    let covered: Vec<TimeRange> = catalog
        .find(source, secid, tf)
        .map(|d| d.range)
        .into_iter()
        .collect();
    missing_ranges(desired, &covered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::history::DatasetMeta;

    const STEP: i64 = 300; // M5

    #[test]
    fn covered_ranges_merges_runs_and_splits_on_gaps() {
        // Бары 0,300,600 (непрерывно), дыра, 1500,1800 — два покрытых куска.
        let ts = [600, 0, 300, 1800, 1500];
        let cov = covered_ranges(&ts, TimeFrame::M5);
        assert_eq!(
            cov,
            vec![
                TimeRange::new(0, 900),     // [0,300,600] → [0, 600+step)
                TimeRange::new(1500, 2100), // [1500,1800] → [1500, 1800+step)
            ]
        );
        assert!(covered_ranges(&[], TimeFrame::M5).is_empty());
    }

    #[test]
    fn plan_finds_interior_gap_and_tail() {
        // Покрыто [0,900) и [1500,2100); хотим [0, 3000) → дыра [900,1500) и
        // хвост [2100,3000).
        let covered = covered_ranges(&[0, STEP, 2 * STEP, 5 * STEP, 6 * STEP], TimeFrame::M5);
        let gaps = plan_history_fetch(TimeRange::new(0, 10 * STEP), &covered);
        assert_eq!(
            gaps,
            vec![TimeRange::new(900, 1500), TimeRange::new(2100, 3000)]
        );
    }

    #[test]
    fn plan_from_catalog_covers_leading_and_trailing_gaps() {
        let mut cat = Catalog::new();
        cat.upsert(DatasetMeta {
            source: DataSource::Finam,
            secid: "SBER".into(),
            tf: TimeFrame::M5,
            range: TimeRange::new(1500, 2100),
            bars: 2,
            updated_ts: 2100,
        });
        // Хотим [0, 3000): дыра до датасета [0,1500) и хвост [2100,3000).
        let gaps = plan_from_catalog(
            &cat,
            DataSource::Finam,
            "SBER",
            TimeFrame::M5,
            TimeRange::new(0, 3000),
        );
        assert_eq!(
            gaps,
            vec![TimeRange::new(0, 1500), TimeRange::new(2100, 3000)]
        );
        // Нет датасета — качаем весь запрошенный диапазон.
        let all = plan_from_catalog(
            &cat,
            DataSource::MoexAlgo,
            "GAZP",
            TimeFrame::M5,
            TimeRange::new(0, 3000),
        );
        assert_eq!(all, vec![TimeRange::new(0, 3000)]);
    }
}
