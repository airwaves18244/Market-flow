//! Сервис историзации: фоновая загрузка исторических баров с планом
//! дозагрузки, событиями прогресса и отменой (фаза 11.3).
//!
//! Оркестрация построена вокруг чистого асинхронного цикла [`run_load`],
//! который проходит очередь задач `(тикер, TF)` последовательно и
//! детерминированно (порядок событий воспроизводим — важно и для UI, и для
//! тестов):
//! - план загрузки строится через [`AppState::history_missing`] поверх
//!   [`storage::Store::history_missing_ranges`] — качаются только «дыры», уже
//!   покрытые каталогом диапазоны пропускаются;
//! - бары тянутся абстрактным [`HistorySource`] (в тестах —
//!   `data::FakeHistorySource`, в бою — `FinamHistory`/`MoexHistory`), пишутся
//!   в стор ([`AppState::ingest_history_bars`]) и фиксируются в каталоге
//!   ([`AppState::history_commit`]);
//! - прогресс/завершение/ошибка отдаются колбэком [`HistoryEvent`] — ошибка
//!   одной задачи не роняет остальные (логируется как `Error`, цикл идёт
//!   дальше);
//! - отмена — кооперативный флаг [`CancelFlag`]: проверяется перед каждой
//!   задачей и перед каждым «дырявым» диапазоном, поэтому останов не оставляет
//!   каталог в противоречивом состоянии (фиксируются только фактически
//!   обработанные диапазоны).
//!
//! Реестр задач [`HistoryTasks`] раздаёт `task_id` и хранит их флаги отмены,
//! чтобы IPC-команда `history_cancel(taskId?)` могла остановить конкретную
//! загрузку или все сразу. Модуль потребляет async-runtime (фича `ingest`);
//! боевые источники подключает Tauri-слой (`crate::tauri_app`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use data::history::HistorySource;
use domain::history::{normalize_ranges, DataSource, HistoryBar, TimeRange};
use domain::TimeFrame;

use crate::state::AppState;

/// Кооперативный флаг отмены загрузки. Клонируется дёшево (общий
/// `Arc<AtomicBool>`): одна копия живёт в реестре [`HistoryTasks`], другую
/// проверяет цикл [`run_load`].
#[derive(Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    /// Новый неотменённый флаг.
    pub fn new() -> Self {
        Self::default()
    }

    /// Пометить загрузку отменённой (идемпотентно).
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Отменена ли загрузка.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Реестр активных загрузок: `task_id → CancelFlag`. Позволяет отменить
/// конкретную задачу или все сразу, не завися от конкретного источника.
#[derive(Default)]
pub struct HistoryTasks {
    next: AtomicU64,
    flags: Mutex<HashMap<u64, CancelFlag>>,
}

impl HistoryTasks {
    /// Зарегистрировать новую загрузку: выдать свежий `task_id` и его флаг
    /// отмены (копия остаётся в реестре до [`HistoryTasks::finish`]).
    pub fn start(&self) -> (u64, CancelFlag) {
        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        let flag = CancelFlag::new();
        if let Ok(mut map) = self.flags.lock() {
            map.insert(id, flag.clone());
        }
        (id, flag)
    }

    /// Снять задачу с учёта (по завершении/отмене). Идемпотентно.
    pub fn finish(&self, id: u64) {
        if let Ok(mut map) = self.flags.lock() {
            map.remove(&id);
        }
    }

    /// Отменить задачу `id` (или все активные, если `None`). Возвращает число
    /// затронутых задач.
    pub fn cancel(&self, id: Option<u64>) -> usize {
        let map = match self.flags.lock() {
            Ok(m) => m,
            Err(_) => return 0,
        };
        match id {
            Some(id) => match map.get(&id) {
                Some(flag) => {
                    flag.cancel();
                    1
                }
                None => 0,
            },
            None => {
                for flag in map.values() {
                    flag.cancel();
                }
                map.len()
            }
        }
    }

    /// Число активных (незавершённых) задач — для диагностики/тестов.
    pub fn active(&self) -> usize {
        self.flags.lock().map(|m| m.len()).unwrap_or(0)
    }
}

/// Запрос на загрузку истории: источник, тикеры, набор тайм-фреймов и окно.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryLoadRequest {
    pub source: DataSource,
    pub tickers: Vec<String>,
    pub timeframes: Vec<TimeFrame>,
    pub range: TimeRange,
}

/// Событие хода загрузки для UI (эмиттеры `history:*` в Tauri-слое).
#[derive(Debug, Clone, PartialEq)]
pub enum HistoryEvent {
    /// Прогресс задачи `(ticker, tf)` в процентах `0..=100` (монотонно растёт).
    Progress {
        task_id: u64,
        ticker: String,
        tf: TimeFrame,
        percent: u8,
    },
    /// Завершение: задачи `(ticker, tf)` при `ticker=Some`, либо всей загрузки
    /// при `ticker=None` (итоговая сводка).
    Done {
        task_id: u64,
        ticker: Option<String>,
        tf: Option<TimeFrame>,
        bars: u64,
        summary: String,
    },
    /// Ошибка задачи `(ticker, tf)` — не прерывает остальные задачи очереди.
    Error {
        task_id: u64,
        ticker: Option<String>,
        tf: Option<TimeFrame>,
        message: String,
    },
}

/// Итог загрузки (возвращается из [`run_load`] для команды/тестов).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct HistorySummary {
    /// Всего записано баров.
    pub bars: u64,
    /// Успешно завершённых задач `(ticker, tf)`.
    pub completed: usize,
    /// Задач, завершившихся ошибкой.
    pub errors: usize,
    /// Была ли загрузка отменена.
    pub cancelled: bool,
}

/// Прогнать загрузку истории по очереди `(тикер × TF)` последовательно.
///
/// Возвращает сводку [`HistorySummary`]; по ходу дёргает `emit` событиями
/// [`HistoryEvent`]. Ошибка одной задачи фиксируется событием `Error` и не
/// прерывает очередь; отмена (`cancel`) прекращает обработку между задачами и
/// диапазонами. Каталог обновляется только по фактически обработанным
/// диапазонам, поэтому и ошибка, и отмена оставляют его согласованным.
pub async fn run_load<S, F>(
    state: &AppState,
    source: &S,
    req: &HistoryLoadRequest,
    task_id: u64,
    cancel: &CancelFlag,
    emit: &F,
) -> HistorySummary
where
    S: HistorySource,
    F: Fn(HistoryEvent),
{
    let mut summary = HistorySummary::default();

    'outer: for ticker in &req.tickers {
        for &tf in &req.timeframes {
            if cancel.is_cancelled() {
                break 'outer;
            }
            emit(HistoryEvent::Progress {
                task_id,
                ticker: ticker.clone(),
                tf,
                percent: 0,
            });
            match load_key(state, source, req, ticker, tf, task_id, cancel, emit).await {
                Ok(n) => {
                    summary.bars += n;
                    summary.completed += 1;
                    emit(HistoryEvent::Done {
                        task_id,
                        ticker: Some(ticker.clone()),
                        tf: Some(tf),
                        bars: n,
                        summary: format!("{ticker} · {}: {n} баров", tf.code()),
                    });
                }
                Err(message) => {
                    summary.errors += 1;
                    emit(HistoryEvent::Error {
                        task_id,
                        ticker: Some(ticker.clone()),
                        tf: Some(tf),
                        message,
                    });
                }
            }
        }
    }

    summary.cancelled = cancel.is_cancelled();
    let summary_text = if summary.cancelled {
        format!(
            "отменено: {} баров, задач {}, ошибок {}",
            summary.bars, summary.completed, summary.errors
        )
    } else {
        format!(
            "готово: {} баров, задач {}, ошибок {}",
            summary.bars, summary.completed, summary.errors
        )
    };
    emit(HistoryEvent::Done {
        task_id,
        ticker: None,
        tf: None,
        bars: summary.bars,
        summary: summary_text,
    });

    summary
}

/// Загрузить одну пару `(ticker, tf)`: план дыр → загрузка → запись →
/// фиксация каталога. Возвращает число записанных баров либо текст ошибки
/// (первого сбоя источника/хранилища).
#[allow(clippy::too_many_arguments)]
async fn load_key<S, F>(
    state: &AppState,
    source: &S,
    req: &HistoryLoadRequest,
    ticker: &str,
    tf: TimeFrame,
    task_id: u64,
    cancel: &CancelFlag,
    emit: &F,
) -> Result<u64, String>
where
    S: HistorySource,
    F: Fn(HistoryEvent),
{
    let missing = state
        .history_missing(req.source, ticker, tf, req.range)
        .map_err(|e| e.to_string())?;

    if missing.is_empty() {
        // Всё уже покрыто — сразу 100%, сеть не трогаем.
        emit(HistoryEvent::Progress {
            task_id,
            ticker: ticker.to_owned(),
            tf,
            percent: 100,
        });
        return Ok(0);
    }

    let total = missing.len() as u64;
    let mut written = 0u64;
    let mut attempted: Vec<TimeRange> = Vec::new();
    let mut failure: Option<String> = None;

    for (i, gap) in missing.iter().enumerate() {
        if cancel.is_cancelled() {
            break;
        }
        match source.load(ticker, tf, gap.from, gap.till).await {
            Ok(bars) => {
                // Страхуемся: берём только бары нужного ключа (источник обязан
                // это гарантировать, но фильтр дешевле дальнейшей путаницы).
                let bars: Vec<HistoryBar> = bars
                    .into_iter()
                    .filter(|b| b.secid == ticker && b.tf == tf)
                    .collect();
                if !bars.is_empty() {
                    written += state
                        .ingest_history_bars(&bars)
                        .map_err(|e| e.to_string())? as u64;
                }
                attempted.push(*gap);
                let percent = (((i as u64) + 1) * 100 / total) as u8;
                emit(HistoryEvent::Progress {
                    task_id,
                    ticker: ticker.to_owned(),
                    tf,
                    percent,
                });
            }
            Err(e) => {
                failure = Some(e.to_string());
                break;
            }
        }
    }

    // Фиксируем каталог по фактически обработанным диапазонам — даже при
    // ошибке/отмене на середине очереди дыр (что уже скачано, то и учтено).
    for covered in normalize_ranges(&attempted) {
        state
            .history_commit(req.source, ticker, tf, covered)
            .map_err(|e| e.to_string())?;
    }

    match failure {
        Some(message) => Err(message),
        None => Ok(written),
    }
}

/// Разобрать вход IPC (`HistoryLoadInput`) в [`HistoryLoadRequest`].
///
/// Живёт под фичей `tauri` (единственный потребитель — команда `history_load`);
/// в тестах запрос собирается напрямую.
#[cfg(feature = "tauri")]
pub fn parse_load_input(
    input: &crate::dto::HistoryLoadInput,
) -> Result<HistoryLoadRequest, String> {
    let source = DataSource::from_code(&input.source)
        .ok_or_else(|| format!("неизвестный источник: {}", input.source))?;
    if input.tickers.is_empty() {
        return Err("не выбран ни один тикер".to_owned());
    }
    if input.timeframes.is_empty() {
        return Err("не выбран ни один тайм-фрейм".to_owned());
    }
    let mut timeframes = Vec::with_capacity(input.timeframes.len());
    for code in &input.timeframes {
        let tf =
            TimeFrame::from_code(code).ok_or_else(|| format!("неизвестный тайм-фрейм: {code}"))?;
        timeframes.push(tf);
    }
    Ok(HistoryLoadRequest {
        source,
        tickers: input.tickers.clone(),
        timeframes,
        range: TimeRange::new(input.from, input.till),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::history::FakeHistorySource;
    use data::DataError;
    use domain::history::DatasetMeta;
    use std::sync::Mutex as StdMutex;
    use storage::{MemStore, Store};

    /// Собрать состояние поверх мигрированного `MemStore`.
    fn state() -> AppState {
        let mut store = MemStore::new();
        store.migrate().unwrap();
        AppState::new(store)
    }

    /// Бар источника `finam` для тикера/TF на отметке `ts`.
    fn bar(ticker: &str, tf: TimeFrame, ts: i64) -> HistoryBar {
        HistoryBar::ohlcv(
            DataSource::Finam,
            ticker,
            tf,
            ts,
            100.0,
            101.0,
            99.0,
            100.5,
            10.0,
        )
    }

    /// Коллектор событий: собирает [`HistoryEvent`] в порядке эмита.
    #[derive(Default)]
    struct Collector(StdMutex<Vec<HistoryEvent>>);
    impl Collector {
        fn push(&self, ev: HistoryEvent) {
            self.0.lock().unwrap().push(ev);
        }
        fn events(&self) -> Vec<HistoryEvent> {
            self.0.lock().unwrap().clone()
        }
        /// Проценты прогресса задачи `(ticker, tf)` в порядке эмита.
        fn progress(&self, ticker: &str, tf: TimeFrame) -> Vec<u8> {
            self.events()
                .into_iter()
                .filter_map(|e| match e {
                    HistoryEvent::Progress {
                        ticker: t,
                        tf: f,
                        percent,
                        ..
                    } if t == ticker && f == tf => Some(percent),
                    _ => None,
                })
                .collect()
        }
    }

    fn req(
        source: DataSource,
        tickers: &[&str],
        tfs: &[TimeFrame],
        from: i64,
        till: i64,
    ) -> HistoryLoadRequest {
        HistoryLoadRequest {
            source,
            tickers: tickers.iter().map(|s| s.to_string()).collect(),
            timeframes: tfs.to_vec(),
            range: TimeRange::new(from, till),
        }
    }

    #[tokio::test]
    async fn loads_bars_writes_store_and_updates_catalog() {
        let st = state();
        let day = TimeFrame::D1.seconds();
        let src = FakeHistorySource::with_bars(vec![
            bar("SBER", TimeFrame::D1, 0),
            bar("SBER", TimeFrame::D1, day),
            bar("SBER", TimeFrame::D1, 2 * day),
        ]);
        let col = Collector::default();
        let request = req(DataSource::Finam, &["SBER"], &[TimeFrame::D1], 0, 3 * day);

        let summary = run_load(&st, &src, &request, 1, &CancelFlag::new(), &|e| col.push(e)).await;

        assert_eq!(summary.bars, 3);
        assert_eq!(summary.completed, 1);
        assert_eq!(summary.errors, 0);
        assert!(!summary.cancelled);

        // Бары действительно в сторе.
        let stored = st.history_preview("finam", "SBER", "d1", 100).unwrap();
        assert_eq!(stored.len(), 3);

        // Каталог (in-memory, читается history_datasets) обновлён.
        let datasets = st.history_datasets();
        assert_eq!(datasets.len(), 1);
        assert_eq!(datasets[0].secid, "SBER");
        assert_eq!(datasets[0].bars, 3);
    }

    #[tokio::test]
    async fn progress_is_monotonic_zero_to_hundred() {
        let st = state();
        let day = TimeFrame::D1.seconds();
        let bars: Vec<HistoryBar> = (0..5)
            .map(|i| bar("SBER", TimeFrame::D1, i * day))
            .collect();
        let src = FakeHistorySource::with_bars(bars);
        let col = Collector::default();
        let request = req(DataSource::Finam, &["SBER"], &[TimeFrame::D1], 0, 5 * day);

        run_load(&st, &src, &request, 7, &CancelFlag::new(), &|e| col.push(e)).await;

        let pcts = col.progress("SBER", TimeFrame::D1);
        assert_eq!(pcts.first(), Some(&0));
        assert_eq!(pcts.last(), Some(&100));
        assert!(
            pcts.windows(2).all(|w| w[0] <= w[1]),
            "прогресс должен быть монотонным: {pcts:?}"
        );
    }

    #[tokio::test]
    async fn incremental_reload_fetches_only_gaps() {
        let st = state();
        let day = TimeFrame::D1.seconds();
        let src = FakeHistorySource::with_bars(vec![
            bar("SBER", TimeFrame::D1, 0),
            bar("SBER", TimeFrame::D1, day),
            bar("SBER", TimeFrame::D1, 2 * day),
        ]);
        let request = req(DataSource::Finam, &["SBER"], &[TimeFrame::D1], 0, 3 * day);

        // Первый прогон качает всё.
        let s1 = run_load(&st, &src, &request, 1, &CancelFlag::new(), &|_| {}).await;
        assert_eq!(s1.bars, 3);

        // Повторный прогон того же окна: дыр нет → 0 баров, задача завершена.
        let col = Collector::default();
        let s2 = run_load(&st, &src, &request, 2, &CancelFlag::new(), &|e| col.push(e)).await;
        assert_eq!(s2.bars, 0);
        assert_eq!(s2.completed, 1);
        // Прогресс сразу 100% (сеть не трогали).
        assert_eq!(col.progress("SBER", TimeFrame::D1), vec![0, 100]);
        // Баров в сторе не прибавилось.
        assert_eq!(
            st.history_preview("finam", "SBER", "d1", 100)
                .unwrap()
                .len(),
            3
        );
    }

    #[tokio::test]
    async fn cancel_stops_and_keeps_catalog_consistent() {
        let st = state();
        let day = TimeFrame::D1.seconds();
        let src = FakeHistorySource::with_bars(vec![bar("SBER", TimeFrame::D1, 0)]);
        // Флаг отменён до старта — ни одной задачи не обработаем.
        let cancel = CancelFlag::new();
        cancel.cancel();
        let request = req(
            DataSource::Finam,
            &["SBER", "GAZP"],
            &[TimeFrame::D1],
            0,
            3 * day,
        );

        let summary = run_load(&st, &src, &request, 1, &cancel, &|_| {}).await;
        assert!(summary.cancelled);
        assert_eq!(summary.completed, 0);
        assert_eq!(summary.bars, 0);
        // Каталог не тронут (нет наполовину записанных датасетов).
        assert!(st.history_datasets().is_empty());
        assert!(st
            .history_preview("finam", "SBER", "d1", 100)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn one_task_error_does_not_stop_the_rest() {
        // Источник всегда возвращает ошибку — но обе задачи должны быть
        // «пройдены» (по одной ошибке на каждую), сервис не падает.
        let st = state();
        let day = TimeFrame::D1.seconds();
        let src = FakeHistorySource {
            bars: Vec::new(),
            error: Some(DataError::Transport("сбой сети".into())),
        };
        let col = Collector::default();
        let request = req(
            DataSource::Finam,
            &["SBER", "GAZP"],
            &[TimeFrame::D1],
            0,
            day,
        );

        let summary = run_load(&st, &src, &request, 1, &CancelFlag::new(), &|e| col.push(e)).await;
        assert_eq!(summary.errors, 2);
        assert_eq!(summary.completed, 0);

        let errors: Vec<String> = col
            .events()
            .into_iter()
            .filter_map(|e| match e {
                HistoryEvent::Error { ticker, .. } => ticker,
                _ => None,
            })
            .collect();
        assert_eq!(errors, vec!["SBER", "GAZP"]);
        // Каталог пуст — ничего записать не удалось, но и не испорчено.
        assert!(st.history_datasets().is_empty());
    }

    #[tokio::test]
    async fn multi_tf_completes_each_key() {
        let st = state();
        let hour = TimeFrame::H1.seconds();
        let day = TimeFrame::D1.seconds();
        let src = FakeHistorySource::with_bars(vec![
            bar("SBER", TimeFrame::H1, 0),
            bar("SBER", TimeFrame::H1, hour),
            bar("SBER", TimeFrame::D1, 0),
        ]);
        let request = req(
            DataSource::Finam,
            &["SBER"],
            &[TimeFrame::H1, TimeFrame::D1],
            0,
            2 * day,
        );

        let summary = run_load(&st, &src, &request, 1, &CancelFlag::new(), &|_| {}).await;
        assert_eq!(summary.completed, 2);
        assert_eq!(summary.bars, 3);
        // Два разных датасета в каталоге.
        let mut tfs: Vec<String> = st.history_datasets().into_iter().map(|d| d.tf).collect();
        tfs.sort();
        assert_eq!(tfs, vec!["d1", "h1"]);
    }

    #[test]
    fn tasks_registry_cancels_by_id_and_all() {
        let tasks = HistoryTasks::default();
        let (id1, f1) = tasks.start();
        let (id2, f2) = tasks.start();
        assert_ne!(id1, id2);
        assert_eq!(tasks.active(), 2);

        assert_eq!(tasks.cancel(Some(id1)), 1);
        assert!(f1.is_cancelled());
        assert!(!f2.is_cancelled());

        assert_eq!(tasks.cancel(None), 2); // отменяет все активные
        assert!(f2.is_cancelled());

        tasks.finish(id1);
        tasks.finish(id2);
        assert_eq!(tasks.active(), 0);
        assert_eq!(tasks.cancel(Some(id1)), 0); // уже снята с учёта
    }

    #[test]
    fn catalog_commit_records_actual_bar_count() {
        // history_commit пересчитывает метаданные датасета по фактически
        // записанным барам и кладёт их в каталог.
        let st = state();
        let day = TimeFrame::D1.seconds();
        st.ingest_history_bars(&[
            bar("SBER", TimeFrame::D1, 0),
            bar("SBER", TimeFrame::D1, day),
        ])
        .unwrap();
        let meta: DatasetMeta = st
            .history_commit(
                DataSource::Finam,
                "SBER",
                TimeFrame::D1,
                TimeRange::new(0, 2 * day),
            )
            .unwrap();
        assert_eq!(meta.bars, 2);

        let datasets = st.history_datasets();
        assert_eq!(datasets.len(), 1);
        assert_eq!(datasets[0].bars, 2);
    }
}
