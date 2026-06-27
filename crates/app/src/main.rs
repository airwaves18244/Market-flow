//! Точка входа десктопного терминала.
//!
//! ## Статус: каркас Tauri (Фаза 3)
//!
//! Ядро IPC ([`api`]/[`dto`]/[`state`]) реализовано и протестировано на
//! `MemStore`. Привязка к Tauri (команды, события, билдер) живёт в модуле
//! [`tauri_app`] за фичей `tauri` — её сборка требует десктопного окружения
//! (webkit2gtk), поэтому по умолчанию выключена и не ломает кросс-платформенный
//! CI. Без фичи `tauri` бинарь работает как консольный smoke, прогоняющий путь
//! данных `domain` → `storage` → `dto`.

mod api;
mod dto;
mod ingest;
mod state;

#[cfg(feature = "tauri")]
mod tauri_app;

use data::auth::TokenManager;
use data::rate_limit::RateLimiter;
use data::secret::{SecretStore, StaticSecretStore};
use data::{DataError, MarketData};
use domain::{AssetClass, Bar, Instrument, Quote, TimeFrame, Trade};
use ingest::{poll_cycle, sync_instruments, DEFAULT_MAX_BARS};
use state::AppState;
use storage::ingest::{BatchCursor, Writer};
use storage::{schema, MemStore, Store};

fn demo_bar(ts: i64, open: f64, close: f64, volume: f64) -> Bar {
    Bar {
        ts,
        open,
        high: open.max(close),
        low: open.min(close),
        close,
        volume,
    }
}

/// Минимальный демо-исполнитель фьючерсов для smoke (продакшен использует
/// tokio). Гоняет `poll` до готовности; демо-фьючерсы готовы сразу.
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    use std::pin::pin;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    fn noop(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);

    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut cx = Context::from_waker(&waker);
    let mut fut = pin!(fut);
    loop {
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

/// Инициализировать `tracing`: форматированный вывод с фильтром по `RUST_LOG`
/// (по умолчанию `info`). Идемпотентна — повторный вызов безопасен.
fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).with_target(false).try_init();
}

/// Демо-источник рыночных данных для smoke (вместо живого gRPC-клиента Finam):
/// отдаёт небольшой справочник и синтетические дневные бары.
struct DemoSource;

impl MarketData for DemoSource {
    async fn assets(&self, _mic: &str) -> Result<Vec<Instrument>, DataError> {
        Ok(vec![
            Instrument {
                symbol: "SBER@MISX".into(),
                ticker: "SBER".into(),
                name: "Сбербанк".into(),
                asset_class: AssetClass::Equity,
                sector: Some("Финансы".into()),
                lot_size: 10,
                isin: None,
            },
            Instrument {
                symbol: "LKOH@MISX".into(),
                ticker: "LKOH".into(),
                name: "Лукойл".into(),
                asset_class: AssetClass::Equity,
                sector: Some("Нефтегаз".into()),
                lot_size: 1,
                isin: None,
            },
        ])
    }

    async fn bars(
        &self,
        symbol: &str,
        _tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<Bar>, DataError> {
        let base = if symbol.starts_with("LKOH") {
            7000.0
        } else {
            300.0
        };
        let bars: Vec<Bar> = (from_ts.max(1)..=to_ts.min(5))
            .map(|ts| demo_bar(ts, base, base * 1.01, 1_000.0))
            .collect();
        Ok(bars)
    }

    async fn last_quote(&self, _symbol: &str) -> Result<Quote, DataError> {
        Err(DataError::Other("демо: котировки не реализованы".into()))
    }

    async fn latest_trades(&self, _symbol: &str) -> Result<Vec<Trade>, DataError> {
        Ok(Vec::new())
    }
}

/// Наполнить хранилище демонстрационными данными (для smoke без живого API).
fn seed_demo_store() -> Result<MemStore, Box<dyn std::error::Error>> {
    use domain::Instrument;

    let mut store = MemStore::new();
    store.migrate()?;
    store.upsert_instruments(&[
        Instrument {
            symbol: "SBER@MISX".into(),
            ticker: "SBER".into(),
            name: "Сбербанк".into(),
            asset_class: AssetClass::Equity,
            sector: Some("Финансы".into()),
            lot_size: 10,
            isin: Some("RU0009029540".into()),
        },
        Instrument {
            symbol: "LKOH@MISX".into(),
            ticker: "LKOH".into(),
            name: "Лукойл".into(),
            asset_class: AssetClass::Equity,
            sector: Some("Нефтегаз".into()),
            lot_size: 1,
            isin: None,
        },
        Instrument {
            symbol: "SiH5@RTSX".into(),
            ticker: "SiH5".into(),
            name: "Si-3.25 (USD/RUB)".into(),
            asset_class: AssetClass::Future,
            sector: None,
            lot_size: 1,
            isin: None,
        },
        Instrument {
            symbol: "SU26240@MISX".into(),
            ticker: "SU26240".into(),
            name: "ОФЗ 26240".into(),
            asset_class: AssetClass::Bond,
            sector: None,
            lot_size: 1,
            isin: None,
        },
    ])?;

    let mut w = Writer::new(&mut store);
    w.load_sector_map([("SBER", "Финансы"), ("LKOH", "Нефтегаз")])?;
    for (sym, base) in [
        ("SBER@MISX", 300.0),
        ("LKOH@MISX", 7000.0),
        ("SiH5@RTSX", 90_000.0),
        ("SU26240@MISX", 800.0),
    ] {
        let bars = [
            demo_bar(1, base, base * 1.01, 1_000.0),
            demo_bar(2, base * 1.01, base * 0.999, 900.0),
            demo_bar(3, base * 0.999, base * 1.02, 1_500.0),
        ];
        w.bars(sym, TimeFrame::D1, &bars)?;
        w.snapshot_from_bars(sym, &bars, 3)?;
    }
    Ok(store)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    #[cfg(feature = "tauri")]
    {
        tauri_app::run();
        return Ok(());
    }

    #[cfg(not(feature = "tauri"))]
    {
        println!("market terminal — каркас (Фаза 3: Tauri-оболочка + IPC)");
        println!("Классы активов: {:?}", AssetClass::ALL);
        println!("Эндпоинт API: {}", finam_proto::ENDPOINT);
        println!("Таблиц в схеме DuckDB: {}", schema::ALL_DDL.len());

        let store = seed_demo_store()?;
        let state = AppState::new(store);

        println!("\nIPC-команды (демо на MemStore):");
        println!("  instruments(): {}", state.instruments()?.len());
        let rows = state.sector_rollup(0, i64::MAX)?;
        println!("  sector_rollup(): {} секторов", rows.len());
        for r in &rows {
            println!(
                "    {:<10} turnover={:>12.0} change={:+.2}%",
                r.sector,
                r.turnover,
                r.weighted_change * 100.0
            );
        }
        let series = state.turnover_series("SBER@MISX", 0, i64::MAX)?;
        println!("  turnover_series(SBER@MISX): {} точек", series.len());
        let candles = state.bars("SBER@MISX", TimeFrame::D1, 0, i64::MAX)?;
        println!("  bars(SBER@MISX, d1): {} свечей", candles.len());
        println!("  sector_map(): {} записей", state.sector_map()?.len());

        // Фаза 4 — представление «Акции/секторы».
        let breadth = state.breadth_data(0, i64::MAX)?;
        println!(
            "  breadth(): +{} / -{} (растущих {:.0}%)",
            breadth.advancers,
            breadth.decliners,
            breadth.pct_advancing.unwrap_or(0.0) * 100.0
        );
        let movers = state.top_movers(0, i64::MAX, Some(3))?;
        println!("  top_movers(3): {} строк", movers.len());
        for m in &movers {
            println!("    {:<8} {:+.2}%", m.ticker, m.change * 100.0);
        }
        println!(
            "  rrg_sectors(): {} секторов",
            state.rrg_sectors(0, i64::MAX)?.len()
        );

        // Фаза 5 — представления «Фьючерсы» и «Облигации».
        let futures = state.futures_rollup(0, i64::MAX)?;
        println!("  futures_rollup(): {} групп", futures.len());
        for f in &futures {
            println!(
                "    {:<4} contracts={} turnover={:.0}",
                f.group, f.contracts, f.turnover
            );
        }
        let bonds = state.bonds_rollup(0, i64::MAX)?;
        println!("  bonds_rollup(): {} эмитентов", bonds.len());
        for b in &bonds {
            println!(
                "    {:<6} bonds={} turnover={:.0}",
                b.issuer, b.bonds, b.turnover
            );
        }
        println!("  yield_curve(): {} точек", state.yield_curve()?.len());

        // Фаза 6 — представление «Сумма всех» (кросс-актив).
        let summary = state.cross_asset_summary(0, i64::MAX)?;
        println!("  cross_asset_summary(): итого {:.0}", summary.total);
        for s in &summary.shares {
            println!("    {:<8} доля={:.1}%", s.asset_class, s.share * 100.0);
        }
        println!(
            "  turnover_timeline(): {} точек",
            state.turnover_timeline(0, i64::MAX)?.len()
        );
        println!(
            "  flow_sankey(): {} рёбер перетока",
            state.flow_sankey(0, i64::MAX)?.len()
        );

        // Фаза 7 — live-функции (DOM, Time & Sales, алёрты, replay).
        let book = state.order_book("SBER@MISX", 10)?;
        println!(
            "  order_book(SBER@MISX): {} bids / {} asks, спред {:.2}, дисбаланс {:+.2}",
            book.bids.len(),
            book.asks.len(),
            book.spread.unwrap_or(0.0),
            book.imbalance.unwrap_or(0.0)
        );
        let tape = state.time_and_sales("SBER@MISX", 50)?;
        println!(
            "  time_and_sales(SBER@MISX): {} сделок, CVD {:+.0}, VWAP {:.2}",
            tape.stats.trades,
            tape.stats.cvd,
            tape.stats.vwap.unwrap_or(0.0)
        );
        let alerts = state.active_alerts()?;
        println!("  active_alerts(): {} срабатываний", alerts.len());
        for a in &alerts {
            println!("    [{}] {} — {}", a.severity, a.symbol, a.message);
        }
        let replay = state.replay_state("SBER@MISX", 2)?;
        println!(
            "  replay_state(SBER@MISX, 2): кадр {}/{} ({:.0}%)",
            replay.pos,
            replay.frames,
            replay.progress * 100.0
        );

        // Фаза 0 — инфраструктура авторизации (без сети).
        println!("\nФаза 0 — инфраструктура data (демо):");
        let secret = StaticSecretStore::new("demo-api-secret");
        println!(
            "  secret.api_secret(): {} символов",
            secret.api_secret()?.len()
        );
        let mut tokens = TokenManager::new();
        let jwt =
            block_on(tokens.valid_token(0, || async { Ok(("demo.jwt.token".to_string(), 900)) }))
                .map_err(|e| e.to_string())?;
        println!(
            "  token_manager: получен JWT ({} симв.), TTL 900с",
            jwt.len()
        );
        let mut limiter = RateLimiter::per_minute(200);
        let ok = limiter.try_acquire("Bars", 0);
        println!(
            "  rate_limit(Bars 200/мин): первый запрос={}, остаток={}",
            ok,
            limiter.available("Bars", 0)
        );

        // Фаза 1 — асинхронный цикл ингеста (демо-источник → MemStore).
        println!("\nФаза 1 — цикл ингеста (демо-источник → MemStore):");
        let src = DemoSource;
        let mut ingest_store = MemStore::new();
        ingest_store.migrate()?;
        let synced = block_on(sync_instruments(&src, &mut ingest_store, &["MISX"]))
            .map_err(|e| e.to_string())?;
        println!("  sync_instruments(): {synced} инструментов");
        let mut cursor = BatchCursor::new(
            ingest_store
                .instruments()?
                .iter()
                .map(|i| i.symbol.clone())
                .collect(),
            10,
        );
        let report = block_on(poll_cycle(
            &src,
            &mut ingest_store,
            &mut cursor,
            TimeFrame::D1,
            1,
            10,
            DEFAULT_MAX_BARS,
        ));
        println!(
            "  poll_cycle(): дозагружено {} символов, {} баров, ошибок {}",
            report.backfilled.len(),
            report.bars_written(),
            report.errors.len()
        );

        Ok(())
    }
}
