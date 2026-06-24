//! gRPC-клиент Finam Trade API: транспорт, авторизация, rate-limit (§ 0.2–0.4).
//!
//! [`FinamClient`] держит tonic-клиентов поверх общего TLS-канала, кэширует JWT
//! (с авто-refresh перед истечением) и реализует трейт [`MarketData`], переводя
//! ответы API в доменные типы через [`crate::convert`].
//!
//! Сетевые методы здесь не покрываются юнит-тестами (нет ключа/доступа к API);
//! проверяемая логика вынесена в `convert`, `auth`, `ratelimit`, `resilience`.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::RwLock;
use tonic::metadata::MetadataValue;
use tonic::transport::{Channel, ClientTlsConfig};
use tonic::{Code, Request, Status};

use finam_proto::{
    assets, auth, marketdata, AssetsServiceClient, AuthServiceClient, MarketDataServiceClient,
    ENDPOINT,
};

use crate::auth::TokenCache;
use crate::ratelimit::Limiters;
use crate::{convert, DataError, MarketData, TimeFrame};
use domain::{Bar, Instrument, Quote, Trade};

/// Запас по времени до истечения JWT, при котором инициируется refresh.
const REFRESH_MARGIN_SECS: i64 = 60;

/// Клиент Finam Trade API (read-only).
pub struct FinamClient {
    auth: AuthServiceClient<Channel>,
    assets: AssetsServiceClient<Channel>,
    market: MarketDataServiceClient<Channel>,
    secret: String,
    source_app_id: String,
    token: Arc<RwLock<TokenCache>>,
    limiters: Limiters,
}

impl FinamClient {
    /// Подключиться к [`ENDPOINT`] и получить первичный JWT по `secret`.
    pub async fn connect(secret: impl Into<String>) -> Result<Self, DataError> {
        Self::connect_with(secret, String::new()).await
    }

    /// Как [`connect`](Self::connect), но с явным `source_app_id`.
    pub async fn connect_with(
        secret: impl Into<String>,
        source_app_id: impl Into<String>,
    ) -> Result<Self, DataError> {
        let tls = ClientTlsConfig::new().with_webpki_roots();
        let channel = Channel::from_static(ENDPOINT)
            .tls_config(tls)
            .map_err(|e| DataError::Transport(e.to_string()))?
            .connect()
            .await
            .map_err(|e| DataError::Transport(e.to_string()))?;

        let client = Self {
            auth: AuthServiceClient::new(channel.clone()),
            assets: AssetsServiceClient::new(channel.clone()),
            market: MarketDataServiceClient::new(channel),
            secret: secret.into(),
            source_app_id: source_app_id.into(),
            token: Arc::new(RwLock::new(TokenCache::new())),
            limiters: Limiters::default(),
        };
        client.refresh().await?;
        Ok(client)
    }

    /// Обменять секрет на свежий JWT и узнать его срок действия.
    async fn refresh(&self) -> Result<(), DataError> {
        self.limiters.auth.acquire().await;
        let mut auth = self.auth.clone();
        let token = auth
            .auth(auth::AuthRequest {
                secret: self.secret.clone(),
                source_app_id: self.source_app_id.clone(),
            })
            .await
            .map_err(map_status)?
            .into_inner()
            .token;

        let details = auth
            .token_details(auth::TokenDetailsRequest {
                token: token.clone(),
            })
            .await
            .map_err(map_status)?
            .into_inner();
        let expires_at = details
            .expires_at
            .map(|t| t.seconds)
            .unwrap_or_else(|| now() + 600);

        tracing::debug!(expires_at, "обновлён JWT Finam");
        self.token.write().await.set(token, expires_at);
        Ok(())
    }

    /// Гарантировать актуальный токен и вернуть его строкой.
    async fn ensure_token(&self) -> Result<String, DataError> {
        if self.token.read().await.needs_refresh(now(), REFRESH_MARGIN_SECS) {
            self.refresh().await?;
        }
        self.token
            .read()
            .await
            .token()
            .map(str::to_string)
            .ok_or_else(|| DataError::Auth("нет токена после refresh".into()))
    }
}

/// Завернуть сообщение в запрос с заголовком авторизации.
fn authorize<T>(token: &str, message: T) -> Result<Request<T>, DataError> {
    let mut request = Request::new(message);
    let value: MetadataValue<_> = token
        .parse()
        .map_err(|_| DataError::Auth("некорректный токен для метаданных".into()))?;
    request.metadata_mut().insert("authorization", value);
    Ok(request)
}

/// Сопоставить gRPC-статус с доменной ошибкой слоя данных.
fn map_status(status: Status) -> DataError {
    match status.code() {
        Code::ResourceExhausted => DataError::RateLimited("grpc"),
        Code::Unauthenticated => DataError::Auth(status.message().to_string()),
        Code::Unavailable => DataError::Transport(status.message().to_string()),
        other => DataError::Other(format!("{other:?}: {}", status.message())),
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl MarketData for FinamClient {
    async fn assets(&self, mic: &str) -> Result<Vec<Instrument>, DataError> {
        // `AllAssets` — постраничный (курсор по sec_id); `Assets` устарел.
        let mut out = Vec::new();
        let mut cursor = 0i64;
        loop {
            self.limiters.assets.acquire().await;
            let token = self.ensure_token().await?;
            let request = authorize(
                &token,
                assets::AllAssetsRequest {
                    cursor,
                    only_active: true,
                    only_disabled: false,
                },
            )?;
            let response = self
                .assets
                .clone()
                .all_assets(request)
                .await
                .map_err(map_status)?
                .into_inner();
            out.extend(
                response
                    .assets
                    .iter()
                    .filter(|a| mic.is_empty() || a.mic.eq_ignore_ascii_case(mic))
                    .map(convert::instrument),
            );
            // Конец: пустая страница, нулевой или неизменившийся курсор.
            if response.assets.is_empty()
                || response.next_cursor == 0
                || response.next_cursor == cursor
            {
                break;
            }
            cursor = response.next_cursor;
        }
        tracing::debug!(count = out.len(), mic, "получены инструменты");
        Ok(out)
    }

    async fn bars(
        &self,
        symbol: &str,
        tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<Bar>, DataError> {
        self.limiters.bars.acquire().await;
        let token = self.ensure_token().await?;
        let request = authorize(
            &token,
            marketdata::BarsRequest {
                symbol: symbol.to_string(),
                timeframe: convert::timeframe(tf) as i32,
                interval: Some(convert::interval(from_ts, to_ts)),
            },
        )?;
        let response = self
            .market
            .clone()
            .bars(request)
            .await
            .map_err(map_status)?
            .into_inner();
        Ok(response.bars.iter().map(convert::bar).collect())
    }

    async fn last_quote(&self, symbol: &str) -> Result<Quote, DataError> {
        self.limiters.quote.acquire().await;
        let token = self.ensure_token().await?;
        let request = authorize(
            &token,
            marketdata::QuoteRequest {
                symbol: symbol.to_string(),
            },
        )?;
        let response = self
            .market
            .clone()
            .last_quote(request)
            .await
            .map_err(map_status)?
            .into_inner();
        response
            .quote
            .as_ref()
            .map(convert::quote)
            .ok_or_else(|| DataError::Other("пустая котировка".into()))
    }

    async fn latest_trades(&self, symbol: &str) -> Result<Vec<Trade>, DataError> {
        self.limiters.trades.acquire().await;
        let token = self.ensure_token().await?;
        let request = authorize(
            &token,
            marketdata::LatestTradesRequest {
                symbol: symbol.to_string(),
            },
        )?;
        let response = self
            .market
            .clone()
            .latest_trades(request)
            .await
            .map_err(map_status)?
            .into_inner();
        Ok(response.trades.iter().map(convert::trade).collect())
    }
}
