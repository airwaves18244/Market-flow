//! gRPC-клиент Finam Trade API: транспорт, авторизация, rate-limit, стримы
//! (§ 0.2–0.4).
//!
//! [`FinamClient`] держит tonic-клиентов поверх общего TLS-канала, обновляет JWT
//! через [`AuthManager`] и реализует трейт [`MarketData`] (унарные методы), а
//! также отдаёт переподключаемые стримы рыночных данных.
//!
//! Сетевые вызовы здесь не покрываются юнит-тестами (нет ключа/доступа к API);
//! проверяемая логика вынесена в `convert`, `auth`, `ratelimit`, `resilience`,
//! `stream`.

use std::sync::Arc;

use tokio_stream::{Stream, StreamExt};
use tonic::transport::{Channel, ClientTlsConfig};

use finam_proto::{
    assets, marketdata, AssetsServiceClient, AuthServiceClient, MarketDataServiceClient, ENDPOINT,
};

use crate::auth::AuthManager;
use crate::ratelimit::Limiters;
use crate::stream::reconnecting;
use crate::{authorize, convert, map_status, DataError, MarketData, TimeFrame};
use domain::{Bar, Instrument, Quote, Trade};

/// Клиент Finam Trade API (read-only).
pub struct FinamClient {
    auth: Arc<AuthManager>,
    assets: AssetsServiceClient<Channel>,
    market: MarketDataServiceClient<Channel>,
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

        let auth = Arc::new(AuthManager::new(
            AuthServiceClient::new(channel.clone()),
            secret,
            source_app_id,
        ));
        // Первичная авторизация — наружу отдаём готовый к работе клиент.
        auth.refresh().await?;

        Ok(Self {
            auth,
            assets: AssetsServiceClient::new(channel.clone()),
            market: MarketDataServiceClient::new(channel),
            limiters: Limiters::default(),
        })
    }

    // --- Стримы рыночных данных (§ 0.3) с авто-reconnect -------------------

    /// Подписка на сделки по инструменту. Стрим автоматически переподключается.
    pub fn subscribe_trades(
        &self,
        symbol: &str,
    ) -> impl Stream<Item = Result<Vec<Trade>, DataError>> + '_ {
        let market = self.market.clone();
        let auth = self.auth.clone();
        let symbol = symbol.to_string();
        reconnecting(move |_attempt| {
            let mut market = market.clone();
            let auth = auth.clone();
            let symbol = symbol.clone();
            async move {
                let token = auth.token().await?;
                let request =
                    authorize(&token, marketdata::SubscribeLatestTradesRequest { symbol })?;
                let inner = market
                    .subscribe_latest_trades(request)
                    .await
                    .map_err(map_status)?
                    .into_inner();
                Ok(inner.map(|item| {
                    item.map_err(map_status)
                        .map(|resp| resp.trades.iter().map(convert::trade).collect())
                }))
            }
        })
    }

    /// Подписка на агрегированные свечи по инструменту (с авто-reconnect).
    pub fn subscribe_bars(
        &self,
        symbol: &str,
        tf: TimeFrame,
    ) -> impl Stream<Item = Result<Vec<Bar>, DataError>> + '_ {
        let market = self.market.clone();
        let auth = self.auth.clone();
        let symbol = symbol.to_string();
        reconnecting(move |_attempt| {
            let mut market = market.clone();
            let auth = auth.clone();
            let symbol = symbol.clone();
            async move {
                let token = auth.token().await?;
                let request = authorize(
                    &token,
                    marketdata::SubscribeBarsRequest {
                        symbol,
                        timeframe: convert::timeframe(tf) as i32,
                    },
                )?;
                let inner = market
                    .subscribe_bars(request)
                    .await
                    .map_err(map_status)?
                    .into_inner();
                Ok(inner.map(|item| {
                    item.map_err(map_status)
                        .map(|resp| resp.bars.iter().map(convert::bar).collect())
                }))
            }
        })
    }

    /// Подписка на котировки по набору инструментов (с авто-reconnect).
    /// Стрим-ошибка сервиса (`StreamError`) пробрасывается как [`DataError`].
    pub fn subscribe_quotes(
        &self,
        symbols: &[String],
    ) -> impl Stream<Item = Result<Vec<Quote>, DataError>> + '_ {
        let market = self.market.clone();
        let auth = self.auth.clone();
        let symbols = symbols.to_vec();
        reconnecting(move |_attempt| {
            let mut market = market.clone();
            let auth = auth.clone();
            let symbols = symbols.clone();
            async move {
                let token = auth.token().await?;
                let request = authorize(&token, marketdata::SubscribeQuoteRequest { symbols })?;
                let inner = market
                    .subscribe_quote(request)
                    .await
                    .map_err(map_status)?
                    .into_inner();
                Ok(inner.map(|item| {
                    item.map_err(map_status).and_then(|resp| match resp.error {
                        Some(e) if e.code != 0 => {
                            Err(DataError::Other(format!("стрим {}: {}", e.code, e.description)))
                        }
                        _ => Ok(resp.quote.iter().map(convert::quote).collect()),
                    })
                }))
            }
        })
    }
}

impl MarketData for FinamClient {
    async fn assets(&self, mic: &str) -> Result<Vec<Instrument>, DataError> {
        // `AllAssets` — постраничный (курсор по sec_id); `Assets` устарел.
        let mut out = Vec::new();
        let mut cursor = 0i64;
        loop {
            self.limiters.assets.acquire().await;
            let token = self.auth.token().await?;
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
        let token = self.auth.token().await?;
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
        let token = self.auth.token().await?;
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
        let token = self.auth.token().await?;
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
