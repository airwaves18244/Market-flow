//! DTO для фронтенда — сериализуемые ответы IPC-команд.
//!
//! Это «провод» между Rust-ядром и вебвью: типы намеренно плоские и
//! `camelCase` (привычно для TypeScript), чтобы фронт получал готовые к
//! отрисовке структуры (treemap/heatmap/свечи/временные ряды) без доустройки.

use serde::{Deserialize, Serialize};

use domain::backtest::{
    BacktestConfig, BacktestReport, FillTiming, PerfMetrics, SimTrade, StrategyDescriptor,
};
use domain::delta::{FootprintBar, RobotConfig, RobotSignal};
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
}
