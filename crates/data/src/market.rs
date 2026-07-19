//! gRPC-реализация источника рыночных данных (фича `grpc`).
//!
//! [`FinamMarketData`] реализует трейт [`MarketData`](crate::MarketData) поверх
//! сгенерированных стабов `AssetsService`/`MarketDataService`. Каждый вызов:
//! берёт действующий JWT у [`AuthManager`](crate::AuthManager) и кладёт его в
//! метаданные `authorization`, держит per-method лимит ([`RateLimiter`]),
//! повторяет транзиентные сбои с [`Backoff`] и переводит протобаф-типы в чистые
//! доменные ([`Instrument`]/[`Bar`]/[`Quote`]/[`Trade`]).
//!
//! Маппинг (Decimal/Timestamp/Side → доменные значения) вынесен в чистые
//! функции и покрыт тестами; сами сетевые вызовы интеграционно проверяются при
//! наличии реального секрета (в CI выключено).

use std::time::Duration;

use domain::{AssetClass, Bar, BookLevel, Instrument, OrderBook, Quote, TimeFrame, Trade};
use prost_types::Timestamp;

use finam_proto::pb::google::r#type::{Decimal, Interval};
use finam_proto::pb::grpc::tradeapi::v1::Side;

use crate::backoff::jitter_fraction;
use crate::grpc::AuthManager;
use crate::{AuthTransport, Backoff, DataError, MarketData, Method, RateLimiter, SecretStore};

/// gRPC-источник рыночных данных Finam.
///
/// Параметризован транспортом авторизации `T` и хранилищем секрета `S`
/// (через [`AuthManager`]); канал к gRPC-эндпоинту переиспользуется между
/// вызовами (ленивое подключение).
pub struct FinamMarketData<T: AuthTransport, S: SecretStore> {
    pub(crate) auth: AuthManager<T, S>,
    pub(crate) channel: tonic::transport::Channel,
    limiter: RateLimiter,
    backoff: Backoff,
}

impl<T: AuthTransport, S: SecretStore> FinamMarketData<T, S> {
    /// Подключиться к стандартному эндпоинту Finam ([`finam_proto::ENDPOINT`]).
    pub fn connect(auth: AuthManager<T, S>) -> Result<Self, DataError> {
        Self::connect_to(finam_proto::ENDPOINT, auth)
    }

    /// Подключиться к произвольному эндпоинту (стенд/прокси). Для `https`
    /// включается TLS с системными корневыми сертификатами.
    pub fn connect_to(endpoint: &str, auth: AuthManager<T, S>) -> Result<Self, DataError> {
        Ok(Self {
            auth,
            channel: build_endpoint(endpoint)?.connect_lazy(),
            limiter: RateLimiter::finam_default(),
            backoff: Backoff::finam_default(),
        })
    }

    /// Заменить политики лимитера/backoff (например, в тестах).
    pub fn with_policy(mut self, limiter: RateLimiter, backoff: Backoff) -> Self {
        self.limiter = limiter;
        self.backoff = backoff;
        self
    }

    async fn sleep_for(&self, attempt: u32) {
        let delay = self.backoff.delay_with_jitter(attempt, jitter_fraction());
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
    }
}

/// Выполнить gRPC-вызов с учётом лимита метода, повторов транзиентных сбоев и
/// однократного re-auth при отказе авторизации.
///
/// `$body` — `async`-блок одной попытки; он берётся по месту (без замыкания),
/// поэтому свободно заимствует `self` без проблем с лайфтаймами.
///
/// `DataError::Auth` сам по себе не ретраябелен ([`Backoff`]): обычно это
/// «неверный секрет». Но сервер может инвалидировать выданный JWT до истечения
/// skew-окна — тогда `access_token()` продолжает отдавать кэш, и без сброса
/// вызовы отказывают до ~14 минут. Поэтому на первый `Auth` делаем форс-refresh
/// токена и повторяем вызов ровно один раз (флаг `reauthed` исключает
/// бесконечный цикл re-auth). Второй `Auth` подряд — реальный отказ, отдаём его.
macro_rules! call_with_retry {
    ($self:expr, $method:expr, $body:expr) => {{
        let mut attempt = 0u32;
        let mut reauthed = false;
        loop {
            if let Err(e) = $self.limiter.try_acquire($method) {
                if $self.backoff.is_exhausted(attempt) {
                    break Err(e);
                }
                $self.sleep_for(attempt).await;
                attempt += 1;
                continue;
            }
            match $body.await {
                Ok(v) => break Ok(v),
                // Однократный форс-refresh при отказе авторизации: сбрасываем
                // кэш токена и повторяем без задержки и без расхода попыток
                // backoff (это не транзиентный сбой, а протухший токен).
                Err(DataError::Auth(_)) if !reauthed => {
                    reauthed = true;
                    if let Err(e) = $self.auth.force_refresh().await {
                        break Err(e);
                    }
                }
                Err(e) if e.is_retryable() && !$self.backoff.is_exhausted(attempt) => {
                    $self.sleep_for(attempt).await;
                    attempt += 1;
                }
                Err(e) => break Err(e),
            }
        }
    }};
}

impl<T, S> MarketData for FinamMarketData<T, S>
where
    T: AuthTransport,
    S: SecretStore + Send + Sync,
{
    async fn assets(&self, mic: &str) -> Result<Vec<Instrument>, DataError> {
        use finam_proto::assets::assets_service_client::AssetsServiceClient;
        use finam_proto::assets::AssetsRequest;

        call_with_retry!(self, Method::Assets, async {
            let token = self.auth.access_token().await?;
            let mut client = AssetsServiceClient::new(self.channel.clone());
            let mut request = tonic::Request::new(AssetsRequest {});
            prepare_unary(&mut request, &token)?;
            let resp = client
                .assets(request)
                .await
                .map_err(status_to_error)?
                .into_inner();
            let instruments = resp
                .assets
                .into_iter()
                .filter(|a| mic.is_empty() || a.mic == mic)
                .filter_map(map_asset)
                .collect();
            Ok::<_, DataError>(instruments)
        })
    }

    async fn bars(
        &self,
        symbol: &str,
        tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<Bar>, DataError> {
        use finam_proto::marketdata::market_data_service_client::MarketDataServiceClient;
        use finam_proto::marketdata::BarsRequest;

        call_with_retry!(self, Method::Bars, async {
            let token = self.auth.access_token().await?;
            let mut client = MarketDataServiceClient::new(self.channel.clone());
            let mut request = tonic::Request::new(BarsRequest {
                symbol: symbol.to_owned(),
                timeframe: timeframe_to_proto(tf),
                interval: Some(Interval {
                    start_time: Some(secs_to_ts(from_ts)),
                    end_time: Some(secs_to_ts(to_ts)),
                }),
            });
            prepare_unary(&mut request, &token)?;
            let resp = client
                .bars(request)
                .await
                .map_err(status_to_error)?
                .into_inner();
            Ok::<_, DataError>(resp.bars.iter().map(map_bar).collect())
        })
    }

    async fn last_quote(&self, symbol: &str) -> Result<Quote, DataError> {
        use finam_proto::marketdata::market_data_service_client::MarketDataServiceClient;
        use finam_proto::marketdata::QuoteRequest;

        call_with_retry!(self, Method::LastQuote, async {
            let token = self.auth.access_token().await?;
            let mut client = MarketDataServiceClient::new(self.channel.clone());
            let mut request = tonic::Request::new(QuoteRequest {
                symbol: symbol.to_owned(),
            });
            prepare_unary(&mut request, &token)?;
            let resp = client
                .last_quote(request)
                .await
                .map_err(status_to_error)?
                .into_inner();
            resp.quote
                .as_ref()
                .map(map_quote)
                .ok_or_else(|| DataError::Other("пустая котировка в ответе".to_owned()))
        })
    }

    async fn latest_trades(&self, symbol: &str) -> Result<Vec<Trade>, DataError> {
        use finam_proto::marketdata::market_data_service_client::MarketDataServiceClient;
        use finam_proto::marketdata::LatestTradesRequest;

        call_with_retry!(self, Method::LatestTrades, async {
            let token = self.auth.access_token().await?;
            let mut client = MarketDataServiceClient::new(self.channel.clone());
            let mut request = tonic::Request::new(LatestTradesRequest {
                symbol: symbol.to_owned(),
            });
            prepare_unary(&mut request, &token)?;
            let resp = client
                .latest_trades(request)
                .await
                .map_err(status_to_error)?
                .into_inner();
            Ok::<_, DataError>(resp.trades.iter().map(map_trade).collect())
        })
    }
}

impl<T, S> FinamMarketData<T, S>
where
    T: AuthTransport,
    S: SecretStore + Send + Sync,
{
    /// Текущий стакан (DOM) по инструменту (`MarketDataService.OrderBook`).
    pub async fn order_book(&self, symbol: &str) -> Result<OrderBook, DataError> {
        use finam_proto::marketdata::market_data_service_client::MarketDataServiceClient;
        use finam_proto::marketdata::OrderBookRequest;

        call_with_retry!(self, Method::OrderBook, async {
            let token = self.auth.access_token().await?;
            let mut client = MarketDataServiceClient::new(self.channel.clone());
            let mut request = tonic::Request::new(OrderBookRequest {
                symbol: symbol.to_owned(),
            });
            prepare_unary(&mut request, &token)?;
            let resp = client
                .order_book(request)
                .await
                .map_err(status_to_error)?
                .into_inner();
            resp.orderbook
                .as_ref()
                .map(map_order_book)
                .ok_or_else(|| DataError::Other("пустой стакан в ответе".to_owned()))
        })
    }
}

/// Предел на установление TCP/TLS-соединения канала. Отсекает «висящий»
/// коннект (в т.ч. полуоткрытый TCP) на этапе установления — иначе первый
/// вызов по ленивому каналу мог ждать бесконечно.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Дедлайн одного unary-вызова (от отправки до ответа). Ставится per-request
/// через [`tonic::Request::set_timeout`], а НЕ на весь канал: канал общий с
/// server-streaming подписками (`stream.rs`), где таймаут на весь стрим убил бы
/// долгоживущий поток. Полуоткрытое уже установленное соединение без этого
/// дедлайна повесило бы `bars().await` навечно (ретраи backoff не срабатывают,
/// пока будущее не завершилось).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Сконфигурировать gRPC-эндпоинт: для `https` включить TLS с системными
/// корневыми сертификатами, ограничить время установления соединения. Единый
/// билдер для auth-транспорта и клиента данных.
pub(crate) fn build_endpoint(url: &str) -> Result<tonic::transport::Endpoint, DataError> {
    let mut ep = tonic::transport::Channel::from_shared(url.to_owned())
        .map_err(|e| DataError::Transport(format!("неверный эндпоинт: {e}")))?
        // Таймаут только на коннект — безопасен и для стримов (ограничивает
        // установление, но не длительность самого стрима).
        .connect_timeout(CONNECT_TIMEOUT);
    if url.starts_with("https") {
        let tls = tonic::transport::ClientTlsConfig::new().with_native_roots();
        ep = ep
            .tls_config(tls)
            .map_err(|e| DataError::Transport(format!("tls: {e}")))?;
    }
    Ok(ep)
}

/// Положить JWT в метаданные `authorization` (Finam ждёт «голый» токен).
pub(crate) fn attach_auth<M>(
    request: &mut tonic::Request<M>,
    token: &str,
) -> Result<(), DataError> {
    let value = token
        .parse()
        .map_err(|_| DataError::Auth("токен не пригоден для HTTP-заголовка".to_owned()))?;
    request.metadata_mut().insert("authorization", value);
    Ok(())
}

/// Подготовить unary-запрос: JWT в метаданные + per-request дедлайн
/// ([`REQUEST_TIMEOUT`]). Для server-streaming подписок дедлайн НЕ ставится
/// (там используется голый [`attach_auth`]).
pub(crate) fn prepare_unary<M>(
    request: &mut tonic::Request<M>,
    token: &str,
) -> Result<(), DataError> {
    attach_auth(request, token)?;
    request.set_timeout(REQUEST_TIMEOUT);
    Ok(())
}

/// Маппинг `tonic::Status` в [`DataError`] с учётом ретраябельности.
pub(crate) fn status_to_error(status: tonic::Status) -> DataError {
    use tonic::Code;
    match status.code() {
        Code::Unauthenticated | Code::PermissionDenied => {
            DataError::Auth(status.message().to_owned())
        }
        Code::Unavailable | Code::DeadlineExceeded | Code::Aborted | Code::ResourceExhausted => {
            DataError::Transport(format!("{}: {}", status.code(), status.message()))
        }
        other => DataError::Other(format!("{}: {}", other, status.message())),
    }
}

// --- Чистые помощники маппинга (тестируются без сети) ---

/// `google.type.Decimal` (строка) → `f64`. Пусто/None/нечисло → `0.0`.
fn decimal_to_f64(d: Option<&Decimal>) -> f64 {
    d.map(|d| d.value.trim())
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0)
}

/// `google.protobuf.Timestamp` → UNIX-секунды UTC (`i64`). None → `0`.
fn ts_to_secs(t: Option<&Timestamp>) -> i64 {
    t.map(|t| t.seconds).unwrap_or(0)
}

/// UNIX-секунды UTC → `google.protobuf.Timestamp` (наносекунды = 0).
fn secs_to_ts(secs: i64) -> Timestamp {
    Timestamp {
        seconds: secs,
        nanos: 0,
    }
}

/// Доменный тайм-фрейм → числовой код enum `TimeFrame` из proto.
pub(crate) fn timeframe_to_proto(tf: TimeFrame) -> i32 {
    // Значения соответствуют enum TimeFrame в marketdata_service.proto.
    match tf {
        TimeFrame::M1 => 1,
        TimeFrame::M5 => 5,
        TimeFrame::M15 => 9,
        TimeFrame::H1 => 12,
        TimeFrame::D1 => 19,
    }
}

/// Тип инструмента Finam (строка) → доменный класс актива.
///
/// `None` для классов вне интереса терминала (валюты, индексы, опционы и т.п.) —
/// такие инструменты отбрасываются из списка.
fn asset_class_from_type(raw: &str) -> Option<AssetClass> {
    let t = raw.to_ascii_uppercase();
    if t.contains("FUTUR") {
        Some(AssetClass::Future)
    } else if t.contains("BOND") {
        Some(AssetClass::Bond)
    } else if t.contains("EQUIT") || t.contains("STOCK") || t.contains("SHARE") || t.contains("ETF")
    {
        Some(AssetClass::Equity)
    } else {
        None
    }
}

/// proto `Asset` → доменный [`Instrument`]; `None`, если класс не распознан.
///
/// Список инструментов не несёт размер лота/сектор — лот по умолчанию `1`,
/// сектор заполняется позже из таблицы классификации (`SectorMap`).
fn map_asset(a: finam_proto::assets::Asset) -> Option<Instrument> {
    let asset_class = asset_class_from_type(&a.r#type)?;
    Some(Instrument {
        symbol: a.symbol,
        ticker: a.ticker,
        name: a.name,
        asset_class,
        sector: None,
        lot_size: 1,
        isin: Some(a.isin).filter(|s| !s.is_empty()),
    })
}

/// proto `Bar` → доменный [`Bar`].
pub(crate) fn map_bar(b: &finam_proto::marketdata::Bar) -> Bar {
    Bar {
        ts: ts_to_secs(b.timestamp.as_ref()),
        open: decimal_to_f64(b.open.as_ref()),
        high: decimal_to_f64(b.high.as_ref()),
        low: decimal_to_f64(b.low.as_ref()),
        close: decimal_to_f64(b.close.as_ref()),
        volume: decimal_to_f64(b.volume.as_ref()),
    }
}

/// proto `Quote` → доменный [`Quote`].
pub(crate) fn map_quote(q: &finam_proto::marketdata::Quote) -> Quote {
    Quote {
        ts: ts_to_secs(q.timestamp.as_ref()),
        last: decimal_to_f64(q.last.as_ref()),
        bid: decimal_to_f64(q.bid.as_ref()),
        ask: decimal_to_f64(q.ask.as_ref()),
        volume: decimal_to_f64(q.volume.as_ref()),
    }
}

/// proto `Trade` → доменный [`Trade`]. Сторона: BUY → инициирована покупателем.
pub(crate) fn map_trade(t: &finam_proto::marketdata::Trade) -> Trade {
    let buyer_initiated = match t.side {
        x if x == Side::Buy as i32 => Some(true),
        x if x == Side::Sell as i32 => Some(false),
        _ => None,
    };
    Trade {
        ts: ts_to_secs(t.timestamp.as_ref()),
        price: decimal_to_f64(t.price.as_ref()),
        size: decimal_to_f64(t.size.as_ref()),
        buyer_initiated,
    }
}

/// proto `OrderBook` → доменный [`OrderBook`] (DOM).
///
/// Строки с `buy_size` идут в биды, с `sell_size` — в аски. Биды сортируются по
/// убыванию цены (лучший — первый), аски — по возрастанию. `ts` снимка — самая
/// поздняя метка среди строк.
pub(crate) fn map_order_book(ob: &finam_proto::marketdata::OrderBook) -> OrderBook {
    use finam_proto::marketdata::order_book::row::Side as RowSide;

    let mut bids = Vec::new();
    let mut asks = Vec::new();
    let mut ts = 0i64;
    for row in &ob.rows {
        ts = ts.max(ts_to_secs(row.timestamp.as_ref()));
        let price = decimal_to_f64(row.price.as_ref());
        match row.side.as_ref() {
            Some(RowSide::BuySize(size)) => bids.push(BookLevel {
                price,
                size: decimal_to_f64(Some(size)),
            }),
            Some(RowSide::SellSize(size)) => asks.push(BookLevel {
                price,
                size: decimal_to_f64(Some(size)),
            }),
            None => {}
        }
    }
    // Лучший бид — наивысшая цена; лучший аск — наименьшая.
    bids.sort_by(|a, b| b.price.total_cmp(&a.price));
    asks.sort_by(|a, b| a.price.total_cmp(&b.price));
    OrderBook { ts, bids, asks }
}

/// proto `StreamOrderBook` (сообщение стрима `SubscribeOrderBook`) → доменный
/// [`OrderBook`].
///
/// Тип `StreamOrderBook` структурно повторяет разовый `OrderBook`, но это
/// отдельный сгенерированный тип, поэтому нужен свой маппер. Поле `action`
/// (ADD/REMOVE/UPDATE) здесь **сознательно игнорируется**: каждое сообщение
/// стрима трактуется как самодостаточный снимок сторон — ровно так же, как
/// разовый [`map_order_book`] уже игнорирует `action`. Это сохраняет единый
/// смысл «стакан = снимок» во всём коде; полноценная инкрементальная сборка
/// книги по дельтам — предмет отдельной задачи, если Finam будет слать именно
/// дельты, а не периодические снимки.
pub(crate) fn map_stream_order_book(ob: &finam_proto::marketdata::StreamOrderBook) -> OrderBook {
    use finam_proto::marketdata::stream_order_book::row::Side as RowSide;

    let mut bids = Vec::new();
    let mut asks = Vec::new();
    let mut ts = 0i64;
    for row in &ob.rows {
        ts = ts.max(ts_to_secs(row.timestamp.as_ref()));
        let price = decimal_to_f64(row.price.as_ref());
        match row.side.as_ref() {
            Some(RowSide::BuySize(size)) => bids.push(BookLevel {
                price,
                size: decimal_to_f64(Some(size)),
            }),
            Some(RowSide::SellSize(size)) => asks.push(BookLevel {
                price,
                size: decimal_to_f64(Some(size)),
            }),
            None => {}
        }
    }
    bids.sort_by(|a, b| b.price.total_cmp(&a.price));
    asks.sort_by(|a, b| a.price.total_cmp(&b.price));
    OrderBook { ts, bids, asks }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finam_proto::marketdata::{Bar as PbBar, Quote as PbQuote, Trade as PbTrade};

    fn dec(v: &str) -> Option<Decimal> {
        Some(Decimal {
            value: v.to_owned(),
        })
    }

    #[test]
    fn decimal_parsing_handles_empty_and_garbage() {
        assert_eq!(decimal_to_f64(dec("12.5").as_ref()), 12.5);
        assert_eq!(decimal_to_f64(dec("  -3 ").as_ref()), -3.0);
        assert_eq!(decimal_to_f64(dec("").as_ref()), 0.0);
        assert_eq!(decimal_to_f64(dec("nan-ish").as_ref()), 0.0);
        assert_eq!(decimal_to_f64(None), 0.0);
    }

    #[test]
    fn timestamp_roundtrips_seconds() {
        assert_eq!(ts_to_secs(Some(&secs_to_ts(1_700_000_000))), 1_700_000_000);
        assert_eq!(ts_to_secs(None), 0);
        assert_eq!(secs_to_ts(42).nanos, 0);
    }

    #[test]
    fn timeframe_codes_match_proto() {
        assert_eq!(timeframe_to_proto(TimeFrame::M1), 1);
        assert_eq!(timeframe_to_proto(TimeFrame::M5), 5);
        assert_eq!(timeframe_to_proto(TimeFrame::M15), 9);
        assert_eq!(timeframe_to_proto(TimeFrame::H1), 12);
        assert_eq!(timeframe_to_proto(TimeFrame::D1), 19);
    }

    #[test]
    fn asset_class_classification() {
        assert_eq!(asset_class_from_type("EQUITIES"), Some(AssetClass::Equity));
        assert_eq!(asset_class_from_type("ETF"), Some(AssetClass::Equity));
        assert_eq!(asset_class_from_type("FUTURES"), Some(AssetClass::Future));
        assert_eq!(asset_class_from_type("BONDS"), Some(AssetClass::Bond));
        // Вне интереса терминала.
        assert_eq!(asset_class_from_type("CURRENCY"), None);
        assert_eq!(asset_class_from_type("INDICES"), None);
    }

    #[test]
    fn map_asset_skips_unknown_and_normalizes_isin() {
        let known = finam_proto::assets::Asset {
            symbol: "SBER@MISX".into(),
            id: "1".into(),
            ticker: "SBER".into(),
            mic: "MISX".into(),
            isin: "RU0009029540".into(),
            r#type: "EQUITIES".into(),
            name: "Сбербанк".into(),
            is_archived: false,
        };
        let inst = map_asset(known).unwrap();
        assert_eq!(inst.symbol, "SBER@MISX");
        assert_eq!(inst.asset_class, AssetClass::Equity);
        assert_eq!(inst.isin.as_deref(), Some("RU0009029540"));
        assert_eq!(inst.lot_size, 1);
        assert_eq!(inst.sector, None);

        let unknown = finam_proto::assets::Asset {
            r#type: "CURRENCY".into(),
            isin: String::new(),
            ..Default::default()
        };
        assert!(map_asset(unknown).is_none());
    }

    #[test]
    fn map_bar_converts_decimals_and_time() {
        let pb = PbBar {
            timestamp: Some(secs_to_ts(1_700_000_000)),
            open: dec("100.0"),
            high: dec("110.0"),
            low: dec("90.0"),
            close: dec("105.0"),
            volume: dec("1234"),
        };
        let b = map_bar(&pb);
        assert_eq!(b.ts, 1_700_000_000);
        assert_eq!(
            (b.open, b.high, b.low, b.close, b.volume),
            (100.0, 110.0, 90.0, 105.0, 1234.0)
        );
    }

    #[test]
    fn map_quote_picks_core_fields() {
        let pb = PbQuote {
            symbol: "SBER@MISX".into(),
            timestamp: Some(secs_to_ts(10)),
            last: dec("105.5"),
            bid: dec("105.4"),
            ask: dec("105.6"),
            volume: dec("9999"),
            ..Default::default()
        };
        let q = map_quote(&pb);
        assert_eq!(q.ts, 10);
        assert_eq!(
            (q.last, q.bid, q.ask, q.volume),
            (105.5, 105.4, 105.6, 9999.0)
        );
    }

    #[test]
    fn map_trade_maps_side_to_initiator() {
        let buy = PbTrade {
            timestamp: Some(secs_to_ts(5)),
            price: dec("10"),
            size: dec("2"),
            side: Side::Buy as i32,
            ..Default::default()
        };
        let t = map_trade(&buy);
        assert_eq!((t.ts, t.price, t.size), (5, 10.0, 2.0));
        assert_eq!(t.buyer_initiated, Some(true));

        let sell = PbTrade {
            side: Side::Sell as i32,
            ..buy.clone()
        };
        assert_eq!(map_trade(&sell).buyer_initiated, Some(false));

        let unk = PbTrade {
            side: Side::Unspecified as i32,
            ..buy.clone()
        };
        assert_eq!(map_trade(&unk).buyer_initiated, None);
    }

    #[test]
    fn map_order_book_splits_and_sorts_sides() {
        use finam_proto::marketdata::order_book::{row::Side as RowSide, Row};
        use finam_proto::marketdata::OrderBook as PbOrderBook;

        let row = |price: &str, side: RowSide, ts: i64| Row {
            price: dec(price),
            side: Some(side),
            action: 0,
            mpid: String::new(),
            timestamp: Some(secs_to_ts(ts)),
        };
        let ob = PbOrderBook {
            rows: vec![
                row("100.0", RowSide::BuySize(Decimal { value: "5".into() }), 10),
                row(
                    "101.0",
                    RowSide::SellSize(Decimal { value: "7".into() }),
                    12,
                ),
                row("99.5", RowSide::BuySize(Decimal { value: "3".into() }), 11),
                row("102.0", RowSide::SellSize(Decimal { value: "2".into() }), 9),
            ],
        };
        let dom = map_order_book(&ob);
        assert_eq!(dom.ts, 12); // самая поздняя метка
                                // Биды по убыванию цены: 100.0 затем 99.5.
        assert_eq!(
            dom.bids.iter().map(|l| l.price).collect::<Vec<_>>(),
            [100.0, 99.5]
        );
        // Аски по возрастанию цены: 101.0 затем 102.0.
        assert_eq!(
            dom.asks.iter().map(|l| l.price).collect::<Vec<_>>(),
            [101.0, 102.0]
        );
        assert_eq!(dom.best_bid().unwrap().size, 5.0);
        assert_eq!(dom.spread(), Some(1.0)); // 101.0 - 100.0
    }

    #[test]
    fn map_stream_order_book_splits_and_sorts_sides() {
        use finam_proto::marketdata::stream_order_book::{row::Side as RowSide, Row};
        use finam_proto::marketdata::StreamOrderBook;

        let row = |price: &str, side: RowSide, ts: i64| Row {
            price: dec(price),
            side: Some(side),
            action: 0,
            mpid: String::new(),
            timestamp: Some(secs_to_ts(ts)),
        };
        let ob = StreamOrderBook {
            symbol: "SBER@MISX".into(),
            rows: vec![
                row("100.0", RowSide::BuySize(Decimal { value: "5".into() }), 10),
                row(
                    "101.0",
                    RowSide::SellSize(Decimal { value: "7".into() }),
                    12,
                ),
                row("99.5", RowSide::BuySize(Decimal { value: "3".into() }), 11),
            ],
        };
        let dom = map_stream_order_book(&ob);
        assert_eq!(dom.ts, 12);
        assert_eq!(
            dom.bids.iter().map(|l| l.price).collect::<Vec<_>>(),
            [100.0, 99.5]
        );
        assert_eq!(dom.asks.iter().map(|l| l.price).collect::<Vec<_>>(), [101.0]);
    }

    // --- R-1: таймауты канала/запроса ---

    #[test]
    fn endpoint_builds_with_sane_timeouts() {
        // Константы дедлайнов заданы и не вырождены (иначе полуоткрытый TCP
        // повесил бы вызов навечно). Значения самого Endpoint приватны у tonic,
        // поэтому проверяем инварианты констант и что билдер строит канал.
        assert!(!CONNECT_TIMEOUT.is_zero());
        assert!(!REQUEST_TIMEOUT.is_zero());
        assert_eq!(CONNECT_TIMEOUT, Duration::from_secs(10));
        assert_eq!(REQUEST_TIMEOUT, Duration::from_secs(30));
        // Билдер строит эндпоинт и для http, и для https (TLS-ветка).
        assert!(build_endpoint("http://localhost:1").is_ok());
        assert!(build_endpoint("https://localhost:1").is_ok());
        assert!(build_endpoint("не-url").is_err());
    }

    // --- R-2: однократный re-auth при UNAUTHENTICATED ---

    use crate::{AuthManager, AuthToken, MemSecretStore, TokenState};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    /// Фейковый auth-транспорт: по программе отдаёт токены (последний
    /// повторяется, когда программа исчерпана), считая обмены через общий
    /// счётчик (доступ к нему у теста — приватное поле `transport` у
    /// `AuthManager` из другого модуля не видно).
    struct ReauthTransport {
        calls: Arc<AtomicU32>,
        program: Vec<AuthToken>,
    }

    impl ReauthTransport {
        fn new(calls: Arc<AtomicU32>, tokens: &[&str]) -> Self {
            Self {
                calls,
                program: tokens
                    .iter()
                    .map(|t| AuthToken {
                        token: (*t).to_owned(),
                        ttl: Duration::from_secs(900),
                    })
                    .collect(),
            }
        }
    }

    impl crate::AuthTransport for ReauthTransport {
        async fn authenticate(&self, _secret: &str) -> Result<AuthToken, DataError> {
            let i = self.calls.fetch_add(1, Ordering::SeqCst) as usize;
            Ok(self.program[i.min(self.program.len() - 1)].clone())
        }
    }

    /// Клиент данных поверх фейкового auth-транспорта; канал ленивый и в тесте
    /// не используется (тела вызовов подменены на локальный «сервер»). Возвращает
    /// клиент и общий счётчик обменов токена.
    fn fake_market(
        tokens: &[&str],
    ) -> (FinamMarketData<ReauthTransport, MemSecretStore>, Arc<AtomicU32>) {
        let calls = Arc::new(AtomicU32::new(0));
        // Backoff без задержек — тест не должен спать.
        let no_sleep = Backoff::new(Duration::ZERO, 1.0, Duration::ZERO, 5);
        let auth = AuthManager::with_policy(
            ReauthTransport::new(Arc::clone(&calls), tokens),
            MemSecretStore::with_secret("api-secret"),
            TokenState::new(),
            RateLimiter::finam_default(),
            no_sleep,
        );
        let fmd = FinamMarketData {
            auth,
            channel: build_endpoint("http://localhost:1").unwrap().connect_lazy(),
            limiter: RateLimiter::finam_default(),
            backoff: no_sleep,
        };
        (fmd, calls)
    }

    #[tokio::test]
    async fn reauth_retries_once_after_unauthenticated() {
        // Первый выданный токен сервер «инвалидировал» → отдаёт UNAUTHENTICATED;
        // после форс-refresh приходит новый токен и вызов проходит.
        let (fmd, calls) = fake_market(&["jwt-stale", "jwt-fresh"]);
        let hits = AtomicU32::new(0);

        let out: Result<&str, DataError> = call_with_retry!(fmd, Method::Bars, async {
            hits.fetch_add(1, Ordering::SeqCst);
            let token = fmd.auth.access_token().await?;
            if token == "jwt-stale" {
                // Сервер отклонил протухший токен.
                Err(DataError::Auth("сервер инвалидировал токен".to_owned()))
            } else {
                Ok::<_, DataError>("bars-ok")
            }
        });

        assert_eq!(out.unwrap(), "bars-ok");
        // Ровно два обмена токена: исходный + один форс-refresh.
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        // Два прохода тела: отказ, затем успех с новым токеном.
        assert_eq!(hits.load(Ordering::SeqCst), 2);
        assert!(fmd.auth.has_fresh_token());
    }

    #[tokio::test]
    async fn reauth_happens_at_most_once_no_infinite_loop() {
        // Сервер отвергает любой токен → после единственного форс-refresh
        // повторный отказ отдаётся наружу (без бесконечного цикла re-auth).
        let (fmd, calls) = fake_market(&["jwt-stale"]);
        let hits = AtomicU32::new(0);

        let out: Result<&str, DataError> = call_with_retry!(fmd, Method::Bars, async {
            hits.fetch_add(1, Ordering::SeqCst);
            let _token = fmd.auth.access_token().await?;
            Err::<&str, DataError>(DataError::Auth("всегда отказ".to_owned()))
        });

        assert!(matches!(out, Err(DataError::Auth(_))));
        // Один re-auth: исходный обмен + один форс-refresh.
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        // Тело выполнено дважды: исходный вызов и один повтор после re-auth.
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }
}
