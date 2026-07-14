//! Чистый парсер ответов MOEX ISS (формат `{"<блок>": {"columns": [...],
//! "data": [[...], ...]}}`, опционально `<блок>.cursor` для пагинации).
//!
//! Форма блока и точные имена колонок **не сверены по живому ответу**
//! (`(unverified)`, см. `crates/data/tests/fixtures/moex/README.md`): здесь
//! заложена устойчивость к обеим правдоподобным конвенциям ISS — единый блок
//! `data` (типично для `iss.moex.com`) и блок, названный по датасету (как у
//! некоторых ресурсов `datashop`). [`IssTable::find`] пробует кандидатов по
//! порядку, поэтому смена конвенции на боевом ключе не потребует правки кода,
//! только фикстур.
//!
//! Доступ к полям — по имени колонки (без учёта регистра), не по индексу:
//! перестановка/переименование колонок в ответе не ломает парсер, а
//! отсутствующее поле мягко превращается в `None` там, где домен это допускает
//! ([`domain::algo::ObstatsPoint`]/[`domain::algo::OrderstatsPoint`] и т.п.
//! полностью опциональны; у `tradestats`/`futoi`/`hi2` обязательны только
//! `ts`/`secid` — как и в существующих доменных типах).

use domain::algo::futoi::ClientGroup;
use domain::algo::{FutoiPoint, Hi2Point, ObstatsPoint, OrderstatsPoint, SuperCandle};
use serde_json::Value;

/// Разобранная таблица ISS: имена колонок + строки значений.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IssTable {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

impl IssTable {
    /// Число строк.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Пустая ли таблица (нет строк).
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Индекс колонки по имени (без учёта регистра).
    fn column_index(&self, name: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|c| c.eq_ignore_ascii_case(name))
    }

    /// Представление строки `idx` для доступа к полям по имени.
    pub fn row(&self, idx: usize) -> RowView<'_> {
        RowView { table: self, idx }
    }

    /// Итератор по всем строкам.
    pub fn rows_iter(&self) -> impl Iterator<Item = RowView<'_>> {
        (0..self.rows.len()).map(move |i| self.row(i))
    }

    /// Найти таблицу под одним из блоков `candidates` (по порядку) в `value`.
    /// Блок без `columns`/`data` или с неожиданной формой пропускается.
    /// Возвращает `None`, если ни один кандидат не подошёл (пустой блок
    /// `data: []` — валидный случай, тогда таблица есть, но без строк).
    pub fn find(value: &Value, candidates: &[&str]) -> Option<IssTable> {
        for block in candidates {
            if let Some(table) = parse_block(value, block) {
                return Some(table);
            }
        }
        None
    }
}

fn parse_block(value: &Value, block: &str) -> Option<IssTable> {
    let obj = value.get(block)?;
    let columns: Vec<String> = obj
        .get("columns")?
        .as_array()?
        .iter()
        .map(|c| c.as_str().unwrap_or_default().to_owned())
        .collect();
    let raw_rows = obj.get("data")?.as_array()?;
    let rows = raw_rows
        .iter()
        .filter_map(|r| r.as_array().cloned())
        .collect();
    Some(IssTable { columns, rows })
}

/// Курсор пагинации ISS: `INDEX`/`TOTAL`/`PAGESIZE` (см. блок `<блок>.cursor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IssCursor {
    pub index: i64,
    pub total: i64,
    pub page_size: i64,
}

impl IssCursor {
    /// Есть ли ещё страницы после текущей.
    pub fn has_more(&self) -> bool {
        self.page_size > 0 && self.index + self.page_size < self.total
    }

    /// Значение параметра `start` для следующей страницы.
    pub fn next_start(&self) -> i64 {
        self.index + self.page_size
    }

    /// Найти курсор под `<block>.cursor` для одного из блоков-кандидатов.
    pub fn find(value: &Value, candidates: &[&str]) -> Option<IssCursor> {
        for block in candidates {
            let cursor_block = format!("{block}.cursor");
            if let Some(table) = parse_block(value, &cursor_block) {
                if table.is_empty() {
                    continue;
                }
                let row = table.row(0);
                let index = row.i64("INDEX")?;
                let total = row.i64("TOTAL")?;
                let page_size = row.i64("PAGESIZE")?;
                return Some(IssCursor {
                    index,
                    total,
                    page_size,
                });
            }
        }
        None
    }
}

/// Доступ к значению строки таблицы по имени колонки.
#[derive(Debug, Clone, Copy)]
pub struct RowView<'a> {
    table: &'a IssTable,
    idx: usize,
}

impl<'a> RowView<'a> {
    /// Сырое значение колонки `name` (без учёта регистра), если есть.
    pub fn value(&self, name: &str) -> Option<&'a Value> {
        let col = self.table.column_index(name)?;
        self.table.rows[self.idx].get(col)
    }

    /// Значение как `f64` (число или строка с числом; `null` → `None`).
    pub fn f64(&self, name: &str) -> Option<f64> {
        value_to_f64(self.value(name)?)
    }

    /// Значение как `i64` (число или строка с числом).
    pub fn i64(&self, name: &str) -> Option<i64> {
        let v = self.value(name)?;
        match v {
            Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
    }

    /// Значение как непустая строка (`null`/пустая строка → `None`).
    pub fn str(&self, name: &str) -> Option<&'a str> {
        match self.value(name)? {
            Value::String(s) if !s.is_empty() => Some(s.as_str()),
            _ => None,
        }
    }
}

fn value_to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Секунды UTC от `ts`/`systime`-колонки, либо из связки дата+время MSK
/// (`TRADEDATE`+`TRADETIME`, UTC+3 без перевода часов — как биржевое время
/// MOEX). `None`, если ни один вариант не разобрать.
fn row_ts(row: &RowView<'_>) -> Option<i64> {
    if let Some(ts) = row.i64("ts") {
        return Some(ts);
    }
    if let Some(ts) = row.i64("systime") {
        return Some(ts);
    }
    let date = row.str("tradedate")?;
    let time = row.str("tradetime").unwrap_or("00:00:00");
    moex_datetime_to_unix(date, time)
}

/// Перевести `YYYY-MM-DD` + `HH:MM:SS` московского времени (UTC+3, без
/// перевода часов) в unix-секунды UTC. Без внешней библиотеки дат — по
/// алгоритму Ховарда Хинанта (`days_from_civil`).
pub fn moex_datetime_to_unix(date: &str, time: &str) -> Option<i64> {
    const MSK_OFFSET_SECS: i64 = 3 * 3600;
    let mut dp = date.splitn(3, '-');
    let y: i64 = dp.next()?.parse().ok()?;
    let m: u32 = dp.next()?.parse().ok()?;
    let d: u32 = dp.next()?.parse().ok()?;
    let mut tp = time.splitn(3, ':');
    let hh: i64 = tp.next()?.parse().ok()?;
    let mm: i64 = tp.next()?.parse().ok()?;
    let ss: i64 = tp.next().unwrap_or("0").trim().parse().ok()?;
    let days = days_from_civil(y, m, d);
    Some(days * 86_400 + hh * 3600 + mm * 60 + ss - MSK_OFFSET_SECS)
}

/// Дни от эпохи (1970-01-01) для григорианской даты. Алгоритм Ховарда
/// Хинанта (`days_from_civil`) — корректен для всего пролептического
/// григорианского календаря, без внешних зависимостей.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (i64::from(m) + 9) % 12; // [0, 11]
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

/// Разобрать таблицу `tradestats` в Super Candles (`domain::algo::tradestats`).
/// Обязательны `secid`/дата-время; отсутствие числового поля даёт `0.0`
/// (Super Candles не опциональны — так же ведёт себя остальной домен фазы 10).
pub fn parse_tradestats(table: &IssTable) -> Vec<SuperCandle> {
    table
        .rows_iter()
        .filter_map(|row| {
            let secid = row.str("secid")?.to_owned();
            let ts = row_ts(&row)?;
            Some(SuperCandle {
                secid,
                ts,
                pr_open: row.f64("pr_open").unwrap_or(0.0),
                pr_high: row.f64("pr_high").unwrap_or(0.0),
                pr_low: row.f64("pr_low").unwrap_or(0.0),
                pr_close: row.f64("pr_close").unwrap_or(0.0),
                pr_std: row.f64("pr_std").unwrap_or(0.0),
                vol: row.f64("vol").unwrap_or(0.0),
                val: row.f64("val").unwrap_or(0.0),
                trades: row.f64("trades").unwrap_or(0.0),
                pr_vwap: row.f64("pr_vwap").unwrap_or(0.0),
                pr_change: row.f64("pr_change").unwrap_or(0.0),
                vol_b: row.f64("vol_b").unwrap_or(0.0),
                vol_s: row.f64("vol_s").unwrap_or(0.0),
                val_b: row.f64("val_b").unwrap_or(0.0),
                val_s: row.f64("val_s").unwrap_or(0.0),
                trades_b: row.f64("trades_b").unwrap_or(0.0),
                trades_s: row.f64("trades_s").unwrap_or(0.0),
                disb: row.f64("disb").unwrap_or(0.0),
                pr_vwap_b: row.f64("pr_vwap_b").unwrap_or(0.0),
                pr_vwap_s: row.f64("pr_vwap_s").unwrap_or(0.0),
            })
        })
        .collect()
}

/// Разобрать таблицу `futoi` в точки FUTOI. Строки без разбираемой группы
/// клиентов (`clgroup`, ожидается `fiz`/`yur`) пропускаются — учитываем
/// только физ/юр (как определяет [`ClientGroup`]).
pub fn parse_futoi(table: &IssTable) -> Vec<FutoiPoint> {
    table
        .rows_iter()
        .filter_map(|row| {
            let secid = row.str("secid")?.to_owned();
            let ts = row_ts(&row)?;
            let clgroup = ClientGroup::from_code(row.str("clgroup")?)?;
            Some(FutoiPoint {
                ts,
                secid,
                clgroup,
                pos: row.f64("pos").unwrap_or(0.0),
                pos_long: row.f64("pos_long").unwrap_or(0.0),
                pos_short: row.f64("pos_short").unwrap_or(0.0),
                pos_long_num: row.f64("pos_long_num").unwrap_or(0.0),
                pos_short_num: row.f64("pos_short_num").unwrap_or(0.0),
            })
        })
        .collect()
}

/// Разобрать таблицу `hi2` в точки индекса концентрации.
pub fn parse_hi2(table: &IssTable) -> Vec<Hi2Point> {
    table
        .rows_iter()
        .filter_map(|row| {
            let secid = row.str("secid")?.to_owned();
            let ts = row_ts(&row)?;
            let concentration = row.f64("hi2").or_else(|| row.f64("concentration"))?;
            Some(Hi2Point {
                ts,
                secid,
                concentration,
            })
        })
        .collect()
}

/// Разобрать таблицу `obstats` в точки статистики стакана. Все метрики,
/// кроме `ts`/`secid`, мягкие — отсутствующая колонка даёт `None`, а не
/// ошибку разбора.
pub fn parse_obstats(table: &IssTable) -> Vec<ObstatsPoint> {
    table
        .rows_iter()
        .filter_map(|row| {
            let secid = row.str("secid")?.to_owned();
            let ts = row_ts(&row)?;
            Some(ObstatsPoint {
                ts,
                secid,
                spread_bbo: row.f64("spread_bbo"),
                spread_lv10: row.f64("spread_lv10"),
                spread_1mio: row.f64("spread_1mio"),
                levels_b: row.f64("levels_b"),
                levels_s: row.f64("levels_s"),
                vol_b: row.f64("vol_b"),
                vol_s: row.f64("vol_s"),
                val_b: row.f64("val_b"),
                val_s: row.f64("val_s"),
                vwap_b: row.f64("vwap_b"),
                vwap_s: row.f64("vwap_s"),
            })
        })
        .collect()
}

/// Разобрать таблицу `orderstats` в точки статистики заявок. Все метрики,
/// кроме `ts`/`secid`, мягкие.
pub fn parse_orderstats(table: &IssTable) -> Vec<OrderstatsPoint> {
    table
        .rows_iter()
        .filter_map(|row| {
            let secid = row.str("secid")?.to_owned();
            let ts = row_ts(&row)?;
            Some(OrderstatsPoint {
                ts,
                secid,
                put_orders_b: row.f64("put_orders_b"),
                put_orders_s: row.f64("put_orders_s"),
                put_vol_b: row.f64("put_vol_b"),
                put_vol_s: row.f64("put_vol_s"),
                put_val_b: row.f64("put_val_b"),
                put_val_s: row.f64("put_val_s"),
                cancel_orders_b: row.f64("cancel_orders_b"),
                cancel_orders_s: row.f64("cancel_orders_s"),
                cancel_vol_b: row.f64("cancel_vol_b"),
                cancel_vol_s: row.f64("cancel_vol_s"),
                cancel_val_b: row.f64("cancel_val_b"),
                cancel_val_s: row.f64("cancel_val_s"),
            })
        })
        .collect()
}

/// OHLCV-свеча из задела под историю (`MoexAlgo::candles`, `(unverified)`).
/// Не связана с доменной `SuperCandle` — обычная биржевая свеча без
/// разбивки покупки/продажи.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IssCandle {
    pub ts: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Разобрать таблицу свечей (`(unverified)`, см. модульную документацию).
pub fn parse_candles(table: &IssTable) -> Vec<IssCandle> {
    table
        .rows_iter()
        .filter_map(|row| {
            let ts = row_ts(&row)?;
            Some(IssCandle {
                ts,
                open: row.f64("open").unwrap_or(0.0),
                high: row.f64("high").unwrap_or(0.0),
                low: row.f64("low").unwrap_or(0.0),
                close: row.f64("close").unwrap_or(0.0),
                volume: row.f64("volume").unwrap_or(0.0),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Value {
        let path = format!("{}/tests/fixtures/moex/{name}", env!("CARGO_MANIFEST_DIR"));
        let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{path}: невалидный JSON: {e}"))
    }

    #[test]
    fn moex_datetime_conversion_matches_known_instant() {
        // 2024-01-15 12:00:00 MSK == 2024-01-15 09:00:00 UTC.
        let ts = moex_datetime_to_unix("2024-01-15", "12:00:00").unwrap();
        // Известный unix-момент 2024-01-15T09:00:00Z.
        assert_eq!(ts, 1_705_309_200);
    }

    #[test]
    fn table_find_tries_candidates_in_order() {
        let v = serde_json::json!({
            "tradestats": {"columns": ["secid"], "data": [["SBER"]]}
        });
        let t = IssTable::find(&v, &["data", "tradestats"]).unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t.row(0).str("secid"), Some("SBER"));
    }

    #[test]
    fn table_find_returns_none_without_any_candidate() {
        let v = serde_json::json!({"unrelated": 1});
        assert!(IssTable::find(&v, &["data", "tradestats"]).is_none());
    }

    #[test]
    fn empty_data_block_parses_to_empty_table() {
        let v = serde_json::json!({"data": {"columns": ["secid"], "data": []}});
        let t = IssTable::find(&v, &["data"]).unwrap();
        assert!(t.is_empty());
        assert_eq!(parse_tradestats(&t), vec![]);
    }

    #[test]
    fn cursor_parses_and_reports_more_pages() {
        let v = serde_json::json!({
            "data": {"columns": ["secid"], "data": []},
            "data.cursor": {"columns": ["INDEX", "TOTAL", "PAGESIZE"], "data": [[0, 250, 100]]}
        });
        let c = IssCursor::find(&v, &["data"]).unwrap();
        assert_eq!(
            c,
            IssCursor {
                index: 0,
                total: 250,
                page_size: 100
            }
        );
        assert!(c.has_more());
        assert_eq!(c.next_start(), 100);
    }

    #[test]
    fn cursor_last_page_has_no_more() {
        let v = serde_json::json!({
            "data.cursor": {"columns": ["INDEX", "TOTAL", "PAGESIZE"], "data": [[200, 250, 100]]}
        });
        let c = IssCursor::find(&v, &["data"]).unwrap();
        assert!(!c.has_more());
    }

    #[test]
    fn cursor_missing_is_none() {
        let v = serde_json::json!({"data": {"columns": [], "data": []}});
        assert!(IssCursor::find(&v, &["data"]).is_none());
    }

    #[test]
    fn parse_tradestats_from_fixture() {
        let v = fixture("tradestats_eq.json");
        let t = IssTable::find(&v, &["data", "tradestats"]).unwrap();
        let candles = parse_tradestats(&t);
        assert!(!candles.is_empty());
        let c = &candles[0];
        assert_eq!(c.secid, "SBER");
        assert!(c.pr_close > 0.0);
        assert!(c.vol > 0.0);
    }

    #[test]
    fn parse_futoi_from_fixture() {
        let v = fixture("futoi_fo.json");
        let t = IssTable::find(&v, &["data", "futoi"]).unwrap();
        let points = parse_futoi(&t);
        assert!(!points.is_empty());
        assert!(points.iter().any(|p| p.clgroup == ClientGroup::Fiz));
        assert!(points.iter().any(|p| p.clgroup == ClientGroup::Yur));
    }

    #[test]
    fn parse_hi2_from_fixture() {
        let v = fixture("hi2_eq.json");
        let t = IssTable::find(&v, &["data", "hi2"]).unwrap();
        let points = parse_hi2(&t);
        assert!(!points.is_empty());
        assert!(points[0].concentration >= 0.0);
    }

    #[test]
    fn parse_obstats_from_fixture_is_soft_on_missing_fields() {
        let v = fixture("obstats_eq.json");
        let t = IssTable::find(&v, &["data", "obstats"]).unwrap();
        let points = parse_obstats(&t);
        assert!(!points.is_empty());
        // Фикстура намеренно опускает `spread_1mio` во второй строке.
        assert!(points.iter().any(|p| p.spread_1mio.is_none()));
        assert!(points.iter().any(|p| p.spread_bbo.is_some()));
    }

    #[test]
    fn parse_orderstats_from_fixture_is_soft_on_missing_fields() {
        let v = fixture("orderstats_eq.json");
        let t = IssTable::find(&v, &["data", "orderstats"]).unwrap();
        let points = parse_orderstats(&t);
        assert!(!points.is_empty());
        assert!(points.iter().any(|p| p.cancel_vol_b.is_none()));
    }

    #[test]
    fn parse_ignores_row_without_secid() {
        let v = serde_json::json!({
            "data": {
                "columns": ["secid", "tradedate", "tradetime", "vol"],
                "data": [[null, "2024-01-15", "10:00:00", 100]]
            }
        });
        let t = IssTable::find(&v, &["data"]).unwrap();
        assert_eq!(parse_tradestats(&t), vec![]);
    }
}
