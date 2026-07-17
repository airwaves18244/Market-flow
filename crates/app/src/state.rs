//! Состояние приложения, разделяемое между IPC-командами.
//!
//! Оборачивает [`storage::Store`] за `Mutex`, чтобы команды Tauri (и фоновый
//! планировщик ингеста) безопасно обращались к хранилищу из разных потоков.
//! Бэкенд абстрактный: в тестах — `MemStore`, в продакшене — `DuckStore`.

use std::path::PathBuf;
use std::sync::Mutex;

#[cfg(feature = "moex")]
use domain::algo::{FutoiPoint, Hi2Point, SuperCandle};
#[cfg(feature = "ingest")]
use domain::history::HistoryBar;
use domain::history::{Catalog, DataSource, DatasetMeta};
#[cfg(feature = "ingest")]
use domain::Bar;
use domain::TimeFrame;
#[cfg(feature = "ingest")]
use storage::ingest::snapshot_from_bars;
use storage::{StorageError, Store};

use domain::backtest::StrategyParams;

use crate::api;
use crate::dto::{
    AccountDto, AlertEventDto, AlertRuleInput, BacktestConfigInput, BacktestReportDto, BarPoint,
    BondIssuerDto, BreadthDto, CrossAssetSummaryDto, DatasetIdInput, DatasetMetaDto, FlowEdgeDto,
    FootprintBarDto, FutoiDto, FutureGroupDto, Hi2Dto, HistoryPlanInput, ImpliedVolDto,
    ImpliedVolInput, InstrumentDto, KeyActivityRowDto, KeyActivityRuleDto, KeyActivitySampleInput,
    KeyActivitySummaryDto, MegaAlertDto, MegaThresholdsInput, OptionPriceDto, OptionPriceInput,
    OrderDto, OrderInput, PositionDto, RobotConfigInput, RobotSignalDto, RrgSectorDto,
    SectorEntryDto, SectorRow, SettingsDto, SmileFitDto, SmileFitInput, SmileModelDto,
    StrategyDescriptorDto, StrategyEvalDto, StrategyEvalInput, SubmitResultDto, TimeRangeDto,
    TopMoverDto, TradestatsDto, TurnoverByClassPoint, TurnoverPoint, YieldCurvePoint,
};
use crate::settings::SettingsStore;
use crate::trade::TradeSession;

/// Разделяемое состояние терминала.
pub struct AppState {
    store: Mutex<Box<dyn Store + Send>>,
    /// Сессия симулированной торговли (paper trading).
    trade: TradeSession,
    /// Каталог локальных датасетов истории (фаза 11). Пока в памяти; боевой
    /// загрузчик/DuckDB-хранилище наполняют его по мере загрузки.
    history: Mutex<Catalog>,
    /// Персист пользовательских настроек и правил Key Activity в JSON-файл
    /// ОС-config-директории (T3/10.5.3/S.2.2). `Mutex` — что несколько
    /// IPC-команд не гонялись за одним временным файлом при атомарной записи.
    settings: Mutex<SettingsStore>,
    /// Сессионный кэш готовых ИИ-резюме Key Activity (фаза 10.4, фича `llm`):
    /// повторный вызов с тем же входом/моделью/провайдером не дёргает
    /// провайдера повторно. Не персистится — живёт, пока живёт процесс.
    /// В headless-сборке (фича `llm` без `tauri`) поле не читается вне тестов.
    #[cfg(feature = "llm")]
    #[allow(dead_code)]
    llm_cache: crate::llm::SummaryCache,
    /// Реестр фоновых загрузок истории (T10, фаза 11.3): раздаёт `task_id` и
    /// хранит флаги отмены, чтобы `history_cancel(taskId?)` мог остановить одну
    /// загрузку или все. Живёт под фичей `ingest` (async-оркестрация).
    #[cfg(feature = "ingest")]
    history_tasks: crate::history::HistoryTasks,
}

// В headless-live режиме (а также при сборке одной фичи `llm`/`moex` без
// `tauri`) IPC-read-методы (обработчики команд) не вызываются — их потребляет
// Tauri-UI и тесты. Глушим dead_code только для этих комбинаций.
#[cfg_attr(
    any(feature = "live", feature = "llm", feature = "moex"),
    allow(dead_code)
)]
impl AppState {
    /// Создать состояние поверх произвольного бэкенда хранилища. Настройки
    /// резолвятся в стандартную ОС-директорию (см. [`SettingsStore::from_env`]);
    /// для тестов/портейбл-режима используйте [`AppState::with_settings_dir`].
    pub fn new(store: impl Store + Send + 'static) -> Self {
        // Гидратируем каталог из хранилища: после перезапуска поверх наполненного
        // стора `history_datasets()` сразу видит реальные датасеты (фаза 11).
        let history = store.catalog().unwrap_or_default();
        Self {
            store: Mutex::new(Box::new(store)),
            trade: TradeSession::new(),
            history: Mutex::new(history),
            settings: Mutex::new(SettingsStore::from_env()),
            #[cfg(feature = "llm")]
            llm_cache: crate::llm::SummaryCache::new(),
            #[cfg(feature = "ingest")]
            history_tasks: crate::history::HistoryTasks::default(),
        }
    }

    /// Создать состояние с явно заданной директорией конфигурации (тесты,
    /// портейбл-режим — чтобы не читать/писать в реальный config-каталог ОС).
    /// В обычной сборке (без `tauri`/`live`, вне тестов) не вызывается —
    /// консольный smoke использует [`AppState::new`], поэтому глушим dead_code.
    #[allow(dead_code)]
    pub fn with_settings_dir(store: impl Store + Send + 'static, settings_dir: PathBuf) -> Self {
        let history = store.catalog().unwrap_or_default();
        Self {
            store: Mutex::new(Box::new(store)),
            trade: TradeSession::new(),
            history: Mutex::new(history),
            settings: Mutex::new(SettingsStore::new(settings_dir)),
            #[cfg(feature = "llm")]
            llm_cache: crate::llm::SummaryCache::new(),
            #[cfg(feature = "ingest")]
            history_tasks: crate::history::HistoryTasks::default(),
        }
    }

    /// Доступ к сессии торговли (для live-эмиттеров `on_trade`/`on_book`).
    pub fn trade_session(&self) -> &TradeSession {
        &self.trade
    }

    /// Выполнить чтение под блокировкой. Отравленный мьютекс → ошибка БД.
    fn read<F, R>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&dyn Store) -> Result<R, StorageError>,
    {
        let guard = self
            .store
            .lock()
            .map_err(|_| StorageError::Db("state lock poisoned".into()))?;
        f(guard.as_ref())
    }

    /// Выполнить запись под блокировкой. Отравленный мьютекс → ошибка БД.
    #[cfg(any(feature = "ingest", feature = "moex"))]
    fn write<F, R>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&mut dyn Store) -> Result<R, StorageError>,
    {
        let mut guard = self
            .store
            .lock()
            .map_err(|_| StorageError::Db("state lock poisoned".into()))?;
        f(guard.as_mut())
    }

    /// Записать бары инструмента и построить снимок оборота на `snapshot_ts`.
    ///
    /// Точка входа планировщика ингеста ([`crate::ingest`]). Идемпотентно по
    /// ключам схемы (повторный ингест не плодит дублей). Пустая серия — no-op;
    /// снимок пишется только для непустой серии.
    #[cfg(feature = "ingest")]
    pub fn ingest_bars(
        &self,
        symbol: &str,
        tf: TimeFrame,
        bars: &[Bar],
        snapshot_ts: i64,
    ) -> Result<(), StorageError> {
        if bars.is_empty() {
            return Ok(());
        }
        self.write(|s| {
            s.insert_bars(symbol, tf, bars)?;
            if let Some(snap) = snapshot_from_bars(bars, snapshot_ts) {
                s.insert_snapshot(symbol, &snap)?;
            }
            Ok(())
        })
    }

    pub fn instruments(&self) -> Result<Vec<InstrumentDto>, StorageError> {
        self.read(api::instruments)
    }

    pub fn bars(
        &self,
        symbol: &str,
        tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<BarPoint>, StorageError> {
        self.read(|s| api::bars(s, symbol, tf, from_ts, to_ts))
    }

    pub fn turnover_series(
        &self,
        symbol: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<TurnoverPoint>, StorageError> {
        self.read(|s| api::turnover_series(s, symbol, from_ts, to_ts))
    }

    pub fn sector_rollup(&self, from_ts: i64, to_ts: i64) -> Result<Vec<SectorRow>, StorageError> {
        self.read(|s| api::sector_rollup(s, from_ts, to_ts))
    }

    pub fn sector_map(&self) -> Result<Vec<SectorEntryDto>, StorageError> {
        self.read(api::sector_map)
    }

    pub fn breadth_data(&self, from_ts: i64, to_ts: i64) -> Result<BreadthDto, StorageError> {
        self.read(|s| api::breadth_data(s, from_ts, to_ts))
    }

    pub fn top_movers(
        &self,
        from_ts: i64,
        to_ts: i64,
        limit: Option<usize>,
    ) -> Result<Vec<TopMoverDto>, StorageError> {
        self.read(|s| api::top_movers(s, from_ts, to_ts, limit))
    }

    pub fn rrg_sectors(&self, from_ts: i64, to_ts: i64) -> Result<Vec<RrgSectorDto>, StorageError> {
        self.read(|s| api::rrg_sectors(s, from_ts, to_ts))
    }

    pub fn futures_rollup(
        &self,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<FutureGroupDto>, StorageError> {
        self.read(|s| api::futures_rollup(s, from_ts, to_ts))
    }

    pub fn bonds_rollup(
        &self,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<BondIssuerDto>, StorageError> {
        self.read(|s| api::bonds_rollup(s, from_ts, to_ts))
    }

    /// Кривая доходности не зависит от хранилища (статический scaffold),
    /// поэтому блокировка стора не нужна.
    pub fn yield_curve(&self) -> Result<Vec<YieldCurvePoint>, StorageError> {
        api::yield_curve()
    }

    pub fn cross_asset_summary(
        &self,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<CrossAssetSummaryDto, StorageError> {
        self.read(|s| api::cross_asset_summary(s, from_ts, to_ts))
    }

    pub fn turnover_timeline(
        &self,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<TurnoverByClassPoint>, StorageError> {
        self.read(|s| api::turnover_timeline(s, from_ts, to_ts))
    }

    pub fn flow_sankey(&self, from_ts: i64, to_ts: i64) -> Result<Vec<FlowEdgeDto>, StorageError> {
        self.read(|s| api::flow_sankey(s, from_ts, to_ts))
    }

    /// Прогон правил алёртов по сохранённым барам (replay-проверка правил).
    pub fn alerts_scan(
        &self,
        rules: &[AlertRuleInput],
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<AlertEventDto>, StorageError> {
        self.read(|s| api::alerts_scan(s, rules, from_ts, to_ts))
    }

    // ── V2 / Бэктестер ──────────────────────────────────────────────────────

    /// Каталог встроенных стратегий бэктестера (не зависит от хранилища).
    pub fn list_strategies(&self) -> Vec<StrategyDescriptorDto> {
        api::list_strategies()
    }

    /// Прогон бэктеста стратегии по сохранённым барам инструмента.
    #[allow(clippy::too_many_arguments)]
    pub fn run_backtest(
        &self,
        symbol: &str,
        tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
        strategy_id: &str,
        params: &StrategyParams,
        config: &BacktestConfigInput,
    ) -> Result<BacktestReportDto, StorageError> {
        self.read(|s| api::run_backtest(s, symbol, tf, from_ts, to_ts, strategy_id, params, config))
    }

    // ── V2 / Delta ──────────────────────────────────────────────────────────

    /// Footprint/дельта инструмента по сохранённым тиковым сделкам.
    pub fn delta_footprint(
        &self,
        symbol: &str,
        tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
        tick_size: f64,
    ) -> Result<Vec<FootprintBarDto>, StorageError> {
        self.read(|s| api::delta_footprint(s, symbol, tf, from_ts, to_ts, tick_size))
    }

    /// Прогон детектирующих роботов по сохранённой ленте инструмента.
    pub fn robot_scan(
        &self,
        symbol: &str,
        from_ts: i64,
        to_ts: i64,
        config: &RobotConfigInput,
    ) -> Result<Vec<RobotSignalDto>, StorageError> {
        self.read(|s| api::robot_scan(s, symbol, from_ts, to_ts, config))
    }

    // ── V2 / Trade (симулятор исполнения) ────────────────────────────────────

    /// Поставить заявку в симулятор. `Err` — причина отклонения/ошибки ввода.
    pub fn submit_order(&self, input: &OrderInput) -> Result<SubmitResultDto, String> {
        self.trade.submit(input)
    }

    /// Отменить активную заявку.
    pub fn cancel_order(&self, id: u64) -> Result<OrderDto, String> {
        self.trade.cancel(id)
    }

    /// Активные заявки (блоттер).
    pub fn order_blotter(&self) -> Vec<OrderDto> {
        self.trade.orders()
    }

    /// Открытые позиции.
    pub fn positions(&self) -> Vec<PositionDto> {
        self.trade.positions()
    }

    /// Состояние счёта.
    pub fn account(&self) -> AccountDto {
        self.trade.account()
    }

    // ── Фаза 12 — Опционы (чистые расчёты, без хранилища) ─────────────────────

    /// Каталог моделей улыбки для селектора в UI.
    pub fn list_smile_models(&self) -> Vec<SmileModelDto> {
        api::list_smile_models()
    }

    /// Теоретическая цена + греки опциона.
    pub fn option_price(&self, input: &OptionPriceInput) -> Result<OptionPriceDto, String> {
        api::option_price(input)
    }

    /// Подразумеваемая волатильность из рыночной цены.
    pub fn option_implied_vol(&self, input: &ImpliedVolInput) -> Result<ImpliedVolDto, String> {
        api::option_implied_vol(input)
    }

    /// Калибровка улыбки по рыночным точкам.
    pub fn smile_fit(&self, input: &SmileFitInput) -> Result<SmileFitDto, String> {
        api::smile_fit(input)
    }

    /// Оценка опционной стратегии (payoff, греки, безубыток).
    pub fn strategy_eval(&self, input: &StrategyEvalInput) -> Result<StrategyEvalDto, String> {
        api::strategy_eval(input)
    }

    /// Опционная доска MOEX через публичный ISS (фаза 12.4, фича `moex`):
    /// котировки + форвард + готовые точки улыбки для калибратора. Сетевой
    /// вызов — метод асинхронный (как [`AppState::key_activity_summary_live`]),
    /// чтобы не блокировать IPC-поток. Логика с выбором серии/форварда — в
    /// [`api::option_board`], протестированном на фейковом источнике;
    /// здесь только live-обёртка над публичным ISS.
    #[cfg(feature = "moex")]
    pub async fn option_board(
        &self,
        input: &crate::dto::OptionBoardInput,
    ) -> Result<crate::dto::OptionBoardDto, String> {
        api::option_board_live(input).await
    }

    // ── Фаза 10 — MOEX ALGO: Key Activity ─────────────────────────────────────

    /// Ключевая активность за период по образцам метрик (встроенные правила).
    pub fn key_activity(
        &self,
        samples: &[KeyActivitySampleInput],
        period: Option<&str>,
    ) -> Vec<KeyActivityRowDto> {
        api::key_activity(samples, period)
    }

    /// Локальный (без LLM) свод «ИТОГО» по ключевой активности.
    pub fn key_activity_summary(
        &self,
        samples: &[KeyActivitySampleInput],
        period: Option<&str>,
    ) -> KeyActivitySummaryDto {
        api::key_activity_summary(samples, period)
    }

    /// Живой ИИ-свод «ИТОГО» по ключевой активности (фаза 10.4, фича `llm`):
    /// провайдер/модель/лимит токенов — из персистентных настроек; при
    /// отсутствии ключа/ошибке провайдера — грациозная деградация в тот же
    /// локальный свод, что и [`AppState::key_activity_summary`]. Сессионный
    /// кэш (см. [`crate::llm::SummaryCache`]) живёт на `self`, поэтому
    /// повторный вызов с тем же входом не дёргает провайдера.
    #[cfg(feature = "llm")]
    pub async fn key_activity_summary_live(
        &self,
        samples: &[KeyActivitySampleInput],
        period: Option<&str>,
    ) -> KeyActivitySummaryDto {
        let settings = self.settings_get();
        api::key_activity_summary_live(&self.llm_cache, &settings, samples, period).await
    }

    /// Встроенные правила Key Activity (для настроек/справки).
    pub fn key_activity_rules(&self) -> Vec<KeyActivityRuleDto> {
        api::key_activity_rules()
    }

    // ── T11 — MOEX ALGO: датасеты ALGOPACK (чтение из storage T8) ──────────────

    /// Размер скользящего окна z-score/спайков по умолчанию (в барах/точках) —
    /// то же значение, что и в тестах `api::algo_*`.
    const ALGO_WINDOW: usize = 20;
    /// Порог z-score всплеска концентрации HI2 по умолчанию.
    const ALGO_HI2_THRESHOLD: f64 = 3.0;

    /// Свечи Super Candles (`tradestats`) инструмента `secid` на рынке `market`.
    pub fn algo_tradestats(
        &self,
        market: &str,
        secid: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<TradestatsDto>, StorageError> {
        self.read(|s| api::algo_tradestats(s, market, secid, from_ts, to_ts))
    }

    /// Точки FUTOI инструмента `secid` на рынке `market` (обычно `fo`).
    pub fn algo_futoi(
        &self,
        market: &str,
        secid: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<FutoiDto>, StorageError> {
        self.read(|s| api::algo_futoi(s, market, secid, from_ts, to_ts))
    }

    /// Точки HI2 инструмента `secid` с проставленным флагом всплеска (окно/порог
    /// по умолчанию, см. [`AppState::ALGO_WINDOW`]/[`AppState::ALGO_HI2_THRESHOLD`]).
    pub fn algo_hi2(
        &self,
        market: &str,
        secid: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<Hi2Dto>, StorageError> {
        self.read(|s| {
            api::algo_hi2(
                s,
                market,
                secid,
                from_ts,
                to_ts,
                Self::ALGO_WINDOW,
                Self::ALGO_HI2_THRESHOLD,
            )
        })
    }

    /// Ранжирование `secids` по последней концентрации HI2 (топ-`limit`,
    /// по убыванию) — эффективный путь для сводных панелей (см.
    /// [`api::algo_hi2_ranking`]): без полного чтения истории на тикер.
    pub fn algo_hi2_ranking(
        &self,
        market: &str,
        secids: &[String],
        limit: usize,
    ) -> Result<Vec<Hi2Dto>, StorageError> {
        self.read(|s| api::algo_hi2_ranking(s, market, secids, limit))
    }

    /// Mega Alerts (10.2.8) по сохранённым датасетам ALGOPACK для `secids` на
    /// рынке `market` в окне `[from_ts, to_ts]`. `thresholds` — `None` для
    /// порогов по умолчанию.
    pub fn algo_mega_alerts(
        &self,
        market: &str,
        secids: &[String],
        from_ts: i64,
        to_ts: i64,
        thresholds: Option<MegaThresholdsInput>,
    ) -> Result<Vec<MegaAlertDto>, StorageError> {
        self.read(|s| {
            api::algo_mega_alerts(
                s,
                market,
                secids,
                from_ts,
                to_ts,
                thresholds.map(MegaThresholdsInput::to_thresholds),
                Self::ALGO_WINDOW,
            )
        })
    }

    /// Записать свечи Super Candles (`tradestats`) для рынка `market`. Точка
    /// входа планировщика ALGOPACK-ингеста ([`crate::ingest::algo`]).
    #[cfg(feature = "moex")]
    pub fn ingest_algo_tradestats(
        &self,
        market: &str,
        candles: &[SuperCandle],
    ) -> Result<usize, StorageError> {
        self.write(|s| s.insert_algo_tradestats(market, candles))
    }

    /// Записать точки FUTOI для рынка `market` (`fo`).
    #[cfg(feature = "moex")]
    pub fn ingest_algo_futoi(
        &self,
        market: &str,
        points: &[FutoiPoint],
    ) -> Result<usize, StorageError> {
        self.write(|s| s.insert_algo_futoi(market, points))
    }

    /// Записать точки HI2 для рынка `market`.
    #[cfg(feature = "moex")]
    pub fn ingest_algo_hi2(
        &self,
        market: &str,
        points: &[Hi2Point],
    ) -> Result<usize, StorageError> {
        self.write(|s| s.insert_algo_hi2(market, points))
    }

    // ── Фаза 11 — Историзация: каталог локальных датасетов ────────────────────

    /// Список локальных датасетов истории (метаданные).
    pub fn history_datasets(&self) -> Vec<DatasetMetaDto> {
        let guard = match self.history.lock() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        guard.datasets.iter().map(DatasetMetaDto::from).collect()
    }

    /// Зарегистрировать/обновить датасет в каталоге (точка для загрузчика).
    pub fn history_register(&self, meta: DatasetMeta) {
        if let Ok(mut guard) = self.history.lock() {
            guard.upsert(meta);
        }
    }

    /// Удалить датасет: из in-memory-каталога, из каталога хранилища и сами бары
    /// (`history_bars`), чтобы после удаления не оставалось сирот. `true` — если
    /// запись каталога существовала.
    pub fn history_delete(&self, input: &DatasetIdInput) -> Result<bool, String> {
        let source = DataSource::from_code(&input.source)
            .ok_or_else(|| format!("неизвестный источник: {}", input.source))?;
        let tf = TimeFrame::from_code(&input.tf)
            .ok_or_else(|| format!("неизвестный тайм-фрейм: {}", input.tf))?;
        // Хранилище: снять датасет с каталога и стереть его бары. Локом стора
        // владеем напрямую (метод компилируется во всех сборках, а хелпер
        // `write` — только под `ingest`/`moex`).
        {
            let mut guard = self
                .store
                .lock()
                .map_err(|_| "state lock poisoned".to_string())?;
            let store = guard.as_mut();
            store
                .remove_dataset(source, &input.secid, tf)
                .map_err(|e| e.to_string())?;
            store
                .delete_history_bars(source, &input.secid, tf)
                .map_err(|e| e.to_string())?;
        }
        let mut guard = self
            .history
            .lock()
            .map_err(|_| "history lock poisoned".to_string())?;
        Ok(guard.remove(source, &input.secid, tf))
    }

    /// План дозагрузки истории (недостающие диапазоны). Чистая обёртка.
    pub fn history_plan(&self, input: &HistoryPlanInput) -> Vec<TimeRangeDto> {
        api::history_plan(input)
    }

    /// Превью загруженного датасета (11.4.4): последние `limit` баров ключа
    /// (source, secid, tf) из локального хранилища истории для отрисовки
    /// свечами. Неизвестный код источника/тайм-фрейма — понятная ошибка.
    pub fn history_preview(
        &self,
        source: &str,
        secid: &str,
        tf: &str,
        limit: usize,
    ) -> Result<Vec<BarPoint>, String> {
        let src = DataSource::from_code(source)
            .ok_or_else(|| format!("неизвестный источник: {source}"))?;
        let timeframe =
            TimeFrame::from_code(tf).ok_or_else(|| format!("неизвестный тайм-фрейм: {tf}"))?;
        self.read(|s| api::history_preview(s, src, secid, timeframe, limit))
            .map_err(|e| e.to_string())
    }

    // ── T10 — Историзация: загрузчик (async-оркестрация, фича `ingest`) ────────

    /// Реестр фоновых загрузок (для запуска/отмены задач).
    #[cfg(feature = "ingest")]
    #[allow(dead_code)]
    pub fn history_tasks(&self) -> &crate::history::HistoryTasks {
        &self.history_tasks
    }

    /// Недостающие диапазоны ключа (source, secid, tf) в окне — план дыр
    /// поверх каталога хранилища (`history_missing_ranges`).
    #[cfg(feature = "ingest")]
    #[allow(dead_code)]
    pub fn history_missing(
        &self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
        range: domain::history::TimeRange,
    ) -> Result<Vec<domain::history::TimeRange>, StorageError> {
        self.read(|s| s.history_missing_ranges(source, secid, tf, range))
    }

    /// Записать загруженные исторические бары (`history_bars`). Идемпотентно по
    /// ключу — повторная запись не плодит дублей. Возвращает число строк.
    #[cfg(feature = "ingest")]
    #[allow(dead_code)]
    pub fn ingest_history_bars(&self, bars: &[HistoryBar]) -> Result<usize, StorageError> {
        self.write(|s| s.insert_history_bars(bars))
    }

    /// Зафиксировать датасет в каталоге после записи баров: пересчитать
    /// метаданные (диапазон/число баров) по фактическому содержимому стора и
    /// обновить каталог хранилища (`upsert_dataset`) и in-memory-каталог,
    /// который читает `history_datasets`. `covered` — обработанный на этом шаге
    /// диапазон; он сливается с уже покрытым. Возвращает актуальные метаданные.
    #[cfg(feature = "ingest")]
    #[allow(dead_code)]
    pub fn history_commit(
        &self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
        covered: domain::history::TimeRange,
    ) -> Result<DatasetMeta, StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let meta = self.write(|s| {
            // Расширить диапазон каталога до огибающей с уже покрытым — несмежные
            // догрузки не теряются, внутренние дыры остаются на совести баров.
            let merged = match s.dataset(source, secid, tf)? {
                Some(existing) => existing.range.envelope(&covered),
                None => covered,
            };
            // Пересчитать число баров по факту (в объединённом диапазоне).
            let count = s.count_history_bars(source, secid, tf, merged.from, merged.till)?;
            let meta = DatasetMeta {
                source,
                secid: secid.to_owned(),
                tf,
                range: merged,
                bars: count,
                updated_ts: now,
            };
            s.upsert_dataset(&meta)?;
            Ok(meta)
        })?;

        // Зеркалим в in-memory-каталог, который читает `history_datasets`.
        self.history_register(meta.clone());
        Ok(meta)
    }

    // ── T3 — Персист настроек и правил Key Activity ────────────────────────────
    // (10.5.3 / S.2.2 / 10.8.* / 11.6.1 / 12.8.1)

    /// Текущие пользовательские настройки (дефолты, если ещё не сохранялись).
    /// Отравленный мьютекс трактуется как «файла ещё нет» — дефолты, а не паника.
    pub fn settings_get(&self) -> SettingsDto {
        match self.settings.lock() {
            Ok(guard) => api::settings_get(&guard),
            Err(_) => SettingsDto::default(),
        }
    }

    /// Сохранить настройки: валидация + атомарная запись.
    pub fn settings_set(&self, doc: SettingsDto) -> Result<(), String> {
        let guard = self
            .settings
            .lock()
            .map_err(|_| "settings lock poisoned".to_string())?;
        api::settings_set(&guard, doc)
    }

    /// Пользовательские правила Key Activity, сохранённые ранее (пусто — ещё
    /// не сохранялись).
    pub fn key_activity_rules_get(&self) -> Vec<KeyActivityRuleDto> {
        match self.settings.lock() {
            Ok(guard) => api::key_activity_rules_get(&guard),
            Err(_) => Vec::new(),
        }
    }

    /// Сохранить пользовательские правила Key Activity (`rules_json` — JSON
    /// доменной модели `domain::keyactivity::Rule`; валидация — сама
    /// десериализация, см. [`api::key_activity_rules_set`]).
    pub fn key_activity_rules_set(
        &self,
        rules_json: &str,
    ) -> Result<Vec<KeyActivityRuleDto>, String> {
        let guard = self
            .settings
            .lock()
            .map_err(|_| "settings lock poisoned".to_string())?;
        api::key_activity_rules_set(&guard, rules_json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::MemStore;

    #[test]
    fn app_state_reads_through_to_store() {
        let mut store = MemStore::new();
        store.migrate().unwrap();
        let state = AppState::new(store);
        // пустое хранилище читается без паники и блокировок
        assert!(state.instruments().unwrap().is_empty());
        assert!(state.sector_rollup(0, i64::MAX).unwrap().is_empty());
    }

    #[test]
    fn history_catalog_register_list_delete() {
        let mut store = MemStore::new();
        store.migrate().unwrap();
        let state = AppState::new(store);
        assert!(state.history_datasets().is_empty());

        state.history_register(DatasetMeta {
            source: DataSource::Finam,
            secid: "SBER".into(),
            tf: TimeFrame::D1,
            range: domain::history::TimeRange::new(0, 86_400 * 10),
            bars: 10,
            updated_ts: 86_400 * 10,
        });
        let ds = state.history_datasets();
        assert_eq!(ds.len(), 1);
        assert_eq!(ds[0].secid, "SBER");
        assert_eq!(ds[0].source, "finam");
        assert_eq!(ds[0].tf, "d1");

        let removed = state
            .history_delete(&DatasetIdInput {
                source: "finam".into(),
                secid: "SBER".into(),
                tf: "d1".into(),
            })
            .unwrap();
        assert!(removed);
        assert!(state.history_datasets().is_empty());
    }

    #[test]
    fn history_catalog_hydrates_from_store_on_construct() {
        use domain::history::TimeRange;
        // Наполняем стор напрямую, затем «перезапуск» — новый AppState поверх
        // того же наполненного стора должен сразу видеть датасеты.
        let mut store = MemStore::new();
        store.migrate().unwrap();
        store
            .upsert_dataset(&DatasetMeta {
                source: DataSource::Finam,
                secid: "SBER".into(),
                tf: TimeFrame::D1,
                range: TimeRange::new(0, 86_400),
                bars: 1,
                updated_ts: 86_400,
            })
            .unwrap();

        let state = AppState::new(store);
        let ds = state.history_datasets();
        assert_eq!(ds.len(), 1);
        assert_eq!(ds[0].secid, "SBER");
        assert_eq!(ds[0].source, "finam");
    }

    #[test]
    fn history_delete_purges_catalog_and_bars() {
        use domain::history::{HistoryBar, TimeRange};
        let mut store = MemStore::new();
        store.migrate().unwrap();
        store
            .insert_history_bars(&[HistoryBar::ohlcv(
                DataSource::Finam,
                "SBER",
                TimeFrame::D1,
                0,
                1.0,
                1.0,
                1.0,
                1.0,
                1.0,
            )])
            .unwrap();
        store
            .upsert_dataset(&DatasetMeta {
                source: DataSource::Finam,
                secid: "SBER".into(),
                tf: TimeFrame::D1,
                range: TimeRange::new(0, 86_400),
                bars: 1,
                updated_ts: 86_400,
            })
            .unwrap();

        let state = AppState::new(store);
        assert_eq!(state.history_datasets().len(), 1);

        let removed = state
            .history_delete(&DatasetIdInput {
                source: "finam".into(),
                secid: "SBER".into(),
                tf: "d1".into(),
            })
            .unwrap();
        assert!(removed);
        // Каталог пуст и бары стёрты (нет сирот в хранилище).
        assert!(state.history_datasets().is_empty());
        let count = state
            .read(|s| s.count_history_bars(DataSource::Finam, "SBER", TimeFrame::D1, 0, i64::MAX))
            .unwrap();
        assert_eq!(count, 0);
    }

    /// Изолированная временная директория для теста (не трогает реальный
    /// пользовательский config-каталог). Удаляется при drop.
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "market-terminal-state-test-{tag}-{}-{:?}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn state_settings_and_key_activity_rules_roundtrip() {
        let mut store = MemStore::new();
        store.migrate().unwrap();
        let tmp = TempDir::new("state-settings");
        let state = AppState::with_settings_dir(store, tmp.0.clone());

        assert_eq!(state.settings_get(), SettingsDto::default());
        let custom = SettingsDto {
            dom_depth: 15,
            ..SettingsDto::default()
        };
        state.settings_set(custom.clone()).unwrap();
        assert_eq!(state.settings_get(), custom);

        assert!(state.key_activity_rules_get().is_empty());
        let json = r#"[{
            "id": "r1", "name": "Правило",
            "scope": {"kind": "market"},
            "expr": {"Cond": {"metric": "volume", "cmp": "ge", "threshold": 100.0}},
            "weight": 1.0
        }]"#;
        let saved = state.key_activity_rules_set(json).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(state.key_activity_rules_get().len(), 1);

        // Невалидный JSON отклоняется и не портит уже сохранённое.
        assert!(state.key_activity_rules_set("{not json").is_err());
        assert_eq!(state.key_activity_rules_get().len(), 1);
    }
}
