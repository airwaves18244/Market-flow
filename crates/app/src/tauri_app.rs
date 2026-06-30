//! Привязка ядра IPC к Tauri (фича `tauri`).
//!
//! Тонкий слой: `#[tauri::command]`-обёртки лишь вызывают уже протестированные
//! методы [`AppState`] и переводят ошибки в строки для фронта. Здесь же —
//! заготовка событий live-push ([`emit_turnover_tick`]), которую в Фазе 7
//! дёргает фоновый планировщик ингеста.
//!
//! Модуль компилируется только в десктопном окружении (на Linux требуется
//! webkit2gtk), поэтому он вне кросс-платформенного CI.

use tauri::{Emitter, State};

use domain::TimeFrame;

use crate::dto::{
    AlertEventDto, AlertRuleInput, BarPoint, BondIssuerDto, BreadthDto, CrossAssetSummaryDto,
    FlowEdgeDto, FutureGroupDto, InstrumentDto, OrderBookDto, RegimeSignalDto, RrgSectorDto,
    SectorEntryDto, SectorRow, TopMoverDto, TradeDto, TurnoverByClassPoint, TurnoverPoint,
    YieldCurvePoint,
};
use crate::state::AppState;

type CmdResult<T> = Result<T, String>;

#[tauri::command]
fn instruments(state: State<AppState>) -> CmdResult<Vec<InstrumentDto>> {
    state.instruments().map_err(|e| e.to_string())
}

#[tauri::command]
fn bars(
    state: State<AppState>,
    symbol: String,
    timeframe: String,
    from_ts: i64,
    to_ts: i64,
) -> CmdResult<Vec<BarPoint>> {
    let tf = TimeFrame::from_code(&timeframe)
        .ok_or_else(|| format!("неизвестный тайм-фрейм: {timeframe}"))?;
    state
        .bars(&symbol, tf, from_ts, to_ts)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn turnover_series(
    state: State<AppState>,
    symbol: String,
    from_ts: i64,
    to_ts: i64,
) -> CmdResult<Vec<TurnoverPoint>> {
    state
        .turnover_series(&symbol, from_ts, to_ts)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn sector_rollup(state: State<AppState>, from_ts: i64, to_ts: i64) -> CmdResult<Vec<SectorRow>> {
    state
        .sector_rollup(from_ts, to_ts)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn sector_map(state: State<AppState>) -> CmdResult<Vec<SectorEntryDto>> {
    state.sector_map().map_err(|e| e.to_string())
}

#[tauri::command]
fn breadth_data(state: State<AppState>, from_ts: i64, to_ts: i64) -> CmdResult<BreadthDto> {
    state
        .breadth_data(from_ts, to_ts)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn top_movers(
    state: State<AppState>,
    from_ts: i64,
    to_ts: i64,
    limit: Option<usize>,
) -> CmdResult<Vec<TopMoverDto>> {
    state
        .top_movers(from_ts, to_ts, limit)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn rrg_sectors(state: State<AppState>, from_ts: i64, to_ts: i64) -> CmdResult<Vec<RrgSectorDto>> {
    state.rrg_sectors(from_ts, to_ts).map_err(|e| e.to_string())
}

#[tauri::command]
fn futures_rollup(
    state: State<AppState>,
    from_ts: i64,
    to_ts: i64,
) -> CmdResult<Vec<FutureGroupDto>> {
    state
        .futures_rollup(from_ts, to_ts)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn bonds_rollup(state: State<AppState>, from_ts: i64, to_ts: i64) -> CmdResult<Vec<BondIssuerDto>> {
    state
        .bonds_rollup(from_ts, to_ts)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn yield_curve(state: State<AppState>) -> CmdResult<Vec<YieldCurvePoint>> {
    state.yield_curve().map_err(|e| e.to_string())
}

#[tauri::command]
fn cross_asset_summary(
    state: State<AppState>,
    from_ts: i64,
    to_ts: i64,
) -> CmdResult<CrossAssetSummaryDto> {
    state
        .cross_asset_summary(from_ts, to_ts)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn turnover_timeline(
    state: State<AppState>,
    from_ts: i64,
    to_ts: i64,
) -> CmdResult<Vec<TurnoverByClassPoint>> {
    state
        .turnover_timeline(from_ts, to_ts)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn flow_sankey(state: State<AppState>, from_ts: i64, to_ts: i64) -> CmdResult<Vec<FlowEdgeDto>> {
    state.flow_sankey(from_ts, to_ts).map_err(|e| e.to_string())
}

#[tauri::command]
fn summary(state: State<AppState>, from_ts: i64, to_ts: i64) -> CmdResult<RegimeSignalDto> {
    state.summary(from_ts, to_ts).map_err(|e| e.to_string())
}

#[tauri::command]
fn alerts_scan(
    state: State<AppState>,
    rules: Vec<AlertRuleInput>,
    from_ts: i64,
    to_ts: i64,
) -> CmdResult<Vec<AlertEventDto>> {
    state
        .alerts_scan(&rules, from_ts, to_ts)
        .map_err(|e| e.to_string())
}

/// Лента сделок (Time&Sales). В store-backed сборке тиковые сделки не
/// сохраняются, поэтому первичный ответ пуст — живые сделки приходят событием
/// `trade:tick` (см. [`emit_trade`]) из live-стрима `subscribe_trades`.
/// Команда сохраняет единый IPC-контракт для фронта.
#[tauri::command]
fn latest_trades(_symbol: String, _limit: Option<usize>) -> CmdResult<Vec<TradeDto>> {
    Ok(Vec::new())
}

/// Снимок стакана (DOM). Аналогично [`latest_trades`]: первичный ответ пуст,
/// живые обновления приходят событием `orderbook:tick` (см. [`emit_order_book`]).
#[tauri::command]
fn order_book(_symbol: String, _depth: Option<usize>) -> CmdResult<OrderBookDto> {
    Ok(OrderBookDto {
        ts: 0,
        bids: Vec::new(),
        asks: Vec::new(),
    })
}

/// Отправить во фронт событие live-обновления оборота (канал `turnover:tick`).
/// Точка интеграции для потокового ингеста (Фаза 7).
#[allow(dead_code)]
pub fn emit_turnover_tick(app: &tauri::AppHandle, point: &TurnoverPoint) -> CmdResult<()> {
    app.emit("turnover:tick", point).map_err(|e| e.to_string())
}

/// Отправить во фронт сделку для ленты Time&Sales (канал `trade:tick`).
/// Точка интеграции для live-стрима `subscribe_trades` (Фаза 7).
#[allow(dead_code)]
pub fn emit_trade(app: &tauri::AppHandle, trade: &TradeDto) -> CmdResult<()> {
    app.emit("trade:tick", trade).map_err(|e| e.to_string())
}

/// Отправить во фронт снимок стакана для DOM (канал `orderbook:tick`).
/// Точка интеграции для live-стрима `subscribe_order_book` (Фаза 7).
#[allow(dead_code)]
pub fn emit_order_book(app: &tauri::AppHandle, book: &OrderBookDto) -> CmdResult<()> {
    app.emit("orderbook:tick", book).map_err(|e| e.to_string())
}

/// Построить состояние с продакшен-бэкендом (DuckDB при фиче `duckdb`,
/// иначе — in-memory).
fn build_state() -> AppState {
    use storage::Store;

    #[cfg(feature = "duckdb")]
    {
        let mut store =
            storage::DuckStore::open("market.duckdb").expect("не удалось открыть БД DuckDB");
        store.migrate().expect("миграция DuckDB не удалась");
        AppState::new(store)
    }
    #[cfg(not(feature = "duckdb"))]
    {
        let mut store = storage::MemStore::new();
        store.migrate().expect("миграция MemStore не удалась");
        AppState::new(store)
    }
}

/// Запустить десктопное приложение Tauri.
pub fn run() {
    tauri::Builder::default()
        .manage(build_state())
        .invoke_handler(tauri::generate_handler![
            instruments,
            bars,
            turnover_series,
            sector_rollup,
            sector_map,
            breadth_data,
            top_movers,
            rrg_sectors,
            futures_rollup,
            bonds_rollup,
            yield_curve,
            cross_asset_summary,
            turnover_timeline,
            flow_sankey,
            summary,
            alerts_scan,
            latest_trades,
            order_book
        ])
        .run(tauri::generate_context!())
        .expect("ошибка запуска приложения Tauri");
}
