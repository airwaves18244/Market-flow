//! DTO для фронтенда — сериализуемые ответы IPC-команд.
//!
//! Это «провод» между Rust-ядром и вебвью: типы намеренно плоские и
//! `camelCase` (привычно для TypeScript), чтобы фронт получал готовые к
//! отрисовке структуры (treemap/heatmap/свечи/временные ряды) без доустройки.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use domain::backtest::{
    BacktestConfig, BacktestReport, FillTiming, PerfMetrics, SimTrade, StrategyDescriptor,
};
use domain::delta::{FootprintBar, RobotConfig, RobotSignal};
use domain::history::{DatasetMeta, TimeRange};
use domain::keyactivity::{KeyActivityRow, Sample};
use domain::metrics::alerts::{AlertCondition, AlertEvent, AlertRule};
use domain::options::{Greeks, LegKind, OptionType, PriceModel, Side as OptSide};
use domain::trading::{Fill, Order, OrderType, Position, TimeInForce};
use domain::{BookLevel, Instrument, OrderBook, Side, Trade};
use storage::store::TurnoverSnapshot;
use storage::SectorEntry;

/// Код стороны сделки/заявки для фронта (`buy|sell`).
fn side_code(side: Side) -> &'static str {
    match side {
        Side::Buy => "buy",
        Side::Sell => "sell",
    }
}

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

// ── Фаза 7 — live-панели (Time&Sales / DOM / алёрты) ───────────────────────

/// Обезличенная сделка для ленты Time&Sales.
///
/// Потребляется UI/live-push слоем (Tauri); в headless-live режиме (`live` без
/// `tauri`) лента не строится, поэтому там тип не конструируется.
#[cfg_attr(feature = "live", allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeDto {
    pub ts: i64,
    pub price: f64,
    pub size: f64,
    /// Сторона-инициатор: `true` — покупка (агрессор-бид), `false` — продажа,
    /// `None` — биржа не отдаёт сторону.
    pub buyer_initiated: Option<bool>,
}

impl From<&Trade> for TradeDto {
    fn from(t: &Trade) -> Self {
        Self {
            ts: t.ts,
            price: t.price,
            size: t.size,
            buyer_initiated: t.buyer_initiated,
        }
    }
}

/// Уровень стакана (цена + совокупный объём) для DOM-лесенки.
#[cfg_attr(feature = "live", allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BookLevelDto {
    pub price: f64,
    pub size: f64,
}

impl From<&BookLevel> for BookLevelDto {
    fn from(l: &BookLevel) -> Self {
        Self {
            price: l.price,
            size: l.size,
        }
    }
}

/// Снимок стакана (DOM): биды (по убыванию цены) и аски (по возрастанию).
#[cfg_attr(feature = "live", allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OrderBookDto {
    pub ts: i64,
    pub bids: Vec<BookLevelDto>,
    pub asks: Vec<BookLevelDto>,
}

impl From<&OrderBook> for OrderBookDto {
    fn from(b: &OrderBook) -> Self {
        Self {
            ts: b.ts,
            bids: b.bids.iter().map(BookLevelDto::from).collect(),
            asks: b.asks.iter().map(BookLevelDto::from).collect(),
        }
    }
}

/// Сработавший алёрт для панели уведомлений.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlertEventDto {
    pub symbol: String,
    pub ts: i64,
    pub price: f64,
    /// Дневное изменение в долях (`0.01` = +1%).
    pub change: f64,
    /// Человекочитаемое описание сработавшего условия.
    pub message: String,
}

impl From<&AlertEvent> for AlertEventDto {
    fn from(e: &AlertEvent) -> Self {
        Self {
            symbol: e.symbol.clone(),
            ts: e.ts,
            price: e.price,
            change: e.change,
            message: e.message.clone(),
        }
    }
}

/// Правило алёрта, приходящее с фронта (вход IPC).
///
/// Плоское представление доменного [`AlertRule`]: `kind` выбирает условие,
/// `threshold` — порог (цена или доля изменения, в зависимости от `kind`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertRuleInput {
    pub symbol: String,
    /// `priceAbove` | `priceBelow` | `changeAbove` | `changeBelow`.
    pub kind: String,
    pub threshold: f64,
}

impl AlertRuleInput {
    /// Преобразовать в доменное правило; `None` при неизвестном `kind`.
    pub fn to_rule(&self) -> Option<AlertRule> {
        let condition = match self.kind.as_str() {
            "priceAbove" => AlertCondition::PriceAbove(self.threshold),
            "priceBelow" => AlertCondition::PriceBelow(self.threshold),
            "changeAbove" => AlertCondition::ChangeAbove(self.threshold),
            "changeBelow" => AlertCondition::ChangeBelow(self.threshold),
            _ => return None,
        };
        Some(AlertRule::new(self.symbol.clone(), condition))
    }
}

// ── V2 / Бэктестер ─────────────────────────────────────────────────────────

/// Описание параметра стратегии (для формы настроек в UI).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyParamDto {
    pub name: String,
    pub label: String,
    pub default: f64,
}

/// Описание стратегии бэктестера: id, подпись и схема параметров.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyDescriptorDto {
    pub id: String,
    pub label: String,
    pub params: Vec<StrategyParamDto>,
}

impl From<&StrategyDescriptor> for StrategyDescriptorDto {
    fn from(d: &StrategyDescriptor) -> Self {
        Self {
            id: d.id.to_string(),
            label: d.label.to_string(),
            params: d
                .params
                .iter()
                .map(|p| StrategyParamDto {
                    name: p.name.to_string(),
                    label: p.label.to_string(),
                    default: p.default,
                })
                .collect(),
        }
    }
}

/// Параметры прогона бэктеста, приходящие с фронта (вход IPC).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestConfigInput {
    pub initial_capital: f64,
    pub commission: f64,
    pub slippage: f64,
    /// `nextOpen` (по умолчанию) | `thisClose`.
    #[serde(default)]
    pub fill_timing: Option<FillTimingInput>,
}

/// Режим момента исполнения сигнала (вход IPC).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FillTimingInput {
    NextOpen,
    ThisClose,
}

impl BacktestConfigInput {
    /// Перевести в доменный конфиг бэктеста.
    pub fn to_config(self) -> BacktestConfig {
        BacktestConfig {
            initial_capital: self.initial_capital,
            commission: self.commission,
            slippage: self.slippage,
            fill_timing: match self.fill_timing {
                Some(FillTimingInput::ThisClose) => FillTiming::ThisClose,
                _ => FillTiming::NextOpen,
            },
        }
    }
}

/// Одна смоделированная сделка бэктеста.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimTradeDto {
    pub ts: i64,
    /// `buy|sell`.
    pub side: String,
    pub qty: f64,
    pub price: f64,
    pub realized_pnl: f64,
}

impl From<&SimTrade> for SimTradeDto {
    fn from(t: &SimTrade) -> Self {
        Self {
            ts: t.ts,
            side: side_code(t.side).to_string(),
            qty: t.qty,
            price: t.price,
            realized_pnl: t.realized_pnl,
        }
    }
}

/// Точка кривой капитала (`ts`, `equity`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct EquityPointDto {
    pub ts: i64,
    pub equity: f64,
}

/// Метрики эффективности стратегии.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfMetricsDto {
    pub net_pnl: f64,
    pub return_pct: f64,
    pub trades: usize,
    pub wins: usize,
    pub losses: usize,
    pub win_rate: f64,
    /// Может быть `Infinity` — сериализуется как null; фронт трактует как «∞».
    pub profit_factor: f64,
    pub max_drawdown: f64,
    pub sharpe: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
}

impl From<&PerfMetrics> for PerfMetricsDto {
    fn from(m: &PerfMetrics) -> Self {
        Self {
            net_pnl: m.net_pnl,
            return_pct: m.return_pct,
            trades: m.trades,
            wins: m.wins,
            losses: m.losses,
            win_rate: m.win_rate,
            profit_factor: m.profit_factor,
            max_drawdown: m.max_drawdown,
            sharpe: m.sharpe,
            avg_win: m.avg_win,
            avg_loss: m.avg_loss,
        }
    }
}

/// Полный отчёт бэктеста для фронта.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BacktestReportDto {
    pub trades: Vec<SimTradeDto>,
    pub equity_curve: Vec<EquityPointDto>,
    pub metrics: PerfMetricsDto,
}

impl From<&BacktestReport> for BacktestReportDto {
    fn from(r: &BacktestReport) -> Self {
        Self {
            trades: r.trades.iter().map(SimTradeDto::from).collect(),
            equity_curve: r
                .equity_curve
                .iter()
                .map(|&(ts, equity)| EquityPointDto { ts, equity })
                .collect(),
            metrics: PerfMetricsDto::from(&r.metrics),
        }
    }
}

// ── V2 / Delta (footprint + роботы) ─────────────────────────────────────────

/// Ячейка footprint: объём на уровне по сторонам агрессора.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FootprintCellDto {
    pub price: f64,
    pub bid_volume: f64,
    pub ask_volume: f64,
    pub delta: f64,
}

/// Footprint одного бара для оверлея дельты.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FootprintBarDto {
    pub ts: i64,
    pub cells: Vec<FootprintCellDto>,
    pub bid_total: f64,
    pub ask_total: f64,
    pub delta: f64,
    pub cumulative_delta: f64,
}

impl From<&FootprintBar> for FootprintBarDto {
    fn from(b: &FootprintBar) -> Self {
        Self {
            ts: b.ts,
            cells: b
                .cells
                .iter()
                .map(|c| FootprintCellDto {
                    price: c.price,
                    bid_volume: c.bid_volume,
                    ask_volume: c.ask_volume,
                    delta: c.delta(),
                })
                .collect(),
            bid_total: b.bid_total,
            ask_total: b.ask_total,
            delta: b.delta,
            cumulative_delta: b.cumulative_delta,
        }
    }
}

/// Сигнал детектирующего робота (маркер на графике дельты).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RobotSignalDto {
    /// Вид: `same_lot|iceberg|absorption`.
    pub kind: String,
    pub ts: i64,
    pub price: f64,
    pub strength: f64,
    pub note: String,
}

impl From<&RobotSignal> for RobotSignalDto {
    fn from(s: &RobotSignal) -> Self {
        Self {
            kind: s.kind.code().to_string(),
            ts: s.ts,
            price: s.price,
            strength: s.strength,
            note: s.note.clone(),
        }
    }
}

/// Настройки детекторов, приходящие с фронта (вход IPC). Все поля
/// необязательные — отсутствующие берутся из значений по умолчанию.
#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RobotConfigInput {
    pub same_lot_enabled: Option<bool>,
    pub same_lot_run: Option<usize>,
    pub lot_tolerance: Option<f64>,
    pub iceberg_enabled: Option<bool>,
    pub iceberg_volume_mult: Option<f64>,
    pub absorption_enabled: Option<bool>,
    pub absorption_min_delta: Option<f64>,
    pub absorption_max_move: Option<f64>,
}

impl RobotConfigInput {
    /// Перевести в доменный конфиг, подставляя значения по умолчанию.
    pub fn to_config(self) -> RobotConfig {
        let d = RobotConfig::default();
        RobotConfig {
            same_lot_enabled: self.same_lot_enabled.unwrap_or(d.same_lot_enabled),
            same_lot_run: self.same_lot_run.unwrap_or(d.same_lot_run),
            lot_tolerance: self.lot_tolerance.unwrap_or(d.lot_tolerance),
            iceberg_enabled: self.iceberg_enabled.unwrap_or(d.iceberg_enabled),
            iceberg_volume_mult: self.iceberg_volume_mult.unwrap_or(d.iceberg_volume_mult),
            absorption_enabled: self.absorption_enabled.unwrap_or(d.absorption_enabled),
            absorption_min_delta: self.absorption_min_delta.unwrap_or(d.absorption_min_delta),
            absorption_max_move: self.absorption_max_move.unwrap_or(d.absorption_max_move),
        }
    }
}

// ── V2 / Trade (симулятор исполнения) ───────────────────────────────────────

/// Заявка на постановку, приходящая с фронта (вход IPC).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderInput {
    pub symbol: String,
    /// `buy|sell`.
    pub side: String,
    pub qty: f64,
    /// `market|limit|stop`.
    pub kind: String,
    /// Цена для limit/stop.
    pub price: Option<f64>,
    /// `gtc|day|ioc` (по умолчанию `gtc`).
    pub tif: Option<String>,
}

impl OrderInput {
    /// Разобрать сторону заявки.
    pub fn parse_side(&self) -> Option<Side> {
        match self.side.as_str() {
            "buy" => Some(Side::Buy),
            "sell" => Some(Side::Sell),
            _ => None,
        }
    }

    /// Разобрать тип заявки.
    pub fn parse_kind(&self) -> Option<OrderType> {
        match self.kind.as_str() {
            "market" => Some(OrderType::Market),
            "limit" => Some(OrderType::Limit),
            "stop" => Some(OrderType::Stop),
            _ => None,
        }
    }

    /// Разобрать TIF (по умолчанию GTC).
    pub fn parse_tif(&self) -> TimeInForce {
        match self.tif.as_deref() {
            Some("ioc") => TimeInForce::Ioc,
            Some("day") => TimeInForce::Day,
            _ => TimeInForce::Gtc,
        }
    }
}

/// Заявка для блоттера.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderDto {
    pub id: u64,
    pub symbol: String,
    /// `buy|sell`.
    pub side: String,
    pub qty: f64,
    pub filled: f64,
    pub price: Option<f64>,
    /// `market|limit|stop`.
    pub kind: String,
    /// `new|partially_filled|filled|cancelled|rejected`.
    pub status: String,
}

impl From<&Order> for OrderDto {
    fn from(o: &Order) -> Self {
        Self {
            id: o.id,
            symbol: o.symbol.clone(),
            side: side_code(o.side).to_string(),
            qty: o.qty,
            filled: o.filled,
            price: o.price,
            kind: match o.kind {
                OrderType::Market => "market",
                OrderType::Limit => "limit",
                OrderType::Stop => "stop",
            }
            .to_string(),
            status: o.status.code().to_string(),
        }
    }
}

/// Факт исполнения (событие `fill:tick` и ответ на постановку).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FillEventDto {
    pub order_id: u64,
    pub ts: i64,
    /// `buy|sell`.
    pub side: &'static str,
    pub qty: f64,
    pub price: f64,
    pub realized_pnl: f64,
}

impl From<&Fill> for FillEventDto {
    fn from(f: &Fill) -> Self {
        Self {
            order_id: f.order_id,
            ts: f.ts,
            side: side_code(f.side),
            qty: f.qty,
            price: f.price,
            realized_pnl: f.realized_pnl,
        }
    }
}

/// Позиция по инструменту для таблицы позиций.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionDto {
    pub symbol: String,
    pub qty: f64,
    pub avg_price: f64,
}

/// Состояние счёта (наличность + реализованный P&L).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDto {
    pub cash: f64,
    pub realized_pnl: f64,
}

/// Результат постановки заявки: итог заявки + исполнения.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitResultDto {
    pub order: OrderDto,
    pub fills: Vec<FillEventDto>,
}

impl PositionDto {
    /// Собрать DTO из доменной позиции.
    pub fn new(symbol: &str, pos: &Position) -> Self {
        Self {
            symbol: symbol.to_string(),
            qty: pos.qty,
            avg_price: pos.avg_price,
        }
    }
}

// ── Фаза 12 — Опционы (калькулятор · улыбка · конструктор стратегий) ─────────

/// Разобрать тип опциона из кода фронта (`call|put`).
fn parse_option_type(code: &str) -> Option<OptionType> {
    match code {
        "call" => Some(OptionType::Call),
        "put" => Some(OptionType::Put),
        _ => None,
    }
}

/// Разобрать модель ценообразования (`black76|bachelier`, по умолчанию Блэк-76).
pub(crate) fn parse_price_model(code: Option<&str>) -> PriceModel {
    match code {
        Some("bachelier") => PriceModel::Bachelier,
        _ => PriceModel::Black76,
    }
}

/// Греки опциона/портфеля для фронта.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GreeksDto {
    pub delta: f64,
    pub gamma: f64,
    pub vega: f64,
    pub theta: f64,
    pub rho: f64,
}

impl From<Greeks> for GreeksDto {
    fn from(g: Greeks) -> Self {
        Self {
            delta: g.delta,
            gamma: g.gamma,
            vega: g.vega,
            theta: g.theta,
            rho: g.rho,
        }
    }
}

/// Вход калькулятора цены/греков опциона.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionPriceInput {
    /// Форвард базового актива.
    pub forward: f64,
    pub strike: f64,
    /// Время до экспирации в годах.
    pub t: f64,
    /// Волатильность (доля для Блэка, абсолютная для Башелье).
    pub vol: f64,
    /// Ставка дисконта (по умолчанию 0 — MOEX-маржируемые).
    pub rate: Option<f64>,
    /// `call|put`.
    pub kind: String,
    /// `black76|bachelier`.
    pub model: Option<String>,
}

impl OptionPriceInput {
    pub fn parse_kind(&self) -> Option<OptionType> {
        parse_option_type(&self.kind)
    }
    pub fn parse_model(&self) -> PriceModel {
        parse_price_model(self.model.as_deref())
    }
    pub fn rate_or_zero(&self) -> f64 {
        self.rate.unwrap_or(0.0)
    }
}

/// Результат калькулятора: теоретическая цена + греки.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionPriceDto {
    pub price: f64,
    pub greeks: GreeksDto,
}

/// Вход решателя подразумеваемой волатильности.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpliedVolInput {
    pub market_price: f64,
    pub forward: f64,
    pub strike: f64,
    pub t: f64,
    pub rate: Option<f64>,
    pub kind: String,
    pub model: Option<String>,
}

impl ImpliedVolInput {
    pub fn parse_kind(&self) -> Option<OptionType> {
        parse_option_type(&self.kind)
    }
}

/// Результат решателя IV (`None` → недостижимо положительной волатильностью).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpliedVolDto {
    pub iv: Option<f64>,
}

/// Рыночная точка улыбки (вход калибровки).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmilePointInput {
    pub strike: f64,
    pub iv: f64,
    /// Вес точки (ликвидность/OI); по умолчанию 1.
    pub weight: Option<f64>,
}

/// Вход калибровки улыбки.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmileFitInput {
    /// `moex|sabr|svi|kalenkovich`.
    pub model: String,
    pub points: Vec<SmilePointInput>,
    pub forward: f64,
    pub t: f64,
    /// Границы страйков для генерации кривой наложения (по умолчанию — по точкам).
    pub curve_lo: Option<f64>,
    pub curve_hi: Option<f64>,
    /// Число точек кривой (по умолчанию 41).
    pub curve_steps: Option<usize>,
}

/// Именованный параметр подгонки улыбки.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmileParamDto {
    pub name: String,
    pub value: f64,
}

/// Точка кривой улыбки (страйк → IV).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmileCurvePoint {
    pub strike: f64,
    pub iv: f64,
}

/// Результат калибровки улыбки: параметры, RMSE, сглаженная кривая наложения.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmileFitDto {
    /// `moex|sabr|svi|kalenkovich`.
    pub model: String,
    pub params: Vec<SmileParamDto>,
    pub rmse: f64,
    pub curve: Vec<SmileCurvePoint>,
}

/// Нога опционной стратегии (вход конструктора).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyLegInput {
    /// `call|put|underlying`.
    pub kind: String,
    /// `long|short`.
    pub side: String,
    pub strike: f64,
    pub expiry_t: f64,
    pub quantity: f64,
    pub entry_price: f64,
}

impl StrategyLegInput {
    pub fn parse_kind(&self) -> Option<LegKind> {
        match self.kind.as_str() {
            "call" => Some(LegKind::Call),
            "put" => Some(LegKind::Put),
            "underlying" => Some(LegKind::Underlying),
            _ => None,
        }
    }
    pub fn parse_side(&self) -> Option<OptSide> {
        match self.side.as_str() {
            "long" => Some(OptSide::Long),
            "short" => Some(OptSide::Short),
            _ => None,
        }
    }
}

/// Вход оценки стратегии.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyEvalInput {
    pub legs: Vec<StrategyLegInput>,
    /// Границы диаграммы payoff (цена базового).
    pub price_lo: f64,
    pub price_hi: f64,
    /// Число точек диаграммы (по умолчанию 61).
    pub steps: Option<usize>,
    /// Форвард/волатильность/модель для текущего P&L и агрегированных греков.
    pub forward: f64,
    pub vol: f64,
    pub rate: Option<f64>,
    /// `black76|bachelier`.
    pub model: Option<String>,
}

/// Точка диаграммы payoff: P&L на экспирацию и текущий.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyPayoffPoint {
    pub price: f64,
    pub pnl_expiry: f64,
    pub pnl_now: f64,
}

/// Результат оценки стратегии для фронта.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyEvalDto {
    pub breakevens: Vec<f64>,
    pub max_profit: Option<f64>,
    pub max_loss: Option<f64>,
    pub net_cost: f64,
    pub payoff: Vec<StrategyPayoffPoint>,
    pub greeks: GreeksDto,
}

/// Описание модели улыбки для UI (селектор моделей).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmileModelDto {
    /// Код: `moex|sabr|svi|kalenkovich`.
    pub id: String,
    /// Человекочитаемое имя.
    pub name: String,
}

// ── Фаза 10 — MOEX ALGO: Key Activity (ключевая активность) ──────────────────

/// Образец метрик инструмента за период (вход движка Key Activity). Приходит с
/// фронта в camelCase; в боевом режиме собирается из датасетов ALGOPACK.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyActivitySampleInput {
    pub secid: String,
    pub ts: i64,
    #[serde(default)]
    pub volume: f64,
    #[serde(default)]
    pub volume_z: f64,
    #[serde(default)]
    pub disb: f64,
    #[serde(default)]
    pub oi_change: f64,
    #[serde(default)]
    pub hi2: f64,
    #[serde(default)]
    pub spread: f64,
    #[serde(default)]
    pub price_change: f64,
}

impl From<&KeyActivitySampleInput> for Sample {
    fn from(s: &KeyActivitySampleInput) -> Self {
        Sample {
            secid: s.secid.clone(),
            asset_class: None,
            ts: s.ts,
            volume: s.volume,
            volume_z: s.volume_z,
            disb: s.disb,
            oi_change: s.oi_change,
            hi2: s.hi2,
            spread: s.spread,
            price_change: s.price_change,
        }
    }
}

/// Строка таблицы «Ключевая активность» для фронта.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyActivityRowDto {
    pub secid: String,
    pub rule_id: String,
    pub rule_name: String,
    /// Человекочитаемая подпись первичной метрики правила.
    pub metric: String,
    pub value: f64,
    pub ts: i64,
    pub importance: f64,
}

impl From<&KeyActivityRow> for KeyActivityRowDto {
    fn from(r: &KeyActivityRow) -> Self {
        Self {
            secid: r.secid.clone(),
            rule_id: r.rule_id.clone(),
            rule_name: r.rule_name.clone(),
            metric: r.metric.label().to_string(),
            value: r.value,
            ts: r.ts,
            importance: r.importance,
        }
    }
}

/// Итоговое ИИ-резюме по ключевой активности (панель «ИТОГО»). В отсутствие
/// LLM-ключа/сети — локально собранный текстовый свод (`fallback`/`source`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyActivitySummaryDto {
    /// Текст резюме (markdown/plain).
    pub text: String,
    /// Подпись периода (`1h|1d|1w|1m|3m`).
    pub period: String,
    /// Число строк ключевой активности, попавших в свод.
    pub row_count: usize,
    /// Локальный свод (`true`) vs. ответ LLM (`false`). Сохранён для
    /// обратной совместимости с фронтом; эквивалентен `source != "llm"`.
    pub fallback: bool,
    /// Источник текста: `"llm"` — живой ответ провайдера, `"local"` —
    /// локальный текстовый свод (фича `llm` выключена, ключ не найден, либо
    /// провайдер недоступен/ошибся — 10.4.3, грациозная деградация).
    pub source: String,
}

/// Описание правила Key Activity по умолчанию (для UI-настроек/справки).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyActivityRuleDto {
    pub id: String,
    pub name: String,
    pub weight: f64,
}

impl From<&domain::keyactivity::Rule> for KeyActivityRuleDto {
    fn from(r: &domain::keyactivity::Rule) -> Self {
        Self {
            id: r.id.clone(),
            name: r.name.clone(),
            weight: r.weight,
        }
    }
}

// ── T3 — Персист настроек и правил Key Activity в ядро ───────────────────────
// (10.5.3 / S.2.2 / 10.8.* / 11.6.1 / 12.8.1)
//
// Документ настроек, который хранит `crate::settings::SettingsStore` в
// JSON-файле ОС-config-директории — единый источник истины вместо
// localStorage. Зеркалит `frontend/src/lib/settings.ts::Settings` поле в
// поле (camelCase), КРОМЕ секретов: ключ LLM-провайдера/токен ALGOPACK сюда
// не попадают — только флаг `llmHasKey` («секрет задан»). Сами секреты живут
// в ОС-keyring/`.env` (`data::SecretStore`).

/// Активные рынки паспорта MOEX ALGOPACK.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketsDto {
    pub eq: bool,
    pub fo: bool,
    pub fx: bool,
}

impl Default for MarketsDto {
    fn default() -> Self {
        Self {
            eq: true,
            fo: true,
            fx: false,
        }
    }
}

/// Документ пользовательских настроек терминала (см. пояснение к разделу выше).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsDto {
    pub tape_limit: i64,
    pub dom_depth: i64,
    pub top_movers_limit: i64,

    // ── Паспорт MOEX ALGO ──────────────────────────────────────────────────
    pub markets: MarketsDto,
    pub watchlist: BTreeMap<String, bool>,

    // ── LLM · ИИ-резюме ────────────────────────────────────────────────────
    pub llm_provider: String,
    pub llm_model: String,
    /// Флаг «ключ провайдера задан» — сам ключ не хранится (S.2.2).
    pub llm_has_key: bool,
    pub llm_token_limit: i64,
    pub llm_auto: bool,
    pub default_period: String,

    // ── Данные / Историзация ───────────────────────────────────────────────
    pub data_source: String,
    pub data_dir: String,
    pub concurrency: i64,

    // ── Опционы ────────────────────────────────────────────────────────────
    pub pricing_model: String,
    pub rate: f64,
    pub default_smile: String,
}

impl Default for SettingsDto {
    fn default() -> Self {
        Self {
            tape_limit: 50,
            dom_depth: 10,
            top_movers_limit: 10,

            markets: MarketsDto::default(),
            watchlist: [
                ("SBER", true),
                ("GAZP", true),
                ("LKOH", true),
                ("GMKN", false),
                ("ROSN", true),
                ("VTBR", false),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),

            llm_provider: "openrouter".to_string(),
            llm_model: "anthropic/claude-3.5-sonnet".to_string(),
            llm_has_key: false,
            llm_token_limit: 2000,
            llm_auto: true,
            default_period: "1h".to_string(),

            data_source: "finam".to_string(),
            data_dir: "~/.market-terminal/history".to_string(),
            concurrency: 4,

            pricing_model: "black76".to_string(),
            rate: 0.0,
            default_smile: "moex".to_string(),
        }
    }
}

impl SettingsDto {
    /// Проверить значения перед атомарной записью (`settings_set`): отклоняет
    /// заведомо нерабочие настройки (неположительные лимиты, неизвестные коды
    /// перечислений, неконечную ставку) с понятной причиной по-русски.
    pub fn validate(&self) -> Result<(), String> {
        if self.tape_limit <= 0 {
            return Err("tapeLimit должен быть положительным".into());
        }
        if self.dom_depth <= 0 {
            return Err("domDepth должен быть положительным".into());
        }
        if self.top_movers_limit <= 0 {
            return Err("topMoversLimit должен быть положительным".into());
        }
        if self.llm_token_limit <= 0 {
            return Err("llmTokenLimit должен быть положительным".into());
        }
        if self.concurrency <= 0 {
            return Err("concurrency должен быть положительным".into());
        }
        if !self.rate.is_finite() {
            return Err("rate должен быть конечным числом".into());
        }
        const PROVIDERS: [&str; 3] = ["openrouter", "anthropic", "openai"];
        if !PROVIDERS.contains(&self.llm_provider.as_str()) {
            return Err(format!("неизвестный llmProvider: {}", self.llm_provider));
        }
        const PERIODS: [&str; 5] = ["1h", "1d", "1w", "1m", "3m"];
        if !PERIODS.contains(&self.default_period.as_str()) {
            return Err(format!(
                "неизвестный defaultPeriod: {}",
                self.default_period
            ));
        }
        const SOURCES: [&str; 2] = ["finam", "moex_algo"];
        if !SOURCES.contains(&self.data_source.as_str()) {
            return Err(format!("неизвестный dataSource: {}", self.data_source));
        }
        const PRICING: [&str; 2] = ["black76", "bachelier"];
        if !PRICING.contains(&self.pricing_model.as_str()) {
            return Err(format!("неизвестный pricingModel: {}", self.pricing_model));
        }
        const SMILES: [&str; 4] = ["moex", "sabr", "svi", "kalen"];
        if !SMILES.contains(&self.default_smile.as_str()) {
            return Err(format!("неизвестный defaultSmile: {}", self.default_smile));
        }
        Ok(())
    }
}

// ── Фаза 11 — Историзация: каталог локальных датасетов ───────────────────────

/// Метаданные локального датасета истории (строка «Локальные датасеты»).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetMetaDto {
    /// Код источника (`finam|moex_algo`).
    pub source: String,
    pub secid: String,
    /// Код тайм-фрейма (`m1|m5|m15|h1|d1`).
    pub tf: String,
    pub from_ts: i64,
    pub to_ts: i64,
    pub bars: u64,
    pub updated_ts: i64,
    /// Полнота покрытия (без крупных дыр).
    pub looks_complete: bool,
}

impl From<&DatasetMeta> for DatasetMetaDto {
    fn from(m: &DatasetMeta) -> Self {
        Self {
            source: m.source.code().to_string(),
            secid: m.secid.clone(),
            tf: m.tf.code().to_string(),
            from_ts: m.range.from,
            to_ts: m.range.till,
            bars: m.bars,
            updated_ts: m.updated_ts,
            looks_complete: m.looks_complete(),
        }
    }
}

/// Диапазон времени для фронта (план дозагрузки). Также вход (уже покрытые
/// диапазоны в `HistoryPlanInput`), поэтому и сериализуется, и десериализуется.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeRangeDto {
    pub from: i64,
    pub till: i64,
}

impl From<&TimeRange> for TimeRangeDto {
    fn from(r: &TimeRange) -> Self {
        Self {
            from: r.from,
            till: r.till,
        }
    }
}

/// Вход планирования дозагрузки истории: что уже покрыто и что запрошено.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPlanInput {
    /// Уже покрытые диапазоны (из каталога/стора).
    pub covered: Vec<TimeRangeDto>,
    pub requested_from: i64,
    pub requested_till: i64,
}

/// Идентификатор датасета для удаления/рефреша.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetIdInput {
    /// `finam|moex_algo`.
    pub source: String,
    pub secid: String,
    /// `m1|m5|m15|h1|d1`.
    pub tf: String,
}

// ── T11 — MOEX ALGO: датасеты ALGOPACK (Super Candles/FUTOI/HI2/Mega Alerts) ──
//
// DTO для чтения датасетов T8-хранилища (`storage::algo_*`) поверх аналитики
// `domain::algo`. Читаются напрямую из `Store` (без сети/фичи `moex` —
// в отличие от [`OptionQuoteDto`], датасеты уже персистентны), поэтому доступны
// в базовой сборке. Ингест этих таблиц (сетевой источник `AlgoSource`) — за
// фичей `moex` (см. `crate::ingest::algo`).

use domain::algo::mega_alerts::MegaAlert;
use domain::algo::{FutoiPoint, Hi2Point, SuperCandle};

/// Свеча Super Candles (датасет `tradestats`) для фронта: поля свечи + готовая
/// метрика [`SuperCandle::buy_pressure`], чтобы фронт не пересчитывал её сам.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradestatsDto {
    pub secid: String,
    pub ts: i64,
    pub pr_open: f64,
    pub pr_high: f64,
    pub pr_low: f64,
    pub pr_close: f64,
    pub pr_std: f64,
    pub vol: f64,
    pub val: f64,
    pub trades: f64,
    pub pr_vwap: f64,
    pub pr_change: f64,
    pub vol_b: f64,
    pub vol_s: f64,
    pub val_b: f64,
    pub val_s: f64,
    pub trades_b: f64,
    pub trades_s: f64,
    /// Дисбаланс потока (−1..1).
    pub disb: f64,
    pub pr_vwap_b: f64,
    pub pr_vwap_s: f64,
    /// Индекс агрессии покупателей (0..1) — [`SuperCandle::buy_pressure`].
    pub buy_pressure: f64,
}

impl From<&SuperCandle> for TradestatsDto {
    fn from(c: &SuperCandle) -> Self {
        Self {
            secid: c.secid.clone(),
            ts: c.ts,
            pr_open: c.pr_open,
            pr_high: c.pr_high,
            pr_low: c.pr_low,
            pr_close: c.pr_close,
            pr_std: c.pr_std,
            vol: c.vol,
            val: c.val,
            trades: c.trades,
            pr_vwap: c.pr_vwap,
            pr_change: c.pr_change,
            vol_b: c.vol_b,
            vol_s: c.vol_s,
            val_b: c.val_b,
            val_s: c.val_s,
            trades_b: c.trades_b,
            trades_s: c.trades_s,
            disb: c.disb,
            pr_vwap_b: c.pr_vwap_b,
            pr_vwap_s: c.pr_vwap_s,
            buy_pressure: c.buy_pressure(),
        }
    }
}

/// Точка FUTOI (открытый интерес физ/юр лиц) для фронта: поля точки + готовые
/// метрики [`FutoiPoint::net`]/[`FutoiPoint::long_share`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FutoiDto {
    pub secid: String,
    pub ts: i64,
    /// `fiz|yur`.
    pub clgroup: String,
    pub pos: f64,
    pub pos_long: f64,
    pub pos_short: f64,
    pub pos_long_num: f64,
    pub pos_short_num: f64,
    /// Нетто-позиция (long − short).
    pub net: f64,
    /// Доля длинных в суммарной позиции (0..1).
    pub long_share: f64,
}

impl From<&FutoiPoint> for FutoiDto {
    fn from(p: &FutoiPoint) -> Self {
        Self {
            secid: p.secid.clone(),
            ts: p.ts,
            clgroup: p.clgroup.code().to_string(),
            pos: p.pos,
            pos_long: p.pos_long,
            pos_short: p.pos_short,
            pos_long_num: p.pos_long_num,
            pos_short_num: p.pos_short_num,
            net: p.net(),
            long_share: p.long_share(),
        }
    }
}

/// Точка HI2 (индекс концентрации участников) для фронта: значение + готовая
/// классификация уровня + флаг всплеска (считается по окну вызывающей стороной
/// — см. `api::algo_hi2`, обёртка над [`domain::algo::hi2::concentration_spikes`]).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hi2Dto {
    pub ts: i64,
    pub secid: String,
    pub concentration: f64,
    /// `distributed|moderate|concentrated|dominated`.
    pub level: String,
    /// `true`, если в этой точке — всплеск концентрации (z-score ≥ порога).
    pub spike: bool,
}

impl From<&Hi2Point> for Hi2Dto {
    fn from(p: &Hi2Point) -> Self {
        Self {
            ts: p.ts,
            secid: p.secid.clone(),
            concentration: p.concentration,
            level: p.level().code().to_string(),
            spike: false,
        }
    }
}

/// Пороги детекторов Mega Alerts, приходящие с фронта (вход IPC). Отсутствующие
/// поля берутся из значений по умолчанию ([`domain::algo::mega_alerts::MegaThresholds`]).
#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MegaThresholdsInput {
    pub vol_z: Option<f64>,
    pub disb: Option<f64>,
    pub spread: Option<f64>,
    pub oi_jump: Option<f64>,
    pub hi2: Option<f64>,
}

impl MegaThresholdsInput {
    /// Перевести в доменные пороги, подставляя значения по умолчанию.
    pub fn to_thresholds(self) -> domain::algo::mega_alerts::MegaThresholds {
        let d = domain::algo::mega_alerts::MegaThresholds::default();
        domain::algo::mega_alerts::MegaThresholds {
            vol_z: self.vol_z.unwrap_or(d.vol_z),
            disb: self.disb.unwrap_or(d.disb),
            spread: self.spread.unwrap_or(d.spread),
            oi_jump: self.oi_jump.unwrap_or(d.oi_jump),
            hi2: self.hi2.unwrap_or(d.hi2),
        }
    }
}

/// Сработавший Mega-сигнал для фронта (лента алёртов).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MegaAlertDto {
    pub secid: String,
    pub ts: i64,
    /// `volume_spike|buy_imbalance|sell_imbalance|spread_widening|oi_jump|concentration_rise`.
    pub kind: String,
    pub value: f64,
    pub message: String,
}

impl From<&MegaAlert> for MegaAlertDto {
    fn from(a: &MegaAlert) -> Self {
        Self {
            secid: a.secid.clone(),
            ts: a.ts,
            kind: a.kind.code().to_string(),
            value: a.value,
            message: a.message.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::metrics::alerts::AlertEvent;
    use domain::{BookLevel, OrderBook, Trade};

    #[test]
    fn alert_rule_input_converts_known_kinds() {
        let r = AlertRuleInput {
            symbol: "SBER@MISX".into(),
            kind: "priceAbove".into(),
            threshold: 300.0,
        }
        .to_rule()
        .unwrap();
        assert_eq!(r.symbol, "SBER@MISX");
        assert_eq!(r.condition, AlertCondition::PriceAbove(300.0));

        assert!(AlertRuleInput {
            symbol: "X".into(),
            kind: "bogus".into(),
            threshold: 1.0,
        }
        .to_rule()
        .is_none());
    }

    #[test]
    fn trade_dto_maps_fields() {
        let t = Trade {
            ts: 10,
            price: 305.5,
            size: 12.0,
            buyer_initiated: Some(true),
        };
        let dto = TradeDto::from(&t);
        assert_eq!(dto.ts, 10);
        assert_eq!(dto.price, 305.5);
        assert_eq!(dto.size, 12.0);
        assert_eq!(dto.buyer_initiated, Some(true));
    }

    #[test]
    fn order_book_dto_preserves_sides() {
        let book = OrderBook {
            ts: 5,
            bids: vec![
                BookLevel {
                    price: 100.0,
                    size: 3.0,
                },
                BookLevel {
                    price: 99.5,
                    size: 7.0,
                },
            ],
            asks: vec![BookLevel {
                price: 100.5,
                size: 4.0,
            }],
        };
        let dto = OrderBookDto::from(&book);
        assert_eq!(dto.ts, 5);
        assert_eq!(dto.bids.len(), 2);
        assert_eq!(dto.bids[0].price, 100.0);
        assert_eq!(dto.asks[0].size, 4.0);
    }

    // ── T3 — Настройки ───────────────────────────────────────────────────────

    #[test]
    fn settings_dto_defaults_pass_validation() {
        assert!(SettingsDto::default().validate().is_ok());
    }

    #[test]
    fn settings_dto_rejects_unknown_enum_codes_and_bad_numbers() {
        let d = SettingsDto {
            llm_provider: "bogus".into(),
            ..SettingsDto::default()
        };
        assert!(d.validate().is_err());

        let d = SettingsDto {
            dom_depth: 0,
            ..SettingsDto::default()
        };
        assert!(d.validate().is_err());

        let d = SettingsDto {
            rate: f64::NAN,
            ..SettingsDto::default()
        };
        assert!(d.validate().is_err());
    }

    #[test]
    fn settings_dto_missing_fields_fill_from_defaults() {
        // Частичный JSON (как из старого/будущего формата файла) не должен падать —
        // отсутствующие поля берутся из Default (container-level `#[serde(default)]`).
        let partial: SettingsDto = serde_json::from_str(r#"{"tapeLimit":100}"#).unwrap();
        assert_eq!(partial.tape_limit, 100);
        assert_eq!(partial.dom_depth, SettingsDto::default().dom_depth);
        assert_eq!(partial.llm_provider, SettingsDto::default().llm_provider);
    }

    #[test]
    fn alert_event_dto_maps_message() {
        let e = AlertEvent {
            symbol: "SBER@MISX".into(),
            ts: 7,
            price: 310.0,
            change: 0.03,
            message: "цена выше 300".into(),
        };
        let dto = AlertEventDto::from(&e);
        assert_eq!(dto.symbol, "SBER@MISX");
        assert_eq!(dto.message, "цена выше 300");
        assert_eq!(dto.change, 0.03);
    }

    // ── T11 — MOEX ALGO: ALGOPACK-датасеты ──────────────────────────────────

    #[test]
    fn tradestats_dto_maps_fields_and_buy_pressure() {
        let c = SuperCandle {
            secid: "SBER".into(),
            ts: 10,
            pr_open: 100.0,
            pr_high: 101.0,
            pr_low: 99.0,
            pr_close: 100.5,
            pr_std: 0.4,
            vol: 100.0,
            val: 10_050.0,
            trades: 20.0,
            pr_vwap: 100.5,
            pr_change: 0.005,
            vol_b: 70.0,
            vol_s: 30.0,
            val_b: 7_000.0,
            val_s: 3_000.0,
            trades_b: 12.0,
            trades_s: 8.0,
            disb: 0.4,
            pr_vwap_b: 100.6,
            pr_vwap_s: 100.3,
        };
        let dto = TradestatsDto::from(&c);
        assert_eq!(dto.secid, "SBER");
        assert_eq!(dto.ts, 10);
        assert!((dto.buy_pressure - 0.7).abs() < 1e-12);
        assert_eq!(dto.disb, 0.4);
    }

    #[test]
    fn futoi_dto_maps_group_code_and_net() {
        let p = FutoiPoint {
            ts: 5,
            secid: "RIH5".into(),
            clgroup: domain::algo::ClientGroup::Fiz,
            pos: 1000.0,
            pos_long: 700.0,
            pos_short: 300.0,
            pos_long_num: 70.0,
            pos_short_num: 30.0,
        };
        let dto = FutoiDto::from(&p);
        assert_eq!(dto.clgroup, "fiz");
        assert_eq!(dto.net, 400.0);
        assert!((dto.long_share - 0.7).abs() < 1e-12);
    }

    #[test]
    fn hi2_dto_maps_level_and_defaults_spike_false() {
        let p = Hi2Point {
            ts: 1,
            secid: "SBER".into(),
            concentration: 0.6,
        };
        let dto = Hi2Dto::from(&p);
        assert_eq!(dto.level, "dominated");
        assert!(!dto.spike);
    }

    #[test]
    fn mega_thresholds_input_falls_back_to_defaults() {
        let t = MegaThresholdsInput {
            vol_z: Some(5.0),
            ..MegaThresholdsInput::default()
        }
        .to_thresholds();
        assert_eq!(t.vol_z, 5.0);
        assert_eq!(
            t.disb,
            domain::algo::mega_alerts::MegaThresholds::default().disb
        );
    }

    #[test]
    fn mega_alert_dto_maps_kind_code() {
        let a = MegaAlert {
            secid: "SBER".into(),
            ts: 3,
            kind: domain::algo::MegaAlertKind::BuyImbalance,
            value: 0.6,
            message: "перевес покупок".into(),
        };
        let dto = MegaAlertDto::from(&a);
        assert_eq!(dto.kind, "buy_imbalance");
        assert_eq!(dto.secid, "SBER");
    }
}
