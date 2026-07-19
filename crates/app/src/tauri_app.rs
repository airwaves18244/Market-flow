//! Привязка ядра IPC к Tauri (фича `tauri`).
//!
//! Тонкий слой: `#[tauri::command]`-обёртки лишь вызывают уже протестированные
//! методы [`AppState`] и переводят ошибки в строки для фронта. Здесь же —
//! заготовка событий live-push ([`emit_turnover_tick`]), которую в Фазе 7
//! дёргает фоновый планировщик ингеста.
//!
//! Модуль компилируется только в десктопном окружении (на Linux требуется
//! webkit2gtk), поэтому он вне кросс-платформенного CI.

use tauri::{Emitter, Manager, State};

use domain::backtest::StrategyParams;
use domain::TimeFrame;

use crate::dto::{
    AccountDto, AlertEventDto, AlertRuleInput, BacktestConfigInput, BacktestReportDto, BarPoint,
    BondIssuerDto, BreadthDto, CrossAssetSummaryDto, DatasetIdInput, DatasetMetaDto, FillEventDto,
    FlowEdgeDto, FootprintBarDto, FutoiDto, FutureGroupDto, Hi2Dto, HistoryPlanInput,
    ImpliedVolDto, ImpliedVolInput, InstrumentDto, KeyActivityRowDto, KeyActivityRuleDto,
    KeyActivitySampleInput, KeyActivitySummaryDto, MegaAlertDto, MegaThresholdsInput,
    OptionPriceDto, OptionPriceInput, OrderBookDto, OrderDto, OrderInput, PositionDto,
    RobotConfigInput, RobotSignalDto, RrgSectorDto, SectorEntryDto, SectorRow, SettingsDto,
    SmileFitDto, SmileFitInput, SmileModelDto, StrategyDescriptorDto, StrategyEvalDto,
    StrategyEvalInput, SubmitResultDto, TimeRangeDto, TopMoverDto, TradeDto, TradestatsDto,
    TurnoverByClassPoint, TurnoverPoint, YieldCurvePoint,
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

#[tauri::command]
fn list_strategies(state: State<AppState>) -> CmdResult<Vec<StrategyDescriptorDto>> {
    Ok(state.list_strategies())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn run_backtest(
    state: State<AppState>,
    symbol: String,
    timeframe: String,
    from_ts: i64,
    to_ts: i64,
    strategy_id: String,
    params: StrategyParams,
    config: BacktestConfigInput,
) -> CmdResult<BacktestReportDto> {
    let tf = TimeFrame::from_code(&timeframe)
        .ok_or_else(|| format!("неизвестный тайм-фрейм: {timeframe}"))?;
    state
        .run_backtest(&symbol, tf, from_ts, to_ts, &strategy_id, &params, &config)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delta_footprint(
    state: State<AppState>,
    symbol: String,
    timeframe: String,
    from_ts: i64,
    to_ts: i64,
    tick_size: f64,
) -> CmdResult<Vec<FootprintBarDto>> {
    let tf = TimeFrame::from_code(&timeframe)
        .ok_or_else(|| format!("неизвестный тайм-фрейм: {timeframe}"))?;
    state
        .delta_footprint(&symbol, tf, from_ts, to_ts, tick_size)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn robot_scan(
    state: State<AppState>,
    symbol: String,
    from_ts: i64,
    to_ts: i64,
    config: RobotConfigInput,
) -> CmdResult<Vec<RobotSignalDto>> {
    state
        .robot_scan(&symbol, from_ts, to_ts, &config)
        .map_err(|e| e.to_string())
}

// ── Фаза 12 — Опционы ────────────────────────────────────────────────────────

#[tauri::command]
fn list_smile_models(state: State<AppState>) -> CmdResult<Vec<SmileModelDto>> {
    Ok(state.list_smile_models())
}

#[tauri::command]
fn option_price(state: State<AppState>, input: OptionPriceInput) -> CmdResult<OptionPriceDto> {
    state.option_price(&input)
}

#[tauri::command]
fn option_implied_vol(state: State<AppState>, input: ImpliedVolInput) -> CmdResult<ImpliedVolDto> {
    state.option_implied_vol(&input)
}

#[tauri::command]
fn smile_fit(state: State<AppState>, input: SmileFitInput) -> CmdResult<SmileFitDto> {
    state.smile_fit(&input)
}

#[tauri::command]
fn strategy_eval(state: State<AppState>, input: StrategyEvalInput) -> CmdResult<StrategyEvalDto> {
    state.strategy_eval(&input)
}

/// Опционная доска MOEX через публичный ISS (фаза 12.4). Существует только
/// со включённой фичей `moex` (как `key_activity_summary` в live-варианте с
/// `llm`, но без локального фолбэка: доска — сетевые данные по определению,
/// без фичи команда отсутствует и фронт работает на мок-доске). Команда
/// асинхронная, чтобы сетевой вызов не блокировал IPC-поток.
#[cfg(feature = "moex")]
#[tauri::command]
async fn option_board(
    state: State<'_, AppState>,
    input: crate::dto::OptionBoardInput,
) -> CmdResult<crate::dto::OptionBoardDto> {
    state.option_board(&input).await
}

// ── Фаза 10 — MOEX ALGO: Key Activity ────────────────────────────────────────

#[tauri::command]
fn key_activity(
    state: State<AppState>,
    samples: Vec<KeyActivitySampleInput>,
    period: Option<String>,
) -> CmdResult<Vec<KeyActivityRowDto>> {
    Ok(state.key_activity(&samples, period.as_deref()))
}

/// Свод «ИТОГО» по ключевой активности. Со включённой фичей `llm` — живой
/// ИИ-провайдер из настроек с грациозной деградацией в локальный свод
/// (см. [`AppState::key_activity_summary_live`]); команда асинхронная, чтобы
/// сетевой вызов не блокировал IPC-поток.
#[cfg(feature = "llm")]
#[tauri::command]
async fn key_activity_summary(
    state: State<'_, AppState>,
    samples: Vec<KeyActivitySampleInput>,
    period: Option<String>,
) -> CmdResult<KeyActivitySummaryDto> {
    Ok(state
        .key_activity_summary_live(&samples, period.as_deref())
        .await)
}

/// Свод «ИТОГО» по ключевой активности: без фичи `llm` — всегда локальный
/// текстовый свод (как и раньше).
#[cfg(not(feature = "llm"))]
#[tauri::command]
fn key_activity_summary(
    state: State<AppState>,
    samples: Vec<KeyActivitySampleInput>,
    period: Option<String>,
) -> CmdResult<KeyActivitySummaryDto> {
    Ok(state.key_activity_summary(&samples, period.as_deref()))
}

#[tauri::command]
fn key_activity_rules(state: State<AppState>) -> CmdResult<Vec<KeyActivityRuleDto>> {
    Ok(state.key_activity_rules())
}

// ── T11 — MOEX ALGO: датасеты ALGOPACK (Super Candles/FUTOI/HI2/Mega Alerts) ─

#[tauri::command]
fn algo_tradestats(
    state: State<AppState>,
    market: String,
    secid: String,
    from_ts: i64,
    to_ts: i64,
) -> CmdResult<Vec<TradestatsDto>> {
    state
        .algo_tradestats(&market, &secid, from_ts, to_ts)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn algo_futoi(
    state: State<AppState>,
    market: String,
    secid: String,
    from_ts: i64,
    to_ts: i64,
) -> CmdResult<Vec<FutoiDto>> {
    state
        .algo_futoi(&market, &secid, from_ts, to_ts)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn algo_hi2(
    state: State<AppState>,
    market: String,
    secid: String,
    from_ts: i64,
    to_ts: i64,
) -> CmdResult<Vec<Hi2Dto>> {
    state
        .algo_hi2(&market, &secid, from_ts, to_ts)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn algo_hi2_ranking(
    state: State<AppState>,
    market: String,
    secids: Vec<String>,
    limit: usize,
) -> CmdResult<Vec<Hi2Dto>> {
    state
        .algo_hi2_ranking(&market, &secids, limit)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn algo_mega_alerts(
    state: State<AppState>,
    market: String,
    secids: Vec<String>,
    from_ts: i64,
    to_ts: i64,
    thresholds: Option<MegaThresholdsInput>,
) -> CmdResult<Vec<MegaAlertDto>> {
    state
        .algo_mega_alerts(&market, &secids, from_ts, to_ts, thresholds)
        .map_err(|e| e.to_string())
}

// ── T3 — Настройки и правила Key Activity (10.5.3/S.2.2/10.8.*/11.6.1/12.8.1) ─

#[tauri::command]
fn settings_get(state: State<AppState>) -> CmdResult<SettingsDto> {
    Ok(state.settings_get())
}

#[tauri::command]
fn settings_set(state: State<AppState>, doc: SettingsDto) -> CmdResult<()> {
    state.settings_set(doc)
}

#[tauri::command]
fn key_activity_rules_get(state: State<AppState>) -> CmdResult<Vec<KeyActivityRuleDto>> {
    Ok(state.key_activity_rules_get())
}

#[tauri::command]
fn key_activity_rules_set(
    state: State<AppState>,
    rules_json: String,
) -> CmdResult<Vec<KeyActivityRuleDto>> {
    state.key_activity_rules_set(&rules_json)
}

// ── Фаза 11 — Историзация: каталог датасетов ─────────────────────────────────

#[tauri::command]
fn history_datasets(state: State<AppState>) -> CmdResult<Vec<DatasetMetaDto>> {
    Ok(state.history_datasets())
}

#[tauri::command]
fn history_delete(state: State<AppState>, id: DatasetIdInput) -> CmdResult<bool> {
    state.history_delete(&id)
}

#[tauri::command]
fn history_plan(state: State<AppState>, input: HistoryPlanInput) -> CmdResult<Vec<TimeRangeDto>> {
    Ok(state.history_plan(&input))
}

/// Превью загруженного датасета (11.4.4): последние `limit` баров ключа
/// (source, secid, tf) для верификации свечами (`CandleChart`).
#[tauri::command]
fn history_preview(
    state: State<AppState>,
    source: String,
    secid: String,
    tf: String,
    limit: Option<usize>,
) -> CmdResult<Vec<BarPoint>> {
    state.history_preview(&source, &secid, &tf, limit.unwrap_or(500))
}

/// Перевести доменное событие загрузчика в live-push событие фронта
/// (`history:progress|done|error`), по образцу [`emit_trade`].
fn emit_history_event(app: &tauri::AppHandle, ev: crate::history::HistoryEvent) {
    use crate::dto::{HistoryDoneDto, HistoryErrorDto, HistoryProgressDto};
    use crate::history::HistoryEvent;

    let result = match ev {
        HistoryEvent::Progress {
            task_id,
            ticker,
            tf,
            percent,
        } => app.emit(
            "history:progress",
            HistoryProgressDto {
                task_id,
                ticker,
                tf: tf.code().to_owned(),
                percent,
            },
        ),
        HistoryEvent::Done {
            task_id,
            ticker,
            tf,
            bars,
            summary,
        } => app.emit(
            "history:done",
            HistoryDoneDto {
                task_id,
                ticker,
                tf: tf.map(|t| t.code().to_owned()),
                bars,
                summary,
            },
        ),
        HistoryEvent::Error {
            task_id,
            ticker,
            tf,
            message,
        } => app.emit(
            "history:error",
            HistoryErrorDto {
                task_id,
                ticker,
                tf: tf.map(|t| t.code().to_owned()),
                message,
            },
        ),
    };
    if let Err(e) = result {
        tracing::warn!(error = %e, "не удалось отправить событие history:*");
    }
}

/// Запустить фоновую загрузку истории (IPC `history_load`).
///
/// Регистрирует задачу в реестре, немедленно возвращает `taskId` и запускает
/// загрузку в фоне (`tauri::async_runtime::spawn`): прогресс, завершение и
/// ошибки идут только событиями `history:*`, команда не блокируется на всё
/// время скачивания. Боевой источник строится по коду (`finam` — Finam Trade
/// API, требует фичи `live`; `moex_algo` — ALGOPACK, требует фичи `moex`).
/// Для `moex_algo` рынок берётся из `input.market` (`eq|fo|fx`, дефолт `eq`).
/// Отмена — `history_cancel(taskId?)`; каждая пара `(тикер, TF)` качает только
/// недостающие диапазоны, ошибка одной задачи не роняет остальные.
#[tauri::command]
async fn history_load(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: crate::dto::HistoryLoadInput,
) -> CmdResult<crate::dto::HistoryTaskDto> {
    use domain::history::DataSource;

    // Разбор входа выполняем синхронно — ошибки ввода возвращаем сразу, до
    // регистрации задачи и спавна.
    let request = crate::history::parse_load_input(&input)?;
    #[cfg(feature = "moex")]
    let market = match input.market.as_deref() {
        None | Some("") => data::moex::Market::Eq,
        Some(code) => data::moex::Market::from_code(code)
            .ok_or_else(|| format!("неизвестный рынок ALGOPACK: {code}"))?,
    };

    let (task_id, cancel) = state.history_tasks().start();

    // Фоновый запуск: AppHandle клонируется в спавн, состояние достаём из него
    // же (`app.state`) — `State<'_>` из аргументов в 'static-спавн не переносим.
    let app_bg = app.clone();
    tauri::async_runtime::spawn(async move {
        let emit = |ev| emit_history_event(&app_bg, ev);
        let state = app_bg.state::<AppState>();
        let state: &AppState = state.inner();

        let outcome: Result<(), String> = match request.source {
            DataSource::MoexAlgo => {
                #[cfg(feature = "moex")]
                {
                    // R-8: токен ALGOPACK берём единым резолвером (env → .env, с
                    // trim), а не голым std::env::var — иначе `.env`-конфиг и
                    // случайные пробелы игнорировались бы.
                    match crate::algo_ingest::load_algo_token() {
                        Ok(token) => match data::ReqwestTransport::new() {
                            Ok(transport) => {
                                let client = data::MoexAlgo::new(transport, token);
                                let source = data::MoexHistory::new(client, market);
                                crate::history::run_load(
                                    state, &source, &request, task_id, &cancel, &emit,
                                )
                                .await;
                                Ok(())
                            }
                            Err(e) => Err(e.to_string()),
                        },
                        Err(e) => Err(e),
                    }
                }
                #[cfg(not(feature = "moex"))]
                {
                    Err(
                        "источник MOEX ALGO недоступен в этой сборке (нужна фича `moex`)"
                            .to_owned(),
                    )
                }
            }
            DataSource::Finam => {
                #[cfg(feature = "live")]
                {
                    match crate::live::load_secret() {
                        Ok(secret) => {
                            let auth = data::AuthManager::new(
                                data::GrpcAuthTransport::new(),
                                data::MemSecretStore::with_secret(secret),
                            );
                            match data::FinamMarketData::connect(auth) {
                                Ok(md) => {
                                    let source = data::FinamHistory::new(md);
                                    crate::history::run_load(
                                        state, &source, &request, task_id, &cancel, &emit,
                                    )
                                    .await;
                                    Ok(())
                                }
                                Err(e) => Err(e.to_string()),
                            }
                        }
                        Err(e) => Err(e),
                    }
                }
                #[cfg(not(feature = "live"))]
                {
                    Err("источник Finam недоступен в этой сборке (нужна фича `live`)".to_owned())
                }
            }
        };

        // Ошибку старта/источника доносим до фронта событием (running на фронте
        // живёт до `history:done`/`history:error`, а не до промиса команды).
        if let Err(message) = outcome {
            emit(crate::history::HistoryEvent::Error {
                task_id,
                ticker: None,
                tf: None,
                message,
            });
        }
        state.history_tasks().finish(task_id);
    });

    Ok(crate::dto::HistoryTaskDto { task_id })
}

/// Отменить фоновую загрузку истории (IPC `history_cancel`): конкретную по
/// `taskId` или все активные, если `taskId` не задан. Возвращает число
/// затронутых задач.
#[tauri::command]
fn history_cancel(state: State<AppState>, task_id: Option<u64>) -> CmdResult<usize> {
    Ok(state.history_tasks().cancel(task_id))
}

// ── V2 / Trade (симулятор исполнения) ───────────────────────────────────────

#[tauri::command]
fn submit_order(state: State<AppState>, order: OrderInput) -> CmdResult<SubmitResultDto> {
    state.submit_order(&order)
}

#[tauri::command]
fn cancel_order(state: State<AppState>, id: u64) -> CmdResult<OrderDto> {
    state.cancel_order(id)
}

#[tauri::command]
fn order_blotter(state: State<AppState>) -> CmdResult<Vec<OrderDto>> {
    Ok(state.order_blotter())
}

#[tauri::command]
fn positions(state: State<AppState>) -> CmdResult<Vec<PositionDto>> {
    Ok(state.positions())
}

#[tauri::command]
fn account(state: State<AppState>) -> CmdResult<AccountDto> {
    Ok(state.account())
}

/// Отправить во фронт исполнение симулятора (канал `fill:tick`). Живые стримы
/// сделок/стакана (см. [`spawn_live_streams`]) прокидывают тик в
/// `state.ingest_live_trades`/`ingest_live_book`, а полученные исполнения — сюда.
/// Без фичи `live` стримов нет, поэтому там глушим dead_code.
#[cfg_attr(not(feature = "live"), allow(dead_code))]
pub fn emit_fill(app: &tauri::AppHandle, fill: &FillEventDto) -> CmdResult<()> {
    app.emit("fill:tick", fill).map_err(|e| e.to_string())
}

/// Лента сделок (Time&Sales) — первичный снимок (GAP-5): последние сделки из
/// кольцевого буфера [`AppState`], наполняемого live-стримом `subscribe_trades`.
/// Свежие — первыми, усечение по `limit`. Дальше фронт слушает событие
/// `trade:tick` (см. [`emit_trade`]). До первого live-тика ответ пуст.
#[tauri::command]
fn latest_trades(
    state: State<AppState>,
    symbol: String,
    limit: Option<usize>,
) -> CmdResult<Vec<TradeDto>> {
    Ok(state.latest_trades_snapshot(&symbol, limit))
}

/// Снимок стакана (DOM) — первичный снимок (GAP-5): последний стакан из
/// [`AppState`], наполняемого live-стримом `subscribe_order_book`, усечённый до
/// `depth` уровней. Дальше фронт слушает событие `orderbook:tick` (см.
/// [`emit_order_book`]). До первого live-обновления ответ пуст.
#[tauri::command]
fn order_book(
    state: State<AppState>,
    symbol: String,
    depth: Option<usize>,
) -> CmdResult<OrderBookDto> {
    Ok(state.order_book_snapshot(&symbol, depth))
}

/// Отправить во фронт событие live-обновления оборота (канал `turnover:tick`).
/// Точка интеграции для потокового ингеста (Фаза 7).
#[allow(dead_code)]
pub fn emit_turnover_tick(app: &tauri::AppHandle, point: &TurnoverPoint) -> CmdResult<()> {
    app.emit("turnover:tick", point).map_err(|e| e.to_string())
}

/// Отправить во фронт сделку для ленты Time&Sales (канал `trade:tick`).
/// Дёргается live-стримом `subscribe_trades` (см. [`spawn_live_streams`]).
/// Без фичи `live` стримов нет — глушим dead_code.
#[cfg_attr(not(feature = "live"), allow(dead_code))]
pub fn emit_trade(app: &tauri::AppHandle, trade: &TradeDto) -> CmdResult<()> {
    app.emit("trade:tick", trade).map_err(|e| e.to_string())
}

/// Отправить во фронт снимок стакана для DOM (канал `orderbook:tick`).
/// Дёргается live-стримом `subscribe_order_book` (см. [`spawn_live_streams`]).
/// Без фичи `live` стримов нет — глушим dead_code.
#[cfg_attr(not(feature = "live"), allow(dead_code))]
pub fn emit_order_book(app: &tauri::AppHandle, book: &OrderBookDto) -> CmdResult<()> {
    app.emit("orderbook:tick", book).map_err(|e| e.to_string())
}

/// Построить состояние с продакшен-бэкендом (DuckDB при фиче `duckdb`,
/// иначе — in-memory).
fn build_state() -> AppState {
    use storage::Store;

    #[cfg(feature = "duckdb")]
    {
        // БД кладём в стандартную конфиг-директорию ОС (та же, где settings.json):
        // рабочий каталог установленного приложения (Program Files) недоступен
        // на запись без прав администратора, а относительный путь зависел бы от
        // того, откуда запущен exe.
        let dir = crate::settings::default_config_dir();
        std::fs::create_dir_all(&dir).expect("не удалось создать директорию данных");
        let db_path = dir.join("market.duckdb");
        let mut store = storage::DuckStore::open(&db_path)
            .unwrap_or_else(|e| panic!("не удалось открыть БД DuckDB {}: {e}", db_path.display()));
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

/// Спавн боевого Finam-ингеста в состояние, которым владеет окно (GAP-1/GAP-2).
///
/// Наполняет ТОТ ЖЕ `Store`, что читают команды (`instruments`/`bars` — вкладка
/// «Обзор»): состояние достаётся из `AppHandle` уже внутри фоновой задачи — тот
/// же паттерн, что у [`history_load`] (`State<'_>` из setup в 'static-спавн не
/// переносим). Секрет и MIC читаются заранее; если секрета нет — НЕ падаем:
/// пишем `warn` и оставляем терминал работать на пустом хранилище (грациозная
/// деградация). MIC — из переменной окружения `FINAM_MIC` (дефолт `MISX`).
#[cfg(all(feature = "tauri", feature = "live"))]
fn spawn_live_ingest(app: tauri::AppHandle) {
    // Fail-fast: без секрета крутить задачу незачем — сразу деградируем.
    if let Err(e) = crate::live::load_secret() {
        tracing::warn!(
            error = %e,
            "live-ингест не запущен: API-секрет не задан — терминал работает на пустом хранилище"
        );
        return;
    }
    let mic = finam_mic();

    tracing::info!(mic = %mic, "live-ингест: запуск боевой задачи в общий стор");
    tauri::async_runtime::spawn(async move {
        // Состояние берём из AppHandle (владелец — Tauri), а не создаём своё —
        // так ингест пишет в стор окна. `&AppState` живёт всё время задачи,
        // потому что `app` перемещён в задачу; синхронный `Mutex` стора через
        // `.await` не держим (`IngestService::tick` берёт лок точечно).
        let cancel = crate::cancel::CancelFlag::new();
        let state = app.state::<AppState>();
        let state: &AppState = state.inner();
        if let Err(e) = crate::live::run_ingest_into(state, &mic, cancel).await {
            tracing::warn!(error = %e, "live-ингест завершился ошибкой");
        }
    });
}

/// Биржа (MIC) для боевого справочника/стримов: переменная окружения
/// `FINAM_MIC`, дефолт `MISX` (основной рынок MOEX). Общий помощник для ингеста
/// и стримов, чтобы обе задачи смотрели на одну биржу.
#[cfg(all(feature = "tauri", feature = "live"))]
fn finam_mic() -> String {
    std::env::var("FINAM_MIC")
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "MISX".to_owned())
}

/// Конкретный тип боевого источника рыночных данных для стрим-задач (канал
/// ленивый, авторизация по секрету из окружения/keyring).
#[cfg(all(feature = "tauri", feature = "live"))]
type LiveMarketData = data::FinamMarketData<data::GrpcAuthTransport, data::MemSecretStore>;

/// Максимум инструментов, на стакан/ленту которых подписываемся одновременно.
///
/// Каждый символ порождает ДВЕ долгоживущие подписки (`subscribe_trades` +
/// `subscribe_order_book`). Явного лимита на число открытых стрим-подписок в
/// Finam-клиенте нет ([`data::RateLimiter`] лимитирует РАЗОВЫЕ запросы —
/// ~200/мин на метод, стримы под него не попадают), но плодить десятки стримов
/// на весь справочник неразумно: стримим ограниченный поднабор вотчлиста.
/// Значение с запасом под DOM/ленту нескольких активных инструментов «Обзора».
#[cfg(all(feature = "tauri", feature = "live"))]
const MAX_STREAM_SYMBOLS: usize = 8;

/// Вотчлист для стримов: включённые тикеры из настроек, приведённые к символам
/// биржи `mic` (`{TICKER}@{MIC}`), ограниченные [`MAX_STREAM_SYMBOLS`].
///
/// Отдельного «выбранного инструмента» в настройках нет — есть карта вотчлиста
/// (тикер → включён), которую ведёт вкладка настроек. Берём её включённое
/// подмножество: это и есть интересующие пользователя инструменты. Пустой
/// вотчлист → пусто (стримы не поднимаем).
#[cfg(all(feature = "tauri", feature = "live"))]
fn stream_watchlist(state: &AppState, mic: &str) -> Vec<String> {
    let settings = state.settings_get();
    settings
        .watchlist
        .into_iter()
        .filter(|(_, on)| *on)
        .map(|(ticker, _)| format!("{ticker}@{mic}"))
        .take(MAX_STREAM_SYMBOLS)
        .collect()
}

/// Доля джиттера `[0, 1)` для пауз переподключения — общая реализация из
/// `data::backoff` (системные часы + атомарный счётчик): два стрима,
/// переподключающиеся в одну наносекунду, всё равно получат разные паузы.
#[cfg(all(feature = "tauri", feature = "live"))]
fn jitter_fraction() -> f64 {
    data::backoff::jitter_fraction()
}

/// Стрим-задача ленты сделок одного инструмента (GAP-3).
///
/// Бесконечный цикл с переподключением ([`data::StreamReconnect`]): стрим Finam
/// обрывается ~раз в 24 ч, поэтому подписку переоткрываем с экспоненциальной
/// паузой (сбрасывается после успешных данных). Ошибки не паникуют — логируются
/// и ведут к переподключению. Задача умирает вместе с процессом (Tauri убьёт
/// runtime при закрытии окна). На каждую пачку сделок: буфер+симулятор в
/// [`AppState::ingest_live_trades`], затем события `trade:tick`/`fill:tick`.
#[cfg(all(feature = "tauri", feature = "live"))]
async fn run_trade_stream(md: std::sync::Arc<LiveMarketData>, app: tauri::AppHandle, symbol: String) {
    let mut reconnect = data::StreamReconnect::default();
    loop {
        match md.subscribe_trades(&symbol).await {
            Ok(mut stream) => {
                reconnect.reset();
                loop {
                    match stream.next().await {
                        Ok(Some(trades)) => {
                            if trades.is_empty() {
                                continue;
                            }
                            // Лок стора не держим через .await: снимок/симулятор
                            // обновляются синхронно, потом эмитим уже без лока.
                            let (dtos, fills) = {
                                let state = app.state::<AppState>();
                                state.inner().ingest_live_trades(&symbol, &trades)
                            };
                            for t in &dtos {
                                let _ = emit_trade(&app, t);
                            }
                            for f in &fills {
                                let _ = emit_fill(&app, f);
                            }
                        }
                        Ok(None) => break, // стрим закрыт — переподключаемся
                        Err(e) => {
                            tracing::warn!(symbol = %symbol, error = %e, "стрим сделок: ошибка чтения");
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(symbol = %symbol, error = %e, "стрим сделок: подписка не удалась");
            }
        }
        tokio::time::sleep(reconnect.next_delay(jitter_fraction())).await;
    }
}

/// Стрим-задача стакана (DOM) одного инструмента (GAP-3). Устройство — как у
/// [`run_trade_stream`]. Одно сообщение стрима может нести несколько снимков;
/// берём последний как актуальный стакан, обновляем снимок/симулятор и эмитим
/// `orderbook:tick`/`fill:tick`.
#[cfg(all(feature = "tauri", feature = "live"))]
async fn run_order_book_stream(
    md: std::sync::Arc<LiveMarketData>,
    app: tauri::AppHandle,
    symbol: String,
) {
    let mut reconnect = data::StreamReconnect::default();
    loop {
        match md.subscribe_order_book(&symbol).await {
            Ok(mut stream) => {
                reconnect.reset();
                loop {
                    match stream.next().await {
                        Ok(Some(books)) => {
                            let Some(book) = books.into_iter().next_back() else {
                                continue;
                            };
                            let (dto, fills) = {
                                let state = app.state::<AppState>();
                                state.inner().ingest_live_book(&symbol, book)
                            };
                            let _ = emit_order_book(&app, &dto);
                            for f in &fills {
                                let _ = emit_fill(&app, f);
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            tracing::warn!(symbol = %symbol, error = %e, "стрим стакана: ошибка чтения");
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(symbol = %symbol, error = %e, "стрим стакана: подписка не удалась");
            }
        }
        tokio::time::sleep(reconnect.next_delay(jitter_fraction())).await;
    }
}

/// Поднять live-стримы сделок и стакана поверх состояния окна (GAP-3).
///
/// Отдельная от [`spawn_live_ingest`] задача: ингест тянет БАРЫ (медленный
/// поллинг в стор для вкладки «Обзор»/бэктестов), а здесь — тиковые сделки и
/// стакан в снимки [`AppState`] и push-события фронта. Строит собственное
/// подключение к Finam (канал ленивый — дёшево), берёт вотчлист из настроек и
/// на каждый символ спавнит по стрим-задаче. Нет секрета/подключения/вотчлиста
/// — не падаем: пишем `warn` и живём без live-тиков (грациозная деградация).
#[cfg(all(feature = "tauri", feature = "live"))]
fn spawn_live_streams(app: tauri::AppHandle, mic: String) {
    tauri::async_runtime::spawn(async move {
        let secret = match crate::live::load_secret() {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "live-стримы не запущены: нет API-секрета");
                return;
            }
        };
        let auth = data::AuthManager::new(
            data::GrpcAuthTransport::new(),
            data::MemSecretStore::with_secret(secret),
        );
        let md = match data::FinamMarketData::connect(auth) {
            Ok(m) => std::sync::Arc::new(m),
            Err(e) => {
                tracing::warn!(error = %e, "live-стримы: не удалось подключиться к Finam");
                return;
            }
        };
        let symbols = {
            let state = app.state::<AppState>();
            stream_watchlist(state.inner(), &mic)
        };
        if symbols.is_empty() {
            tracing::warn!("live-стримы: вотчлист пуст — подписки не подняты");
            return;
        }
        tracing::info!(count = symbols.len(), "live-стримы: запуск подписок сделок/стакана");
        for symbol in symbols {
            tauri::async_runtime::spawn(run_trade_stream(
                std::sync::Arc::clone(&md),
                app.clone(),
                symbol.clone(),
            ));
            tauri::async_runtime::spawn(run_order_book_stream(
                std::sync::Arc::clone(&md),
                app.clone(),
                symbol,
            ));
        }
    });
}

/// Вотчлист для ALGOPACK-ингеста: включённые тикеры из настроек как «голые»
/// `secid` (ALGOPACK адресует инструменты по тикеру без суффикса биржи, в
/// отличие от Finam `{TICKER}@{MIC}`). Пустой вотчлист допустим — планировщик
/// всё равно тянет сводный по рынку `hi2`.
#[cfg(all(feature = "tauri", feature = "moex"))]
fn algo_watchlist(state: &AppState) -> Vec<String> {
    state
        .settings_get()
        .watchlist
        .into_iter()
        .filter(|(_, on)| *on)
        .map(|(ticker, _)| ticker)
        .collect()
}

/// Спавн боевого ALGOPACK-ингеста в состояние, которым владеет окно (GAP-6).
///
/// По образцу [`spawn_live_ingest`]: состояние достаётся из `AppHandle` уже
/// внутри задачи (тот же стор, что читают команды `algo_*`), обмен строится
/// общим ядром [`crate::algo_ingest::run_algo_ingest_into`]. Токен проверяем
/// заранее для fail-fast; без него НЕ падаем — пишем `warn` и живём без
/// ALGOPACK-ингеста (грациозная деградация, как у live-ингеста). Независим от
/// фичи `live` (свой токен/хост `apim.moex.com`), поэтому под фичей `moex`.
#[cfg(all(feature = "tauri", feature = "moex"))]
fn spawn_algo_ingest(app: tauri::AppHandle) {
    // Fail-fast: без токена крутить задачу незачем — сразу деградируем.
    if let Err(e) = crate::algo_ingest::load_algo_token() {
        tracing::warn!(
            error = %e,
            "ALGOPACK-ингест не запущен: токен не задан — терминал работает без ALGOPACK"
        );
        return;
    }
    tracing::info!("ALGOPACK-ингест: запуск боевой задачи в общий стор");
    tauri::async_runtime::spawn(async move {
        let cancel = crate::cancel::CancelFlag::new();
        let state = app.state::<AppState>();
        let state: &AppState = state.inner();
        let symbols = algo_watchlist(state);
        let config = crate::algo_ingest::AlgoIngestConfig::default();
        if let Err(e) =
            crate::algo_ingest::run_algo_ingest_into(state, symbols, config, cancel).await
        {
            tracing::warn!(error = %e, "ALGOPACK-ингест завершился ошибкой");
        }
    });
}

/// Запустить десктопное приложение Tauri.
pub fn run() {
    tauri::Builder::default()
        .manage(build_state())
        .setup(|app| {
            // GAP-1/GAP-2: после `manage(state)` поднимаем боевой ингест в тот же
            // AppState. Только под фичей `live`; без неё сборка терминала цела.
            #[cfg(feature = "live")]
            {
                spawn_live_ingest(app.handle().clone());
                // GAP-3: тиковые сделки/стакан в снимки состояния и push-события
                // фронта (`trade:tick`/`orderbook:tick`/`fill:tick`).
                spawn_live_streams(app.handle().clone(), finam_mic());
            }
            // GAP-6: боевой ALGOPACK-ингест в тот же AppState. Независим от
            // `live` (свой токен/хост), поэтому отдельный блок под фичей `moex`.
            #[cfg(feature = "moex")]
            {
                spawn_algo_ingest(app.handle().clone());
            }
            #[cfg(not(any(feature = "live", feature = "moex")))]
            {
                let _ = app;
            }
            Ok(())
        })
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
            alerts_scan,
            list_strategies,
            run_backtest,
            delta_footprint,
            robot_scan,
            list_smile_models,
            option_price,
            option_implied_vol,
            smile_fit,
            strategy_eval,
            #[cfg(feature = "moex")]
            option_board,
            key_activity,
            key_activity_summary,
            key_activity_rules,
            algo_tradestats,
            algo_futoi,
            algo_hi2,
            algo_hi2_ranking,
            algo_mega_alerts,
            settings_get,
            settings_set,
            key_activity_rules_get,
            key_activity_rules_set,
            history_datasets,
            history_delete,
            history_plan,
            history_preview,
            history_load,
            history_cancel,
            submit_order,
            cancel_order,
            order_blotter,
            positions,
            account,
            latest_trades,
            order_book
        ])
        .run(tauri::generate_context!())
        .expect("ошибка запуска приложения Tauri");
}
