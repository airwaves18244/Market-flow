//! DTO для фронтенда — сериализуемые ответы IPC-команд.
//!
//! Это «провод» между Rust-ядром и вебвью: типы намеренно плоские и
//! `camelCase` (привычно для TypeScript), чтобы фронт получал готовые к
//! отрисовке структуры (treemap/heatmap/свечи/временные ряды) без доустройки.

use serde::Serialize;

use domain::Instrument;
use storage::store::TurnoverSnapshot;
use storage::SectorEntry;

/// Инструмент справочника (для списков/вотчлиста).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentDto {
    pub symbol: String,
    pub ticker: String,
    pub name: String,
    /// Код класса актива: `equity|future|bond`.
    pub asset_class: String,
    pub sector: Option<String>,
}

impl From<&Instrument> for InstrumentDto {
    fn from(i: &Instrument) -> Self {
        Self {
            symbol: i.symbol.clone(),
            ticker: i.ticker.clone(),
            name: i.name.clone(),
            asset_class: i.asset_class.code().to_string(),
            sector: i.sector.clone(),
        }
    }
}

/// Точка свечного графика (Lightweight Charts).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BarPoint {
    pub ts: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Точка временного ряда оборота/потока (для line/area-графиков).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnoverPoint {
    pub ts: i64,
    pub turnover: f64,
    pub net_flow: f64,
    /// Изменение в долях (`0.01` = +1%).
    pub change: f64,
}

impl From<&TurnoverSnapshot> for TurnoverPoint {
    fn from(s: &TurnoverSnapshot) -> Self {
        Self {
            ts: s.ts,
            turnover: s.turnover,
            net_flow: s.net_flow,
            change: s.change,
        }
    }
}

/// Строка секторной агрегации (плитка treemap / ячейка heatmap).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectorRow {
    pub sector: String,
    pub instruments: u32,
    /// Суммарный оборот сектора — размер плитки treemap.
    pub turnover: f64,
    pub net_flow: f64,
    /// Средневзвешенное по обороту изменение — цвет плитки (в долях).
    pub weighted_change: f64,
}

/// Запись классификации секторов (для редактора таблицы соответствий).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectorEntryDto {
    pub key: String,
    pub sector: String,
    pub is_isin: bool,
}

impl From<&SectorEntry> for SectorEntryDto {
    fn from(e: &SectorEntry) -> Self {
        Self {
            key: e.key.clone(),
            sector: e.sector.clone(),
            is_isin: e.is_isin,
        }
    }
}
