//! Историзация для бэктестера: доменная модель локальных датасетов, каталог и
//! нормализация диапазонов (фаза 11).
//!
//! Чистый слой без сети и БД: расширенная свеча с явным источником и
//! тайм-фреймом, метаданные датасета и арифметика диапазонов (слияние без
//! дыр/перекрытий, планирование недостающих кусков для инкрементальной
//! дозагрузки). Хранилище (`storage`) и загрузчик (`app::history`) опираются на
//! эти типы.

use serde::{Deserialize, Serialize};

use crate::model::TimeFrame;

/// Источник исторических данных.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSource {
    /// Finam Trade API (gRPC bars).
    Finam,
    /// MOEX ALGOPACK (REST/ISS).
    MoexAlgo,
}

impl DataSource {
    /// Машинный код источника (ключ партиции в хранилище).
    pub fn code(self) -> &'static str {
        match self {
            DataSource::Finam => "finam",
            DataSource::MoexAlgo => "moex_algo",
        }
    }

    /// Разбор источника из кода.
    pub fn from_code(code: &str) -> Option<DataSource> {
        match code {
            "finam" => Some(DataSource::Finam),
            "moex_algo" => Some(DataSource::MoexAlgo),
            _ => None,
        }
    }
}

/// Расширенная историческая свеча: OHLCV плюс опциональные поля ALGOPACK
/// (VWAP, дисбаланс, открытый интерес, индекс концентрации). Источник и
/// тайм-фрейм заданы явно, чтобы датасеты разных источников не смешивались.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryBar {
    pub source: DataSource,
    pub secid: String,
    pub tf: TimeFrame,
    /// Время начала бара, UNIX-секунды UTC.
    pub ts: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vwap: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disb: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oi: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hi2: Option<f64>,
}

impl HistoryBar {
    /// Минимальная свеча OHLCV без ALGOPACK-полей.
    #[allow(clippy::too_many_arguments)]
    pub fn ohlcv(
        source: DataSource,
        secid: impl Into<String>,
        tf: TimeFrame,
        ts: i64,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: f64,
    ) -> Self {
        HistoryBar {
            source,
            secid: secid.into(),
            tf,
            ts,
            open,
            high,
            low,
            close,
            volume,
            vwap: None,
            disb: None,
            oi: None,
            hi2: None,
        }
    }

    /// Ключ дедупликации/upsert: (source, secid, tf, ts).
    pub fn key(&self) -> (DataSource, &str, TimeFrame, i64) {
        (self.source, &self.secid, self.tf, self.ts)
    }
}

/// Полуоткрытый временной диапазон `[from, till)` в UNIX-секундах.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    pub from: i64,
    pub till: i64,
}

impl TimeRange {
    pub fn new(from: i64, till: i64) -> Self {
        TimeRange { from, till }
    }

    /// Пустой ли диапазон (`till <= from`).
    pub fn is_empty(&self) -> bool {
        self.till <= self.from
    }

    /// Длительность в секундах (0 для пустого).
    pub fn duration(&self) -> i64 {
        (self.till - self.from).max(0)
    }

    /// Содержит ли момент `ts` (полуоткрыто).
    pub fn contains(&self, ts: i64) -> bool {
        ts >= self.from && ts < self.till
    }

    /// Пересекаются или соприкасаются ли диапазоны (для слияния).
    fn touches(&self, other: &TimeRange) -> bool {
        self.from <= other.till && other.from <= self.till
    }

    /// Огибающая двух диапазонов: `[min(from), max(till))`. В отличие от
    /// [`normalize_ranges`], не теряет несмежные куски — граничные значения
    /// берутся по краям, а внутренние дыры остаются «дырами» (их истинное
    /// покрытие определяется барами, а не этим диапазоном).
    pub fn envelope(&self, other: &TimeRange) -> TimeRange {
        TimeRange::new(self.from.min(other.from), self.till.max(other.till))
    }
}

/// Нормализовать набор диапазонов: отбросить пустые, отсортировать и слить
/// пересекающиеся/смежные — на выходе непересекающиеся диапазоны без дыр внутри
/// слитых кусков, в порядке возрастания.
pub fn normalize_ranges(ranges: &[TimeRange]) -> Vec<TimeRange> {
    let mut sorted: Vec<TimeRange> = ranges.iter().copied().filter(|r| !r.is_empty()).collect();
    sorted.sort_by_key(|r| r.from);
    let mut out: Vec<TimeRange> = Vec::new();
    for r in sorted {
        if let Some(last) = out.last_mut() {
            if last.touches(&r) {
                last.till = last.till.max(r.till);
                continue;
            }
        }
        out.push(r);
    }
    out
}

/// Недостающие подсегменты `requested`, не покрытые `covered` — план
/// инкрементальной дозагрузки (закрытие дыр и хвоста).
///
/// `covered` нормализуется внутри, поэтому может содержать перекрытия. Результат
/// — непересекающиеся диапазоны в порядке возрастания; пустой, если всё покрыто.
pub fn missing_ranges(requested: TimeRange, covered: &[TimeRange]) -> Vec<TimeRange> {
    if requested.is_empty() {
        return Vec::new();
    }
    let mut cursor = requested.from;
    let mut gaps = Vec::new();
    for r in normalize_ranges(covered) {
        if r.till <= cursor {
            continue; // целиком слева от курсора
        }
        if r.from >= requested.till {
            break; // целиком справа от запроса
        }
        if r.from > cursor {
            gaps.push(TimeRange::new(cursor, r.from.min(requested.till)));
        }
        cursor = cursor.max(r.till);
        if cursor >= requested.till {
            break;
        }
    }
    if cursor < requested.till {
        gaps.push(TimeRange::new(cursor, requested.till));
    }
    gaps
}

/// Метаданные локального датасета в каталоге.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasetMeta {
    pub source: DataSource,
    pub secid: String,
    pub tf: TimeFrame,
    /// Огибающий диапазон покрытия `[min(from), max(till))`. Внутри возможны
    /// дыры — это лишь внешние границы датасета, а не гарантия сплошного
    /// покрытия. Источник истины по фактическому покрытию — сами бары
    /// (`history_bars`) и построенный по ним план дыр
    /// (`Store::history_missing_ranges`).
    pub range: TimeRange,
    /// Число баров в датасете.
    pub bars: u64,
    /// Время последнего обновления (UNIX-секунды).
    pub updated_ts: i64,
}

impl DatasetMeta {
    /// Ожидаемое число баров для непрерывного покрытия диапазона при данном TF.
    pub fn expected_bars(&self) -> u64 {
        let step = self.tf.seconds();
        if step <= 0 || self.range.is_empty() {
            0
        } else {
            (self.range.duration() / step) as u64
        }
    }

    /// Похоже ли покрытие на полное (число баров ≥ ожидаемого). Грубая эвристика
    /// «нет крупных дыр» для UI-индикатора.
    pub fn looks_complete(&self) -> bool {
        self.bars >= self.expected_bars()
    }
}

/// Каталог локальных датасетов: ключ — (source, secid, tf).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Catalog {
    pub datasets: Vec<DatasetMeta>,
}

impl Catalog {
    pub fn new() -> Self {
        Catalog::default()
    }

    /// Найти датасет по ключу.
    pub fn find(&self, source: DataSource, secid: &str, tf: TimeFrame) -> Option<&DatasetMeta> {
        self.datasets
            .iter()
            .find(|d| d.source == source && d.secid == secid && d.tf == tf)
    }

    /// Вставить или объединить метаданные: при совпадении ключа диапазон
    /// расширяется до огибающей (`[min(from), max(till))`) — несмежные куски не
    /// теряются, внутренние дыры остаются на совести баров/`missing_ranges`.
    /// Число баров и время обновления берутся из нового значения.
    pub fn upsert(&mut self, meta: DatasetMeta) {
        if let Some(existing) = self
            .datasets
            .iter_mut()
            .find(|d| d.source == meta.source && d.secid == meta.secid && d.tf == meta.tf)
        {
            existing.range = existing.range.envelope(&meta.range);
            existing.bars = meta.bars;
            existing.updated_ts = meta.updated_ts;
        } else {
            self.datasets.push(meta);
        }
    }

    /// Удалить датасет по ключу; `true`, если что-то удалено.
    pub fn remove(&mut self, source: DataSource, secid: &str, tf: TimeFrame) -> bool {
        let before = self.datasets.len();
        self.datasets
            .retain(|d| !(d.source == source && d.secid == secid && d.tf == tf));
        self.datasets.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(from: i64, till: i64) -> TimeRange {
        TimeRange::new(from, till)
    }

    #[test]
    fn source_code_roundtrips() {
        assert_eq!(DataSource::from_code("finam"), Some(DataSource::Finam));
        assert_eq!(
            DataSource::from_code("moex_algo"),
            Some(DataSource::MoexAlgo)
        );
        assert_eq!(DataSource::from_code("x"), None);
        assert_eq!(DataSource::MoexAlgo.code(), "moex_algo");
    }

    #[test]
    fn normalize_merges_overlap_and_adjacency() {
        let n = normalize_ranges(&[r(0, 10), r(8, 20), r(20, 30), r(40, 50)]);
        // [0,10]+[8,20] перекрытие → [0,20]; [20,30] смежно → [0,30]; [40,50] отдельно.
        assert_eq!(n, vec![r(0, 30), r(40, 50)]);
    }

    #[test]
    fn normalize_drops_empty_and_sorts() {
        let n = normalize_ranges(&[r(30, 40), r(5, 5), r(0, 10)]);
        assert_eq!(n, vec![r(0, 10), r(30, 40)]);
    }

    #[test]
    fn missing_full_when_nothing_covered() {
        assert_eq!(missing_ranges(r(0, 100), &[]), vec![r(0, 100)]);
    }

    #[test]
    fn missing_none_when_fully_covered() {
        assert!(missing_ranges(r(10, 50), &[r(0, 100)]).is_empty());
    }

    #[test]
    fn missing_finds_interior_gaps_and_tail() {
        // Покрыто [0,20] и [40,60]; запрос [10,100].
        // Дыры: [20,40] и хвост [60,100].
        let gaps = missing_ranges(r(10, 100), &[r(0, 20), r(40, 60)]);
        assert_eq!(gaps, vec![r(20, 40), r(60, 100)]);
    }

    #[test]
    fn missing_leading_gap() {
        // Покрыто только [50,100]; запрос [0,100] → дыра [0,50].
        let gaps = missing_ranges(r(0, 100), &[r(50, 100)]);
        assert_eq!(gaps, vec![r(0, 50)]);
    }

    #[test]
    fn dataset_expected_bars_and_completeness() {
        let meta = DatasetMeta {
            source: DataSource::Finam,
            secid: "SBER".into(),
            tf: TimeFrame::M5,
            range: r(0, 3600), // час = 12 баров по 5 минут
            bars: 12,
            updated_ts: 3600,
        };
        assert_eq!(meta.expected_bars(), 12);
        assert!(meta.looks_complete());
        let sparse = DatasetMeta {
            bars: 5,
            ..meta.clone()
        };
        assert!(!sparse.looks_complete());
    }

    #[test]
    fn catalog_upsert_merges_range() {
        let mut cat = Catalog::new();
        cat.upsert(DatasetMeta {
            source: DataSource::Finam,
            secid: "SBER".into(),
            tf: TimeFrame::H1,
            range: r(0, 3600),
            bars: 1,
            updated_ts: 3600,
        });
        cat.upsert(DatasetMeta {
            source: DataSource::Finam,
            secid: "SBER".into(),
            tf: TimeFrame::H1,
            range: r(3600, 7200),
            bars: 2,
            updated_ts: 7200,
        });
        assert_eq!(cat.datasets.len(), 1);
        let d = cat.find(DataSource::Finam, "SBER", TimeFrame::H1).unwrap();
        assert_eq!(d.range, r(0, 7200)); // смежные слились
        assert_eq!(d.bars, 2);
    }

    #[test]
    fn catalog_upsert_envelopes_non_adjacent_range() {
        // Несмежные догрузки [0,10) и [50,60) не должны терять второй кусок:
        // диапазон расширяется до огибающей [0,60), а дыра [10,50) остаётся на
        // совести баров/missing_ranges.
        let mut cat = Catalog::new();
        cat.upsert(DatasetMeta {
            source: DataSource::Finam,
            secid: "SBER".into(),
            tf: TimeFrame::D1,
            range: r(0, 10),
            bars: 1,
            updated_ts: 10,
        });
        cat.upsert(DatasetMeta {
            source: DataSource::Finam,
            secid: "SBER".into(),
            tf: TimeFrame::D1,
            range: r(50, 60),
            bars: 2,
            updated_ts: 60,
        });
        let d = cat.find(DataSource::Finam, "SBER", TimeFrame::D1).unwrap();
        assert_eq!(d.range, r(0, 60)); // огибающая, а не потерянный [0,10)
        assert_eq!(d.bars, 2);
    }

    #[test]
    fn catalog_distinguishes_keys_and_removes() {
        let mut cat = Catalog::new();
        for tf in [TimeFrame::M1, TimeFrame::H1] {
            cat.upsert(DatasetMeta {
                source: DataSource::MoexAlgo,
                secid: "GAZP".into(),
                tf,
                range: r(0, 100),
                bars: 1,
                updated_ts: 100,
            });
        }
        assert_eq!(cat.datasets.len(), 2);
        assert!(cat.remove(DataSource::MoexAlgo, "GAZP", TimeFrame::M1));
        assert_eq!(cat.datasets.len(), 1);
        assert!(!cat.remove(DataSource::MoexAlgo, "GAZP", TimeFrame::M1));
    }

    #[test]
    fn history_bar_key_and_optional_fields() {
        let mut b = HistoryBar::ohlcv(
            DataSource::MoexAlgo,
            "SBER",
            TimeFrame::M5,
            300,
            100.0,
            101.0,
            99.0,
            100.5,
            1000.0,
        );
        assert_eq!(b.key(), (DataSource::MoexAlgo, "SBER", TimeFrame::M5, 300));
        assert!(b.vwap.is_none());
        b.vwap = Some(100.2);
        // Опциональные поля сериализуются только при наличии.
        let json = serde_json::to_string(&b).unwrap();
        assert!(json.contains("vwap"));
        assert!(!json.contains("disb"));
    }
}
