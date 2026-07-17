//! Живой smoke фазы 10: датасеты MOEX ALGOPACK (боевой ключ `MOEX_ALGO_API`)
//! и один вызов LLM-провайдера (OpenRouter, ключ `OPENROUTER_API_KEY`).
//!
//! Дополняет `live_check` (Finam gRPC): по одному запросу на каждый датасет
//! ALGOPACK через боевой клиент [`data::moex::MoexAlgo`] — сверка живого
//! контракта ISS с парсерами (T14). Секреты читаются из переменных окружения
//! или ближайшего `.env` и не печатаются.
//!
//! Запуск:
//! ```bash
//! cargo run -p data --features "moex,llm" --example algopack_check
//! ```

use data::http::ReqwestTransport;
use data::moex::{DateRange, Market, MoexAlgo};

fn env_or_dotenv(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_owned())
        .or_else(|| {
            let cwd = std::env::current_dir().ok()?;
            data::dotenv::find_dotenv_value(&cwd, 4, &[key])
        })
}

/// Вчерашняя дата UTC (`YYYY-MM-DD`) — свежий торговый день для смоука.
fn recent_date(days_back: i64) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("время до эпохи")
        .as_secs() as i64
        - days_back * 86_400;
    let (y, m, d) = domain::calendar::civil_from_days(ts.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

#[tokio::main]
async fn main() {
    let mut failures = 0u32;

    // ── ALGOPACK ─────────────────────────────────────────────────────────────
    match env_or_dotenv("MOEX_ALGO_API") {
        None => {
            eprintln!("MOEX_ALGO_API не задан (env/.env) — смоук ALGOPACK пропущен.");
            failures += 1;
        }
        Some(key) => {
            let transport = ReqwestTransport::new().expect("reqwest транспорт");
            let algo = MoexAlgo::new(transport, key);
            let range = DateRange::new(recent_date(1), recent_date(1));

            match algo
                .tradestats(Market::Eq, Some("SBER"), range.clone())
                .await
            {
                Ok(v) if !v.is_empty() => println!("TRADESTATS OK: {} строк (SBER)", v.len()),
                Ok(_) => {
                    println!("TRADESTATS ПУСТО (выходной?)");
                }
                Err(e) => {
                    eprintln!("TRADESTATS ERR: {e}");
                    failures += 1;
                }
            }
            match algo
                .orderstats(Market::Eq, Some("SBER"), range.clone())
                .await
            {
                Ok(v) if !v.is_empty() => println!("ORDERSTATS OK: {} строк (SBER)", v.len()),
                Ok(_) => {
                    println!("ORDERSTATS ПУСТО (выходной?)");
                }
                Err(e) => {
                    eprintln!("ORDERSTATS ERR: {e}");
                    failures += 1;
                }
            }
            match algo.obstats(Market::Eq, Some("SBER"), range.clone()).await {
                Ok(v) if !v.is_empty() => println!("OBSTATS OK: {} строк (SBER)", v.len()),
                Ok(_) => {
                    println!("OBSTATS ПУСТО (выходной?)");
                }
                Err(e) => {
                    eprintln!("OBSTATS ERR: {e}");
                    failures += 1;
                }
            }
            match algo.hi2(Market::Eq, range.clone()).await {
                Ok(v) if !v.is_empty() => println!(
                    "HI2 OK: {} точек hhi_volume, первая concentration={:.4}",
                    v.len(),
                    v[0].concentration
                ),
                Ok(_) => {
                    println!("HI2 ПУСТО (выходной?)");
                }
                Err(e) => {
                    eprintln!("HI2 ERR: {e}");
                    failures += 1;
                }
            }
            match algo.futoi(Some("Si"), DateRange::all()).await {
                Ok(v) if !v.is_empty() => println!("FUTOI OK: {} точек (Si)", v.len()),
                Ok(_) => {
                    println!("FUTOI ПУСТО");
                }
                Err(e) => {
                    eprintln!("FUTOI ERR: {e}");
                    failures += 1;
                }
            }
            match algo.candles(Market::Eq, "SBER", 60, range).await {
                Ok(v) if !v.is_empty() => println!("CANDLES OK: {} свечей H1 (SBER)", v.len()),
                Ok(_) => {
                    println!("CANDLES ПУСТО (выходной?)");
                }
                Err(e) => {
                    eprintln!("CANDLES ERR: {e}");
                    failures += 1;
                }
            }
        }
    }

    // ── LLM (OpenRouter) ─────────────────────────────────────────────────────
    match env_or_dotenv("OPENROUTER_API_KEY") {
        None => {
            eprintln!("OPENROUTER_API_KEY не задан (env/.env) — смоук LLM пропущен.");
            failures += 1;
        }
        Some(key) => {
            use data::llm::{LlmProvider, LlmRequest, OpenRouter};
            let transport = ReqwestTransport::new().expect("reqwest транспорт");
            let provider = OpenRouter::new(data::http::HttpClient::new(transport), key);
            // Модель — дефолт настроек терминала (`SettingsDto::default`).
            let req = LlmRequest {
                system: None,
                prompt: "Ответь одним словом: работает ли этот вызов?".to_owned(),
                model: "anthropic/claude-sonnet-5".to_owned(),
                // Reasoning-модели тратят бюджет на размышления: слишком
                // маленький лимит даёт `finish_reason=length` с пустым
                // `content` (сверено живым вызовом).
                max_tokens: 256,
            };
            match provider.summarize(req).await {
                Ok(text) if !text.trim().is_empty() => {
                    println!("LLM OK: ответ {} символов", text.trim().chars().count());
                }
                Ok(_) => {
                    eprintln!("LLM: пустой ответ");
                    failures += 1;
                }
                Err(e) => {
                    eprintln!("LLM ERR: {e}");
                    failures += 1;
                }
            }
        }
    }

    if failures > 0 {
        eprintln!("SMOKE: {failures} сбоев");
        std::process::exit(1);
    }
    println!("SMOKE: всё зелёное");
}
