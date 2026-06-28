//! Живой smoke полного gRPC-пайплайна Finam Trade API.
//!
//! Проверяет сквозной путь: TLS-подключение → `AuthService.Auth` (обмен секрета
//! на JWT) → `AssetsService.Assets` → `MarketDataService.Bars`/`LastQuote`.
//!
//! Секрет читается из переменной окружения `FINAM_API_SECRET` и **не** попадает
//! ни в репозиторий, ни в логи. Без секрета smoke проверяет только связность:
//! сервер ответит ошибкой авторизации (это и подтверждает, что весь пайплайн —
//! сеть/TLS/protobuf/gRPC — работает до точки авторизации).
//!
//! Запуск:
//! ```bash
//! FINAM_API_SECRET=… cargo run -p data --features grpc --example live_check
//! ```

use data::{
    AuthManager, FinamMarketData, GrpcAuthTransport, MarketData, MemSecretStore, TimeFrame,
};

#[tokio::main]
async fn main() {
    // Секрет: переменная окружения `FINAM_API_SECRET`, иначе файл `.env`
    // (ключи `FINAM_API_SECRET`/`FINAM_SECRET`, без учёта регистра).
    let secret = std::env::var(data::SECRET_ENV_VAR)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_owned())
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .and_then(|cwd| data::find_dotenv_secret(&cwd, 4))
        })
        .unwrap_or_default();
    if secret.is_empty() {
        eprintln!(
            "Секрет не задан (ни FINAM_API_SECRET, ни .env) — проверяем только \
             связность (ожидается ошибка авторизации от сервера)."
        );
    }

    let auth = AuthManager::new(
        GrpcAuthTransport::new(),
        MemSecretStore::with_secret(secret),
    );
    let md = match FinamMarketData::connect(auth) {
        Ok(md) => md,
        Err(e) => {
            eprintln!("Не удалось подготовить клиент: {e}");
            std::process::exit(1);
        }
    };

    println!("Подключение к {} …", finam_proto::ENDPOINT);
    match md.assets("MISX").await {
        Ok(instruments) => {
            println!("ASSETS OK: {} инструментов MISX", instruments.len());
            if let Some(first) = instruments.first() {
                let to = now_unix();
                let from = to - 7 * 24 * 60 * 60;
                match md.bars(&first.symbol, TimeFrame::D1, from, to).await {
                    Ok(bars) => println!("BARS OK ({}): {} баров", first.symbol, bars.len()),
                    Err(e) => println!("BARS ERR ({}): {e}", first.symbol),
                }
                match md.last_quote(&first.symbol).await {
                    Ok(q) => println!("QUOTE OK ({}): last={}", first.symbol, q.last),
                    Err(e) => println!("QUOTE ERR ({}): {e}", first.symbol),
                }
            }
        }
        Err(e) => println!("ASSETS ERR (ожидаемо без секрета): {e}"),
    }
}

fn now_unix() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
