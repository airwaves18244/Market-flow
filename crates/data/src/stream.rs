//! Live-стримы рыночных данных (фича `grpc`, Фаза 7).
//!
//! Подписки `MarketDataService.Subscribe*` — серверные стримы: клиент шлёт один
//! запрос, сервер непрерывно отдаёт обновления. Здесь — тонкие хэндлы стримов
//! ([`QuoteStream`]/[`TradeStream`]/[`BarStream`]) поверх `tonic::Streaming` с
//! переводом протобаф→домен, плюс чистый контроллер переподключения
//! ([`StreamReconnect`]): поток Finam обрывается ~раз в 24 ч, поэтому его нужно
//! переоткрывать с экспоненциальной паузой (сбрасываемой после успешных данных).
//!
//! Сам сетевой стрим интеграционно проверяется при наличии реального секрета;
//! здесь покрыты тестами чистые части — маппинг сообщений и политика повторов.

use std::time::Duration;

use domain::{Bar, OrderBook, Quote, TimeFrame, Trade};

use crate::market::{
    map_bar, map_quote, map_stream_order_book, map_trade, status_to_error, FinamMarketData,
};
use crate::{AuthTransport, Backoff, DataError, SecretStore};

use finam_proto::marketdata::{
    SubscribeBarsResponse, SubscribeLatestTradesResponse, SubscribeOrderBookResponse,
    SubscribeQuoteResponse,
};

/// Контроллер переподключения live-стрима.
///
/// Стримы переоткрываются бесконечно (в отличие от разовых запросов): задержка
/// растёт экспоненциально до потолка [`Backoff`] и сбрасывается [`reset`] после
/// успешно полученных данных. Счётчик попыток ограничен сверху, чтобы не
/// переполняться при долгих сериях обрывов (задержка и так ограничена потолком).
///
/// [`reset`]: Self::reset
#[derive(Debug, Clone)]
pub struct StreamReconnect {
    backoff: Backoff,
    attempt: u32,
    cap: u32,
}

impl StreamReconnect {
    /// Контроллер с заданной политикой backoff.
    pub fn new(backoff: Backoff) -> Self {
        Self {
            backoff,
            attempt: 0,
            cap: 16,
        }
    }

    /// Сбросить счётчик попыток после успешно полученных данных.
    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    /// Пауза перед следующей попыткой переподключения и сдвиг счётчика.
    ///
    /// `rand_fraction` ∈ `[0, 1)` — доля джиттера (источник случайности снаружи,
    /// расчёт остаётся детерминированным). Для стримов повторы не исчерпываются.
    pub fn next_delay(&mut self, rand_fraction: f64) -> Duration {
        let delay = self.backoff.delay_with_jitter(self.attempt, rand_fraction);
        if self.attempt < self.cap {
            self.attempt += 1;
        }
        delay
    }

    /// Текущий номер попытки (с 0).
    pub fn attempt(&self) -> u32 {
        self.attempt
    }
}

impl Default for StreamReconnect {
    fn default() -> Self {
        Self::new(Backoff::finam_default())
    }
}

/// Хэндл стрима котировок (`SubscribeQuote`).
pub struct QuoteStream {
    inner: tonic::Streaming<SubscribeQuoteResponse>,
}

impl QuoteStream {
    /// Следующая порция котировок. `Ok(None)` — стрим завершён (нужно
    /// переподключение). Сервисная ошибка стрима → [`DataError`].
    pub async fn next(&mut self) -> Result<Option<Vec<Quote>>, DataError> {
        match self.inner.message().await.map_err(status_to_error)? {
            Some(msg) => quotes_from_message(msg).map(Some),
            None => Ok(None),
        }
    }
}

/// Хэндл стрима сделок (`SubscribeLatestTrades`).
pub struct TradeStream {
    inner: tonic::Streaming<SubscribeLatestTradesResponse>,
}

impl TradeStream {
    /// Следующая порция сделок. `Ok(None)` — стрим завершён.
    pub async fn next(&mut self) -> Result<Option<Vec<Trade>>, DataError> {
        match self.inner.message().await.map_err(status_to_error)? {
            Some(msg) => Ok(Some(msg.trades.iter().map(map_trade).collect())),
            None => Ok(None),
        }
    }
}

/// Хэндл стрима стакана (`SubscribeOrderBook`).
pub struct OrderBookStream {
    inner: tonic::Streaming<SubscribeOrderBookResponse>,
}

impl OrderBookStream {
    /// Следующая порция снимков стакана. `Ok(None)` — стрим завершён (нужно
    /// переподключение). Одно сообщение стрима может нести несколько снимков
    /// (`repeated StreamOrderBook`); каждый трактуется как самодостаточный
    /// снимок сторон (см. [`map_stream_order_book`]) — потребитель обычно берёт
    /// последний как актуальный стакан.
    pub async fn next(&mut self) -> Result<Option<Vec<OrderBook>>, DataError> {
        match self.inner.message().await.map_err(status_to_error)? {
            Some(msg) => Ok(Some(
                msg.order_book.iter().map(map_stream_order_book).collect(),
            )),
            None => Ok(None),
        }
    }
}

/// Хэндл стрима свечей (`SubscribeBars`).
pub struct BarStream {
    inner: tonic::Streaming<SubscribeBarsResponse>,
}

impl BarStream {
    /// Следующая порция свечей. `Ok(None)` — стрим завершён.
    pub async fn next(&mut self) -> Result<Option<Vec<Bar>>, DataError> {
        match self.inner.message().await.map_err(status_to_error)? {
            Some(msg) => Ok(Some(msg.bars.iter().map(map_bar).collect())),
            None => Ok(None),
        }
    }
}

impl<T, S> FinamMarketData<T, S>
where
    T: AuthTransport,
    S: SecretStore + Send + Sync,
{
    /// Подписаться на котировки по списку символов.
    pub async fn subscribe_quotes(&self, symbols: &[String]) -> Result<QuoteStream, DataError> {
        use finam_proto::marketdata::market_data_service_client::MarketDataServiceClient;
        use finam_proto::marketdata::SubscribeQuoteRequest;

        let token = self.auth.access_token().await?;
        let mut client = MarketDataServiceClient::new(self.channel.clone());
        let mut request = tonic::Request::new(SubscribeQuoteRequest {
            symbols: symbols.to_vec(),
        });
        crate::market::attach_auth(&mut request, &token)?;
        let inner = client
            .subscribe_quote(request)
            .await
            .map_err(status_to_error)?
            .into_inner();
        Ok(QuoteStream { inner })
    }

    /// Подписаться на ленту сделок (Time&Sales) по инструменту.
    pub async fn subscribe_trades(&self, symbol: &str) -> Result<TradeStream, DataError> {
        use finam_proto::marketdata::market_data_service_client::MarketDataServiceClient;
        use finam_proto::marketdata::SubscribeLatestTradesRequest;

        let token = self.auth.access_token().await?;
        let mut client = MarketDataServiceClient::new(self.channel.clone());
        let mut request = tonic::Request::new(SubscribeLatestTradesRequest {
            symbol: symbol.to_owned(),
        });
        crate::market::attach_auth(&mut request, &token)?;
        let inner = client
            .subscribe_latest_trades(request)
            .await
            .map_err(status_to_error)?
            .into_inner();
        Ok(TradeStream { inner })
    }

    /// Подписаться на стакан (DOM) по инструменту.
    pub async fn subscribe_order_book(
        &self,
        symbol: &str,
    ) -> Result<OrderBookStream, DataError> {
        use finam_proto::marketdata::market_data_service_client::MarketDataServiceClient;
        use finam_proto::marketdata::SubscribeOrderBookRequest;

        let token = self.auth.access_token().await?;
        let mut client = MarketDataServiceClient::new(self.channel.clone());
        let mut request = tonic::Request::new(SubscribeOrderBookRequest {
            symbol: symbol.to_owned(),
        });
        crate::market::attach_auth(&mut request, &token)?;
        let inner = client
            .subscribe_order_book(request)
            .await
            .map_err(status_to_error)?
            .into_inner();
        Ok(OrderBookStream { inner })
    }

    /// Подписаться на агрегированные свечи по инструменту.
    pub async fn subscribe_bars(
        &self,
        symbol: &str,
        tf: TimeFrame,
    ) -> Result<BarStream, DataError> {
        use finam_proto::marketdata::market_data_service_client::MarketDataServiceClient;
        use finam_proto::marketdata::SubscribeBarsRequest;

        let token = self.auth.access_token().await?;
        let mut client = MarketDataServiceClient::new(self.channel.clone());
        let mut request = tonic::Request::new(SubscribeBarsRequest {
            symbol: symbol.to_owned(),
            timeframe: crate::market::timeframe_to_proto(tf),
        });
        crate::market::attach_auth(&mut request, &token)?;
        let inner = client
            .subscribe_bars(request)
            .await
            .map_err(status_to_error)?
            .into_inner();
        Ok(BarStream { inner })
    }
}

/// Котировки из сообщения стрима; сервисная ошибка (`StreamError`) → `DataError`.
fn quotes_from_message(msg: SubscribeQuoteResponse) -> Result<Vec<Quote>, DataError> {
    if let Some(err) = msg
        .error
        .filter(|e| e.code != 0 || !e.description.is_empty())
    {
        return Err(DataError::Transport(format!(
            "стрим котировок: {} (код {})",
            err.description, err.code
        )));
    }
    Ok(msg.quote.iter().map(map_quote).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finam_proto::marketdata::{Quote as PbQuote, StreamError};
    use finam_proto::pb::google::r#type::Decimal;

    fn dec(v: &str) -> Option<Decimal> {
        Some(Decimal {
            value: v.to_owned(),
        })
    }

    #[test]
    fn reconnect_grows_caps_and_resets() {
        // base 1с, ×2, потолок 8с — детерминированно через полный джиттер (1.0).
        let policy = Backoff::new(Duration::from_secs(1), 2.0, Duration::from_secs(8), 100);
        let mut r = StreamReconnect::new(policy);
        assert_eq!(r.next_delay(1.0), Duration::from_secs(1)); // attempt 0
        assert_eq!(r.next_delay(1.0), Duration::from_secs(2)); // 1
        assert_eq!(r.next_delay(1.0), Duration::from_secs(4)); // 2
        assert_eq!(r.next_delay(1.0), Duration::from_secs(8)); // 3
        assert_eq!(r.next_delay(1.0), Duration::from_secs(8)); // 4 — потолок
                                                               // После успешных данных счётчик сбрасывается.
        r.reset();
        assert_eq!(r.attempt(), 0);
        assert_eq!(r.next_delay(1.0), Duration::from_secs(1));
    }

    #[test]
    fn reconnect_attempt_counter_is_capped() {
        let mut r = StreamReconnect::default();
        for _ in 0..1_000 {
            let _ = r.next_delay(0.0);
        }
        // Счётчик не растёт безгранично.
        assert!(r.attempt() <= 16);
    }

    #[test]
    fn quotes_message_maps_payload() {
        let msg = SubscribeQuoteResponse {
            quote: vec![PbQuote {
                symbol: "SBER@MISX".into(),
                last: dec("105.5"),
                bid: dec("105.4"),
                ask: dec("105.6"),
                volume: dec("1000"),
                ..Default::default()
            }],
            error: None,
        };
        let quotes = quotes_from_message(msg).unwrap();
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].last, 105.5);
    }

    #[test]
    fn quotes_message_surfaces_stream_error() {
        let msg = SubscribeQuoteResponse {
            quote: Vec::new(),
            error: Some(StreamError {
                code: 7,
                description: "rate limited".into(),
            }),
        };
        let err = quotes_from_message(msg).unwrap_err();
        assert!(matches!(err, DataError::Transport(_)));
    }

    #[test]
    fn empty_stream_error_is_ignored() {
        // Пустой StreamError (код 0, без описания) — не ошибка.
        let msg = SubscribeQuoteResponse {
            quote: vec![PbQuote {
                last: dec("1"),
                ..Default::default()
            }],
            error: Some(StreamError {
                code: 0,
                description: String::new(),
            }),
        };
        assert_eq!(quotes_from_message(msg).unwrap().len(), 1);
    }
}
