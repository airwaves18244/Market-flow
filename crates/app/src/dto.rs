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

/// Ширина рынка: статистика по растущим/падающим бумагам.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BreadthDto {
    pub advancers: u32,
    pub decliners: u32,
    pub unchanged: u32,
    /// Доля растущих от всех (0..1), None если пусто.
    pub pct_advancing: Option<f64>,
    /// Ratio растущих к падающим, None если нет падающих.
    pub ad_ratio: Option<f64>,
}

/// Инструмент с наибольшим изменением (для топ-движений).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopMoverDto {
    pub symbol: String,
    pub ticker: String,
    pub name: String,
    pub sector: Option<String>,
    /// Изменение в долях: `0.05` = +5%, `-0.03` = -3%.
    pub change: f64,
    /// Последняя цена закрытия.
    pub last_close: f64,
}

/// Сектор на плоскости RRG.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RrgSectorDto {
    pub sector: String,
    pub rs_ratio: f64,
    pub rs_momentum: f64,
    /// Квадрант: `leading|weakening|lagging|improving`.
    pub quadrant: String,
}

/// Строка агрегации фьючерсов (по группам контрактов).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FutureGroupDto {
    pub group: String,
    pub contracts: u32,
    pub turnover: f64,
    pub net_flow: f64,
    pub weighted_change: f64,
    pub open_interest: f64,
}

/// Строка агрегации облигаций (по эмитентам/секторам).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BondIssuerDto {
    pub issuer: String,
    pub bonds: u32,
    pub turnover: f64,
    pub net_flow: f64,
    pub avg_yield: f64,
    pub weighted_duration: f64,
}

/// Точка кривой доходности (по срокам).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YieldCurvePoint {
    pub maturity_years: f64,
    pub yield_pct: f64,
}

/// Доля одного класса активов в общем обороте (сектор donut'а).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetClassShareDto {
    /// Код класса: `equity|future|bond`.
    pub asset_class: String,
    pub turnover: f64,
    /// Доля в общем обороте (0..1).
    pub share: f64,
}

/// Сводка «Сумма всех»: общий оборот + доли по классам (gauge + donut).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossAssetSummaryDto {
    /// Суммарный оборот по всем классам.
    pub total: f64,
    pub shares: Vec<AssetClassShareDto>,
}

/// Точка оборота по классам активов во времени (stacked area).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnoverByClassPoint {
    pub ts: i64,
    pub equity: f64,
    pub future: f64,
    pub bond: f64,
}

/// Ребро перетока доли между классами активов (Sankey).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowEdgeDto {
    /// Класс-источник (код).
    pub from: String,
    /// Класс-приёмник (код).
    pub to: String,
    /// Вес перетока — сдвиг доли (0..1).
    pub weight: f64,
}
