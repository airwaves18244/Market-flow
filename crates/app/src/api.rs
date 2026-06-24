//! View-model для фронта (Фаза 3, § 3.2).
//!
//! Чистые, сериализуемые функции поверх [`storage::Db`] — это контракт, который
//! Tauri-команды оборачивают на десктоп-цели (там, где доступен webview). Слой
//! **не зависит** от `tauri`, поэтому собирается и тестируется в CI на Linux;
//! на десктопе тонкие `#[tauri::command]`-обёртки просто вызывают эти функции и
//! отдают результат во фронт через `invoke`.
//!
//! Формы DTO ниже совпадают с типами в `frontend/src/lib/types.ts`.

use serde::Serialize;

use storage::query::{FlowPoint, Mover, SectorTurnover};
use storage::Db;

/// Снимок представления «Акции / секторы» за период `[from_ts, to_ts]`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EquityDashboard {
    pub from_ts: i64,
    pub to_ts: i64,
    /// Оборот и нетто-поток по секторам (по убыванию оборота).
    pub sectors: Vec<SectorTurnover>,
    /// Топ-движения по модулю изменения.
    pub top_movers: Vec<Mover>,
}

/// Собрать снимок дашборда акций: обороты по секторам + топ-движения.
pub fn equity_dashboard(
    db: &Db,
    from_ts: i64,
    to_ts: i64,
    movers_limit: usize,
) -> storage::Result<EquityDashboard> {
    Ok(EquityDashboard {
        from_ts,
        to_ts,
        sectors: db.turnover_by_sector(from_ts, to_ts)?,
        top_movers: db.top_movers(from_ts, to_ts, movers_limit)?,
    })
}

/// Временной ряд нетто-потока инструмента за период (для линейного графика).
pub fn flow_series(
    db: &Db,
    symbol: &str,
    from_ts: i64,
    to_ts: i64,
) -> storage::Result<Vec<FlowPoint>> {
    db.net_flow_series(symbol, from_ts, to_ts)
}

#[cfg(test)]
mod tests {
    use domain::{AssetClass, Instrument};
    use storage::ingest::{SymbolSnapshot, TurnoverSnapshot};

    use super::*;

    fn seeded_db() -> Db {
        let db = Db::open_in_memory().unwrap();
        let instruments = [
            Instrument {
                symbol: "SBER@MISX".into(),
                ticker: "SBER".into(),
                name: "Сбербанк".into(),
                asset_class: AssetClass::Equity,
                sector: None,
                lot_size: 10,
                isin: None,
            },
            Instrument {
                symbol: "GAZP@MISX".into(),
                ticker: "GAZP".into(),
                name: "Газпром".into(),
                asset_class: AssetClass::Equity,
                sector: None,
                lot_size: 10,
                isin: None,
            },
        ];
        db.upsert_instruments(&instruments).unwrap();
        db.upsert_sector_map([("SBER", "Финансы"), ("GAZP", "Нефтегаз")])
            .unwrap();
        db.apply_sectors_to_instruments().unwrap();
        db.insert_turnover_snapshots(&[
            SymbolSnapshot {
                symbol: "SBER@MISX".into(),
                snapshot: TurnoverSnapshot::new(1000, 800.0, 120.0, 0.012),
            },
            SymbolSnapshot {
                symbol: "GAZP@MISX".into(),
                snapshot: TurnoverSnapshot::new(1000, 300.0, -40.0, -0.030),
            },
        ])
        .unwrap();
        db
    }

    #[test]
    fn equity_dashboard_assembles_sectors_and_movers() {
        let db = seeded_db();
        let snap = equity_dashboard(&db, 0, 10_000, 10).unwrap();
        assert_eq!(snap.from_ts, 0);
        assert_eq!(snap.to_ts, 10_000);
        // Финансы (оборот 800) идут раньше Нефтегаза (300).
        assert_eq!(snap.sectors[0].sector, "Финансы");
        // Топ-движение по модулю — GAZP (0.030 против 0.012).
        assert_eq!(snap.top_movers[0].symbol, "GAZP@MISX");
    }

    #[test]
    fn flow_series_returns_time_ordered_points() {
        let db = seeded_db();
        let series = flow_series(&db, "SBER@MISX", 0, 10_000).unwrap();
        assert_eq!(series.len(), 1);
        assert_eq!(series[0].ts, 1000);
        assert_eq!(series[0].net_flow, 120.0);
    }
}
