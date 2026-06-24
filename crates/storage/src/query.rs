//! Аналитические запросы (§ 1.7).
//!
//! Запросы возвращают готовые к сериализации строки. Семантика по обороту
//! опирается на таблицу `turnover_snapshots` (снимки накопленного дневного
//! оборота), которую наполняет планировщик ингеста.

use duckdb::Connection;
use serde::Serialize;

use crate::Result;

/// Оборот и нетто-поток по сектору за период.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SectorTurnover {
    pub sector: String,
    pub turnover: f64,
    pub net_flow: f64,
}

/// Запись топ-движения за период.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Mover {
    pub symbol: String,
    pub name: String,
    /// Изменение из последнего снимка периода, доли.
    pub change: f64,
    pub turnover: f64,
}

/// Точка временного ряда нетто-потока.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct FlowPoint {
    pub ts: i64,
    pub net_flow: f64,
}

/// Оборот и нетто-поток по секторам за `[from_ts, to_ts]` (суммы снимков в
/// диапазоне). Инструменты без сектора собираются в группу «Прочее».
pub fn turnover_by_sector(
    conn: &Connection,
    from_ts: i64,
    to_ts: i64,
) -> Result<Vec<SectorTurnover>> {
    const SQL: &str = "\
SELECT COALESCE(i.sector, 'Прочее') AS sector,
       SUM(s.turnover) AS turnover,
       SUM(s.net_flow) AS net_flow
FROM turnover_snapshots s
JOIN instruments i ON i.symbol = s.symbol
WHERE s.ts >= ? AND s.ts <= ?
GROUP BY 1
ORDER BY turnover DESC;";
    let mut stmt = conn.prepare(SQL)?;
    let rows = stmt.query_map([from_ts, to_ts], |row| {
        Ok(SectorTurnover {
            sector: row.get(0)?,
            turnover: row.get(1)?,
            net_flow: row.get(2)?,
        })
    })?;
    collect(rows)
}

/// Топ-движения за период по модулю изменения. Для каждого инструмента берётся
/// последний снимок в диапазоне (по `ts`).
pub fn top_movers(conn: &Connection, from_ts: i64, to_ts: i64, limit: usize) -> Result<Vec<Mover>> {
    const SQL: &str = "\
SELECT s.symbol, i.name, s.change, s.turnover
FROM turnover_snapshots s
JOIN instruments i ON i.symbol = s.symbol
WHERE s.ts >= ? AND s.ts <= ?
QUALIFY ROW_NUMBER() OVER (PARTITION BY s.symbol ORDER BY s.ts DESC) = 1
ORDER BY abs(s.change) DESC
LIMIT ?;";
    let mut stmt = conn.prepare(SQL)?;
    let rows = stmt.query_map(params_limit(from_ts, to_ts, limit), |row| {
        Ok(Mover {
            symbol: row.get(0)?,
            name: row.get(1)?,
            change: row.get(2)?,
            turnover: row.get(3)?,
        })
    })?;
    collect(rows)
}

/// Временной ряд нетто-потока инструмента за период (по возрастанию времени).
pub fn net_flow_series(
    conn: &Connection,
    symbol: &str,
    from_ts: i64,
    to_ts: i64,
) -> Result<Vec<FlowPoint>> {
    const SQL: &str = "\
SELECT ts, net_flow
FROM turnover_snapshots
WHERE symbol = ? AND ts >= ? AND ts <= ?
ORDER BY ts ASC;";
    let mut stmt = conn.prepare(SQL)?;
    let rows = stmt.query_map(
        duckdb::params![symbol, from_ts, to_ts],
        |row| {
            Ok(FlowPoint {
                ts: row.get(0)?,
                net_flow: row.get(1)?,
            })
        },
    )?;
    collect(rows)
}

fn params_limit(from_ts: i64, to_ts: i64, limit: usize) -> impl duckdb::Params {
    duckdb::params_from_iter([from_ts, to_ts, limit as i64])
}

fn collect<T>(
    rows: impl Iterator<Item = std::result::Result<T, duckdb::Error>>,
) -> Result<Vec<T>> {
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}
