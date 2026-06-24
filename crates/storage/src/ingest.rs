//! Запись данных в хранилище (§ 1.3, § 1.5).
//!
//! Все операции идемпотентны (`INSERT ... ON CONFLICT DO UPDATE`) и выполняются
//! в одной транзакции на вызов — повторный ингест тех же данных не плодит
//! дубликаты и не требует предварительной очистки.

use std::time::{SystemTime, UNIX_EPOCH};

use duckdb::{params, Connection};

use crate::{Result, TimeFrame};

/// Снимок агрегированного оборота инструмента на момент сканирования.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurnoverSnapshot {
    pub ts: i64,
    /// Накопленный оборот за день.
    pub turnover: f64,
    pub net_flow: f64,
    /// Дневное изменение, доли.
    pub change: f64,
}

impl TurnoverSnapshot {
    pub fn new(ts: i64, turnover: f64, net_flow: f64, change: f64) -> Self {
        Self {
            ts,
            turnover,
            net_flow,
            change,
        }
    }
}

/// Снимок оборота вместе с символом инструмента (для батч-вставки).
#[derive(Debug, Clone)]
pub struct SymbolSnapshot {
    pub symbol: String,
    pub snapshot: TurnoverSnapshot,
}

/// Выполнить замыкание в транзакции: COMMIT при `Ok`, ROLLBACK при `Err`.
fn with_tx<T>(conn: &Connection, f: impl FnOnce() -> Result<T>) -> Result<T> {
    conn.execute_batch("BEGIN TRANSACTION")?;
    match f() {
        Ok(v) => {
            conn.execute_batch("COMMIT")?;
            Ok(v)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// Upsert справочника инструментов. Возвращает число обработанных записей.
pub fn upsert_instruments(conn: &Connection, instruments: &[domain::Instrument]) -> Result<usize> {
    const SQL: &str = "\
INSERT INTO instruments (symbol, ticker, name, asset_class, sector, lot_size, isin, updated_at)
VALUES (?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT (symbol) DO UPDATE SET
    ticker = excluded.ticker,
    name = excluded.name,
    asset_class = excluded.asset_class,
    sector = excluded.sector,
    lot_size = excluded.lot_size,
    isin = excluded.isin,
    updated_at = excluded.updated_at;";
    let now = now_unix();
    with_tx(conn, || {
        let mut stmt = conn.prepare(SQL)?;
        for i in instruments {
            stmt.execute(params![
                i.symbol,
                i.ticker,
                i.name,
                i.asset_class.code(),
                i.sector.as_deref(),
                i.lot_size as i32,
                i.isin.as_deref(),
                now,
            ])?;
        }
        Ok(instruments.len())
    })
}

/// Батч-вставка баров одного инструмента и тайм-фрейма.
pub fn insert_bars(
    conn: &Connection,
    symbol: &str,
    tf: TimeFrame,
    bars: &[domain::Bar],
) -> Result<usize> {
    const SQL: &str = "\
INSERT INTO bars (symbol, timeframe, ts, open, high, low, close, volume)
VALUES (?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT (symbol, timeframe, ts) DO UPDATE SET
    open = excluded.open,
    high = excluded.high,
    low = excluded.low,
    close = excluded.close,
    volume = excluded.volume;";
    let code = tf.code();
    with_tx(conn, || {
        let mut stmt = conn.prepare(SQL)?;
        for b in bars {
            stmt.execute(params![
                symbol, code, b.ts, b.open, b.high, b.low, b.close, b.volume,
            ])?;
        }
        Ok(bars.len())
    })
}

/// Батч-вставка снимков оборота.
pub fn insert_turnover_snapshots(conn: &Connection, snapshots: &[SymbolSnapshot]) -> Result<usize> {
    const SQL: &str = "\
INSERT INTO turnover_snapshots (symbol, ts, turnover, net_flow, change)
VALUES (?, ?, ?, ?, ?)
ON CONFLICT (symbol, ts) DO UPDATE SET
    turnover = excluded.turnover,
    net_flow = excluded.net_flow,
    change = excluded.change;";
    with_tx(conn, || {
        let mut stmt = conn.prepare(SQL)?;
        for s in snapshots {
            stmt.execute(params![
                s.symbol,
                s.snapshot.ts,
                s.snapshot.turnover,
                s.snapshot.net_flow,
                s.snapshot.change,
            ])?;
        }
        Ok(snapshots.len())
    })
}

/// Загрузить пары «ключ → сектор» в таблицу классификации.
/// ISIN определяется по форме (12 символов, первые два — буквы).
pub fn upsert_sector_map<'a>(
    conn: &Connection,
    pairs: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<usize> {
    const SQL: &str = "\
INSERT INTO sector_map (key, sector, is_isin)
VALUES (?, ?, ?)
ON CONFLICT (key) DO UPDATE SET
    sector = excluded.sector,
    is_isin = excluded.is_isin;";
    with_tx(conn, || {
        let mut stmt = conn.prepare(SQL)?;
        let mut n = 0usize;
        for (key, sector) in pairs {
            stmt.execute(params![key, sector, is_isin(key)])?;
            n += 1;
        }
        Ok(n)
    })
}

/// Проставить `instruments.sector` из таблицы классификации.
/// Приоритет у соответствия по тикеру (`is_isin = FALSE`) над ISIN.
/// Инструменты без совпадения не трогаются. Возвращает число обновлённых строк.
pub fn apply_sectors_to_instruments(conn: &Connection) -> Result<usize> {
    const SQL: &str = "\
UPDATE instruments SET sector = (
    SELECT m.sector FROM sector_map m
    WHERE (m.is_isin = FALSE AND upper(m.key) = upper(instruments.ticker))
       OR (m.is_isin = TRUE AND instruments.isin IS NOT NULL AND m.key = instruments.isin)
    ORDER BY m.is_isin ASC
    LIMIT 1
)
WHERE EXISTS (
    SELECT 1 FROM sector_map m
    WHERE (m.is_isin = FALSE AND upper(m.key) = upper(instruments.ticker))
       OR (m.is_isin = TRUE AND instruments.isin IS NOT NULL AND m.key = instruments.isin)
);";
    let n = conn.execute(SQL, [])?;
    Ok(n)
}

/// Грубая проверка формата ISIN: 12 символов, первые два — латинские буквы,
/// остальные — буквы/цифры. Совпадает с правилом в `data::classify`.
fn is_isin(s: &str) -> bool {
    s.len() == 12
        && s.chars().take(2).all(|c| c.is_ascii_alphabetic())
        && s.chars().skip(2).all(|c| c.is_ascii_alphanumeric())
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
