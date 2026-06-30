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
mod state;
mod telemetry;
mod trade;

// Планировщик ингеста — сервис, который потребляет десктопный рантайм (связка с
// живым `data::MarketData`) и юнит-тесты. В headless-сборке часть его API
// (боевой цикл `run`) не вызывается, поэтому глушим dead_code на уровне модуля.
#[cfg(feature = "ingest")]
#[allow(dead_code)]
mod ingest;

#[cfg(feature = "live")]
mod live;

// Replay-источник (offline-режим): реализует `MarketData` из сохранённых баров.
// В бинаре напрямую не вызывается — потребляется тестами и replay-сценариями.
#[cfg(feature = "ingest")]
#[allow(dead_code)]
mod replay;

#[cfg(feature = "tauri")]
mod tauri_app;

use storage::schema;

// Импорты и хелперы консольного smoke нужны только когда не собран ни Tauri-UI,
// ни боевой live-режим (оба не вызывают демо-наполнение).
#[cfg(not(any(feature = "tauri", feature = "live")))]
use domain::{AssetClass, Bar, BookLevel, OrderBook, TimeFrame, Trade};
#[cfg(not(any(feature = "tauri", feature = "live")))]
use dto::{AlertRuleInput, OrderBookDto, TradeDto};
#[cfg(not(any(feature = "tauri", feature = "live")))]
use state::AppState;
#[cfg(not(any(feature = "tauri", feature = "live")))]
use storage::ingest::Writer;
#[cfg(not(any(feature = "tauri", feature = "live")))]
use storage::{MemStore, Store};

#[cfg(not(any(feature = "tauri", feature = "live")))]
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

/// Наполнить хранилище демонстрационными данными (для smoke без живого API).
#[cfg(not(any(feature = "tauri", feature = "live")))]
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

    // V2 — демо-лента сделок для footprint/дельты и детектирующих роботов:
    // серия из шести равных лотов по 10 на первом дневном баре (ts=1).
    let demo_trades: Vec<Trade> = (0..6)
        .map(|i| Trade {
            ts: 1,
            price: 300.0 + (i % 2) as f64,
            size: 10.0,
            buyer_initiated: Some(i % 3 != 0),
        })
        .collect();
    w.trades("SBER@MISX", &demo_trades)?;

    Ok(store)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    telemetry::init();
    tracing::info!(
        endpoint = finam_proto::ENDPOINT,
        schema_tables = schema::ALL_DDL.len(),
        "market terminal запускается"
    );

    // Боевой режим: live-подключение к Finam (нужны egress-доступ к
    // trade-api.finam.ru:443 и `FINAM_API_SECRET`/keyring).
    #[cfg(feature = "live")]
    {
        // `market-terminal --store-secret` — сохранить FINAM_API_SECRET в keyring.
        #[cfg(feature = "keyring")]
        if std::env::args().any(|a| a == "--store-secret") {
            live::store_secret_from_env()?;
            return Ok(());
        }
        let mic = std::env::var("FINAM_MIC").unwrap_or_else(|_| "MISX".to_owned());
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        // `return` нужен для прочих конфигураций сборки (Tauri/smoke ниже).
        #[allow(clippy::needless_return)]
        return rt.block_on(live::run(&mic));
    }

    #[cfg(feature = "tauri")]
    {
        tauri_app::run();
        return Ok(());
    }

    #[cfg(not(any(feature = "tauri", feature = "live")))]
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

        // Фаза 7 — live-панели: алёрты по сохранённым барам + маппинг
        // Time&Sales/DOM (живые данные приходят стримом, здесь — демо маппинга).
        let rules = vec![AlertRuleInput {
            symbol: "SBER@MISX".into(),
            kind: "priceAbove".into(),
            threshold: 300.0,
        }];
        println!(
            "  alerts_scan(SBER>300): {} срабатываний",
            state.alerts_scan(&rules, 0, i64::MAX)?.len()
        );
        let trade = TradeDto::from(&Trade {
            ts: 3,
            price: 305.0,
            size: 12.0,
            buyer_initiated: Some(true),
        });
        println!("  trade→dto: цена={} объём={}", trade.price, trade.size);
        let book = OrderBookDto::from(&OrderBook {
            ts: 3,
            bids: vec![BookLevel {
                price: 304.5,
                size: 50.0,
            }],
            asks: vec![BookLevel {
                price: 305.5,
                size: 40.0,
            }],
        });
        println!(
            "  orderbook→dto: {} бид(ов) / {} аск(ов)",
            book.bids.len(),
            book.asks.len()
        );

        // V2 — бэктестер + Delta (footprint/дельта и роботы) на MemStore.
        println!(
            "  list_strategies(): {} стратегий",
            state.list_strategies().len()
        );
        let bt_cfg = dto::BacktestConfigInput {
            initial_capital: 100_000.0,
            commission: 0.0,
            slippage: 0.0,
            fill_timing: None,
        };
        let report = state.run_backtest(
            "SBER@MISX",
            TimeFrame::D1,
            0,
            i64::MAX,
            "ma_cross",
            &domain::backtest::StrategyParams::new(),
            &bt_cfg,
        )?;
        println!(
            "  run_backtest(ma_cross): {} сделок, P&L={:+.0}",
            report.trades.len(),
            report.metrics.net_pnl
        );
        let fp = state.delta_footprint("SBER@MISX", TimeFrame::D1, 0, i64::MAX, 1.0)?;
        let total_delta: f64 = fp.iter().map(|b| b.delta).sum();
        println!(
            "  delta_footprint(SBER@MISX): {} баров, суммарная дельта={:+.0}",
            fp.len(),
            total_delta
        );
        let signals =
            state.robot_scan("SBER@MISX", 0, i64::MAX, &dto::RobotConfigInput::default())?;
        println!(
            "  robot_scan(SBER@MISX): {} сигналов роботов",
            signals.len()
        );

        // V2 — симулятор торговли: подаём стакан, ставим рыночную заявку.
        state.trade_session().on_book(&OrderBook {
            ts: 3,
            bids: vec![BookLevel {
                price: 304.5,
                size: 100.0,
            }],
            asks: vec![BookLevel {
                price: 305.5,
                size: 100.0,
            }],
        });
        match state.submit_order(&dto::OrderInput {
            symbol: "SBER@MISX".into(),
            side: "buy".into(),
            qty: 10.0,
            kind: "market".into(),
            price: None,
            tif: None,
        }) {
            Ok(res) => println!(
                "  submit_order(market buy 10): статус={}, исполнений={}",
                res.order.status,
                res.fills.len()
            ),
            Err(e) => println!("  submit_order: отклонено — {e}"),
        }
        println!(
            "  positions(): {} | account.cash={:.0} | blotter={}",
            state.positions().len(),
            state.account().cash,
            state.order_blotter().len()
        );
        // Резервная лимитка + отмена; прокрутка ленты через симулятор.
        if let Ok(res) = state.submit_order(&dto::OrderInput {
            symbol: "SBER@MISX".into(),
            side: "buy".into(),
            qty: 5.0,
            kind: "limit".into(),
            price: Some(300.0),
            tif: None,
        }) {
            let _ = state.cancel_order(res.order.id);
        }
        let sim_fills = state.trade_session().on_trade(&Trade {
            ts: 4,
            price: 305.5,
            size: 5.0,
            buyer_initiated: Some(true),
        });
        println!("  sim on_trade: {} исполнений по ленте", sim_fills.len());

        Ok(())
    }
}
