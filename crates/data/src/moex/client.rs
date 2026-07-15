//! Клиент MOEX ALGOPACK: тонкая обёртка над [`HttpClient`] с Bearer-заголовком,
//! построением URL и склейкой курсорной пагинации.
//!
//! База и пути — по подтверждённому контракту (`SPEC_0-12.md`, `10.0.1`):
//! `https://apim.moex.com/iss/datashop/algopack/{market}/{dataset}.json`,
//! per-ticker — `.../{dataset}/{SECID}.json`. Параметры запроса (`from`,
//! `till`, `start`, `iss.meta=off`) и форма курсора (`(verify)`, `10.0.2`) —
//! не сверены живым ключом, см. `crates/data/tests/fixtures/moex/README.md`.
//!
//! `Authorization: Bearer <token>` добавляется на каждый запрос; клиент не
//! логирует и не выводит токен в `Debug` (см. [`MoexAlgo`] — поле `token`
//! приватное, ручная реализация `Debug` его маскирует).

use domain::algo::{FutoiPoint, Hi2Point, ObstatsPoint, OrderstatsPoint, SuperCandle};

use crate::http::{HttpClient, HttpTransport};
use crate::{DataError, Method};

use super::parse::{
    parse_futoi, parse_hi2, parse_obstats, parse_orderstats, parse_tradestats, IssCursor, IssTable,
};

/// Базовый URL ALGOPACK ISS (см. `10.0.1`).
pub const DEFAULT_BASE_URL: &str = "https://apim.moex.com/iss/datashop/algopack";

/// Верхний предел числа страниц одного запроса — защита от зацикливания при
/// некорректном/бесконечном курсоре сервера (не часть контракта API, только
/// защитный клапан клиента).
const MAX_PAGES: u32 = 1000;

/// Рынок ALGOPACK: `eq` (акции), `fo` (срочный), `fx` (валютный).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Market {
    /// Фондовый рынок (акции).
    Eq,
    /// Срочный рынок (фьючерсы/опционы) — единственный рынок `futoi`.
    Fo,
    /// Валютный рынок.
    Fx,
}

impl Market {
    /// Код рынка в пути URL.
    pub fn code(self) -> &'static str {
        match self {
            Market::Eq => "eq",
            Market::Fo => "fo",
            Market::Fx => "fx",
        }
    }

    /// Разбор рынка из кода (`eq`/`fo`/`fx`); `None` для неизвестного.
    pub fn from_code(code: &str) -> Option<Market> {
        match code {
            "eq" => Some(Market::Eq),
            "fo" => Some(Market::Fo),
            "fx" => Some(Market::Fx),
            _ => None,
        }
    }
}

impl std::fmt::Display for Market {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

/// Диапазон дат запроса (`YYYY-MM-DD`, оба конца необязательны — пустой
/// диапазон означает «последние доступные данные», как определяет сам API).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DateRange {
    pub from: Option<String>,
    pub till: Option<String>,
}

impl DateRange {
    /// Без ограничения диапазона.
    pub fn all() -> Self {
        Self::default()
    }

    /// Явный диапазон `[from, till]` (обе даты в формате `YYYY-MM-DD`).
    pub fn new(from: impl Into<String>, till: impl Into<String>) -> Self {
        Self {
            from: Some(from.into()),
            till: Some(till.into()),
        }
    }
}

/// Клиент MOEX ALGOPACK поверх [`HttpClient`] с произвольным транспортом `T`.
pub struct MoexAlgo<T: HttpTransport> {
    http: HttpClient<T>,
    base_url: String,
    token: String,
}

impl<T: HttpTransport> std::fmt::Debug for MoexAlgo<T> {
    /// Секрет (`token`) не попадает в `Debug` — только его длина, для отладки
    /// без риска утечки в логи.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MoexAlgo")
            .field("base_url", &self.base_url)
            .field("token_len", &self.token.len())
            .finish()
    }
}

impl<T: HttpTransport> MoexAlgo<T> {
    /// Клиент со стандартным базовым URL ([`DEFAULT_BASE_URL`]) и политиками
    /// [`HttpClient`] по умолчанию.
    pub fn new(transport: T, token: impl Into<String>) -> Self {
        Self::with_base_url(transport, DEFAULT_BASE_URL, token)
    }

    /// Клиент с явным базовым URL (тесты/прокси).
    pub fn with_base_url(
        transport: T,
        base_url: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        Self::with_client(HttpClient::new(transport), base_url, token)
    }

    /// Клиент поверх уже сконфигурированного [`HttpClient`] (свои лимиты/backoff).
    pub fn with_client(
        http: HttpClient<T>,
        base_url: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        Self {
            http,
            base_url: base_url.into(),
            token: token.into(),
        }
    }

    fn headers(&self) -> [(String, String); 1] {
        [("Authorization".to_owned(), format!("Bearer {}", self.token))]
    }

    fn dataset_url(&self, market: Market, dataset: &str, ticker: Option<&str>) -> String {
        match ticker {
            Some(t) => format!("{}/{}/{}/{}.json", self.base_url, market.code(), dataset, t),
            None => format!("{}/{}/{}.json", self.base_url, market.code(), dataset),
        }
    }

    fn build_url(dataset_url: &str, range: &DateRange, start: Option<i64>) -> String {
        let mut url = dataset_url.to_owned();
        let mut first = true;
        push_param(&mut url, &mut first, "iss.meta", "off");
        if let Some(from) = &range.from {
            push_param(&mut url, &mut first, "from", from);
        }
        if let Some(till) = &range.till {
            push_param(&mut url, &mut first, "till", till);
        }
        if let Some(start) = start {
            push_param(&mut url, &mut first, "start", &start.to_string());
        }
        url
    }

    /// Выполнить датасет-запрос со склейкой курсорной пагинации: каждая
    /// страница добавляет свои строки к общей таблице, пока
    /// `<блок>.cursor` сообщает `has_more` (или блок курсора отсутствует —
    /// тогда одностраничный ответ и есть весь результат).
    ///
    /// `block_candidates` — кандидаты имени блока-обёртки ответа (порядок
    /// имеет значение, см. `parse.rs`/README фикстур — форма блока
    /// `(unverified)`).
    async fn fetch_paginated(
        &self,
        method: Method,
        dataset_url: &str,
        range: &DateRange,
        block_candidates: &[&str],
    ) -> Result<IssTable, DataError> {
        let mut merged = IssTable::default();
        let mut start: Option<i64> = None;
        let headers = self.headers();
        for _page in 0..MAX_PAGES {
            let url = Self::build_url(dataset_url, range, start);
            let value = self.http.get_json(method, &url, &headers).await?;
            let table = IssTable::find(&value, block_candidates).ok_or_else(|| {
                DataError::Other(format!(
                    "MOEX ISS: не найден блок данных {block_candidates:?} в ответе ({method})"
                ))
            })?;
            if merged.columns.is_empty() {
                merged.columns = table.columns.clone();
            }
            merged.rows.extend(table.rows);

            match IssCursor::find(&value, block_candidates) {
                Some(cursor) if cursor.has_more() => start = Some(cursor.next_start()),
                _ => return Ok(merged),
            }
        }
        Err(DataError::Other(format!(
            "MOEX ISS: превышен лимит страниц пагинации ({MAX_PAGES}) для {method}"
        )))
    }

    /// `tradestats` (Super Candles): `market` — рынок, `ticker` — конкретный
    /// инструмент (`None` — сводный ответ по рынку, если API его поддерживает).
    pub async fn tradestats(
        &self,
        market: Market,
        ticker: Option<&str>,
        range: DateRange,
    ) -> Result<Vec<SuperCandle>, DataError> {
        let url = self.dataset_url(market, "tradestats", ticker);
        let table = self
            .fetch_paginated(
                Method::MoexTradestats,
                &url,
                &range,
                &["data", "tradestats"],
            )
            .await?;
        Ok(parse_tradestats(&table))
    }

    /// `orderstats`: статистика заявок.
    pub async fn orderstats(
        &self,
        market: Market,
        ticker: Option<&str>,
        range: DateRange,
    ) -> Result<Vec<OrderstatsPoint>, DataError> {
        let url = self.dataset_url(market, "orderstats", ticker);
        let table = self
            .fetch_paginated(
                Method::MoexOrderstats,
                &url,
                &range,
                &["data", "orderstats"],
            )
            .await?;
        Ok(parse_orderstats(&table))
    }

    /// `obstats`: статистика стакана.
    pub async fn obstats(
        &self,
        market: Market,
        ticker: Option<&str>,
        range: DateRange,
    ) -> Result<Vec<ObstatsPoint>, DataError> {
        let url = self.dataset_url(market, "obstats", ticker);
        let table = self
            .fetch_paginated(Method::MoexObstats, &url, &range, &["data", "obstats"])
            .await?;
        Ok(parse_obstats(&table))
    }

    /// `hi2`: индекс концентрации участников (сводно по рынку, без тикера).
    pub async fn hi2(&self, market: Market, range: DateRange) -> Result<Vec<Hi2Point>, DataError> {
        let url = self.dataset_url(market, "hi2", None);
        let table = self
            .fetch_paginated(Method::MoexHi2, &url, &range, &["data", "hi2"])
            .await?;
        Ok(parse_hi2(&table))
    }

    /// `futoi`: нетто-позиции физ/юр лиц по фьючерсам. Датасет существует
    /// только на рынке `fo` (см. `10.0.1`), поэтому рынок фиксирован.
    pub async fn futoi(
        &self,
        ticker: Option<&str>,
        range: DateRange,
    ) -> Result<Vec<FutoiPoint>, DataError> {
        let url = self.dataset_url(Market::Fo, "futoi", ticker);
        let table = self
            .fetch_paginated(Method::MoexFutoi, &url, &range, &["data", "futoi"])
            .await?;
        Ok(parse_futoi(&table))
    }

    /// Свечи (задел под историю, фаза 11): формат ответа не входит в
    /// подтверждённый контракт `10.0.1` (перечислены только `tradestats`/
    /// `orderstats`/`obstats`/`hi2`/`futoi`) — метод-заглушка со своим
    /// парсером, помеченным `(unverified)`, чтобы не блокировать интеграцию
    /// `data::history` до появления живого ключа.
    pub async fn candles(
        &self,
        market: Market,
        ticker: &str,
        range: DateRange,
    ) -> Result<Vec<super::parse::IssCandle>, DataError> {
        let url = self.dataset_url(market, "candles", Some(ticker));
        let table = self
            .fetch_paginated(Method::MoexCandles, &url, &range, &["data", "candles"])
            .await?;
        Ok(super::parse::parse_candles(&table))
    }
}

fn push_param(url: &mut String, first: &mut bool, key: &str, value: &str) {
    url.push(if *first { '?' } else { '&' });
    *first = false;
    url.push_str(key);
    url.push('=');
    url.push_str(&percent_encode(value));
}

/// Минимальное percent-кодирование значения параметра запроса (без внешней
/// зависимости) — значения здесь простые (даты, тикеры, `off`), но
/// небезопасные символы (пробел, `/`, `:`) экранируются на всякий случай.
fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};

    use crate::http::HttpResponse;

    /// Один захваченный набор заголовков запроса.
    type CapturedHeaders = Vec<(String, String)>;

    /// Разделяемое состояние фейкового транспорта: клонируется в тест
    /// отдельно от самого транспорта (который переезжает во владение
    /// [`MoexAlgo`]/[`HttpClient`] и после этого недоступен напрямую —
    /// приватное поле в другом модуле крейта).
    #[derive(Clone, Default)]
    struct Shared {
        calls: Arc<AtomicU32>,
        captured_urls: Arc<Mutex<Vec<String>>>,
        captured_headers: Arc<Mutex<Vec<CapturedHeaders>>>,
    }

    struct FakeTransport {
        shared: Shared,
        pages: Vec<&'static str>,
    }

    impl FakeTransport {
        fn new(pages: Vec<&'static str>) -> (Self, Shared) {
            let shared = Shared::default();
            (
                Self {
                    shared: shared.clone(),
                    pages,
                },
                shared,
            )
        }
    }

    impl HttpTransport for FakeTransport {
        async fn get(
            &self,
            url: &str,
            headers: &[(String, String)],
        ) -> Result<HttpResponse, DataError> {
            self.shared
                .captured_urls
                .lock()
                .unwrap()
                .push(url.to_owned());
            self.shared
                .captured_headers
                .lock()
                .unwrap()
                .push(headers.to_vec());
            let i = self.shared.calls.fetch_add(1, Ordering::SeqCst) as usize;
            let body = self.pages[i.min(self.pages.len() - 1)];
            Ok(HttpResponse {
                status: 200,
                body: body.as_bytes().to_vec(),
            })
        }

        async fn post(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            _body: Vec<u8>,
        ) -> Result<HttpResponse, DataError> {
            unreachable!("клиент ALGOPACK использует только GET")
        }
    }

    fn client(transport: FakeTransport) -> MoexAlgo<FakeTransport> {
        MoexAlgo::with_base_url(
            transport,
            "https://example.invalid/algopack",
            "secret-token",
        )
    }

    #[tokio::test]
    async fn tradestats_url_and_bearer_header_reach_transport() {
        let page = r#"{"data": {"columns": ["secid","tradedate","tradetime","pr_close","vol"], "data": [["SBER","2024-01-15","10:00:00",265.5,100]]}}"#;
        let (transport, shared) = FakeTransport::new(vec![page]);
        let c = client(transport);
        let out = c
            .tradestats(Market::Eq, Some("SBER"), DateRange::all())
            .await
            .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].secid, "SBER");

        let urls = shared.captured_urls.lock().unwrap().clone();
        assert_eq!(urls.len(), 1);
        assert!(urls[0]
            .starts_with("https://example.invalid/algopack/eq/tradestats/SBER.json?iss.meta=off"));

        let headers = shared.captured_headers.lock().unwrap().clone();
        assert_eq!(
            headers[0],
            vec![("Authorization".to_owned(), "Bearer secret-token".to_owned())]
        );
    }

    #[tokio::test]
    async fn pagination_merges_pages_and_stops_on_last_cursor() {
        let p1 = r#"{
            "data": {"columns": ["secid","tradedate","tradetime","vol"], "data": [["SBER","2024-01-15","10:00:00",1]]},
            "data.cursor": {"columns": ["INDEX","TOTAL","PAGESIZE"], "data": [[0, 3, 1]]}
        }"#;
        let p2 = r#"{
            "data": {"columns": ["secid","tradedate","tradetime","vol"], "data": [["SBER","2024-01-15","10:05:00",2]]},
            "data.cursor": {"columns": ["INDEX","TOTAL","PAGESIZE"], "data": [[1, 3, 1]]}
        }"#;
        let p3 = r#"{
            "data": {"columns": ["secid","tradedate","tradetime","vol"], "data": [["SBER","2024-01-15","10:10:00",3]]},
            "data.cursor": {"columns": ["INDEX","TOTAL","PAGESIZE"], "data": [[2, 3, 1]]}
        }"#;
        let (transport, shared) = FakeTransport::new(vec![p1, p2, p3]);
        let c = client(transport);
        let out = c
            .tradestats(Market::Eq, None, DateRange::all())
            .await
            .unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(
            out.iter().map(|c| c.vol as i64).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );

        let urls = shared.captured_urls.lock().unwrap().clone();
        assert_eq!(urls.len(), 3);
        assert!(urls[1].contains("start=1"));
        assert!(urls[2].contains("start=2"));
    }

    #[tokio::test]
    async fn single_page_without_cursor_stops_after_one_call() {
        let page = r#"{"data": {"columns": ["secid","tradedate","tradetime","hi2"], "data": [["SBER","2024-01-15","10:00:00",0.2]]}}"#;
        let (transport, shared) = FakeTransport::new(vec![page]);
        let c = client(transport);
        let out = c.hi2(Market::Eq, DateRange::all()).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(shared.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn date_range_appends_from_till_params() {
        let page = r#"{"data": {"columns": ["secid","tradedate","tradetime","hi2"], "data": []}}"#;
        let (transport, shared) = FakeTransport::new(vec![page]);
        let c = client(transport);
        c.hi2(Market::Fo, DateRange::new("2024-01-01", "2024-01-31"))
            .await
            .unwrap();
        let urls = shared.captured_urls.lock().unwrap().clone();
        assert!(urls[0].contains("from=2024-01-01"));
        assert!(urls[0].contains("till=2024-01-31"));
    }

    #[tokio::test]
    async fn futoi_always_uses_fo_market() {
        let page = r#"{"data": {"columns": ["secid","tradedate","tradetime","clgroup","pos","pos_long","pos_short","pos_long_num","pos_short_num"], "data": [["RIH5","2024-01-15","18:45:00","fiz",100,60,40,10,8]]}}"#;
        let (transport, shared) = FakeTransport::new(vec![page]);
        let c = client(transport);
        let out = c.futoi(Some("RIH5"), DateRange::all()).await.unwrap();
        assert_eq!(out.len(), 1);
        let urls = shared.captured_urls.lock().unwrap().clone();
        assert!(urls[0].starts_with("https://example.invalid/algopack/fo/futoi/RIH5.json"));
    }

    #[tokio::test]
    async fn missing_data_block_is_an_error_not_a_panic() {
        let page = r#"{"unexpected": 1}"#;
        let (transport, _shared) = FakeTransport::new(vec![page]);
        let c = client(transport);
        let err = c
            .tradestats(Market::Eq, None, DateRange::all())
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::Other(_)));
    }

    #[test]
    fn debug_output_does_not_expose_token() {
        let (transport, _shared) = FakeTransport::new(vec!["{}"]);
        let c = client(transport);
        let debug = format!("{c:?}");
        assert!(!debug.contains("secret-token"));
    }

    #[test]
    fn dataset_url_shapes_match_contract() {
        let (transport, _shared) = FakeTransport::new(vec!["{}"]);
        let c = client(transport);
        assert_eq!(
            c.dataset_url(Market::Eq, "tradestats", None),
            "https://example.invalid/algopack/eq/tradestats.json"
        );
        assert_eq!(
            c.dataset_url(Market::Fo, "futoi", Some("RIH5")),
            "https://example.invalid/algopack/fo/futoi/RIH5.json"
        );
    }
}
