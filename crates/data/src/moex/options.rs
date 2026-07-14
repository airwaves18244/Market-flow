//! Опционная доска MOEX через **публичный** ISS (`iss.moex.com`), в отличие
//! от [`super::client::MoexAlgo`] (ALGOPACK, `apim.moex.com`, Bearer-токен).
//!
//! Контракт `SPEC_0-12.md` `12.4.1`–`12.4.3`: доска срочного рынка опционов —
//! движок `futures`, рынок `options` (`engines/futures/markets/options`).
//! Публичные ресурсы ISS securities/marketdata не требуют авторизации, поэтому
//! [`MoexIss`] не хранит секрет (в отличие от [`super::client::MoexAlgo`]).
//!
//! **Статус: `(unverified)`.** Egress к `iss.moex.com` в этой среде закрыт —
//! точные имена блоков-обёрток (`securities`/`marketdata`), набор колонок,
//! механизм фильтрации по базовому активу и форма пагинации не сверены живым
//! ответом. См. `crates/data/tests/fixtures/moex/README.md` (раздел
//! «Опционная доска») — там же процедура сверки и текущие допущения:
//! - блоки ответа — `securities` (карточки инструментов) и `marketdata`
//!   (котировки/IV/OI), совместно в одном ответе `securities.json` при
//!   `iss.only=securities,marketdata` (общая практика ISS);
//! - фильтрация по базовому активу (`underlying`) выполняется **на стороне
//!   клиента** (сравнение колонки `ASSETCODE`/`underlying` без учёта
//!   регистра) — какие query-параметры ISS принимает для серверной
//!   фильтрации, не подтверждено, а клиентская фильтрация работает при любом
//!   их наборе (сервер просто возвращает более широкий набор строк);
//! - пагинация — курсор `securities.cursor` (как у прочих ресурсов ISS, см.
//!   [`super::parse::IssCursor`]), `marketdata` собирается по тем же
//!   страницам без отдельного курсора;
//! - форвард базового актива — последняя цена (`LAST`, иначе `SETTLEPRICE`)
//!   фьючерса-андерлаинга с рынка `forts` (`engines/futures/markets/forts`).
//!   Не найден — не ошибка: [`OptionsBoardSnapshot::forward`] остаётся
//!   `None`, вызывающая сторона (`app`) подставляет форвард из настроек/входа.

use std::collections::HashMap;

use domain::options::{implied_vol, OptionType, PriceModel, SmilePoint};

use crate::http::{HttpClient, HttpTransport};
use crate::{DataError, Method};

use super::parse::{moex_datetime_to_unix, IssCursor, IssTable, RowView};

/// Базовый URL публичного ISS MOEX (без авторизации).
pub const DEFAULT_OPTIONS_BASE_URL: &str = "https://iss.moex.com/iss";

/// Верхний предел числа страниц пагинации — тот же защитный клапан, что и у
/// [`super::client::MoexAlgo`] (см. его документацию).
const MAX_PAGES: u32 = 1000;

/// Котировка одного опциона доски (транспортный тип; не путать с
/// [`domain::options::SmilePoint`] — точкой калибровки).
///
/// `secid`/`underlying`/`strike`/`kind`/`expiration_ts` обязательны (строка
/// без них бесполезна и отбрасывается парсером); котировочные поля мягкие —
/// биржа может не прислать часть из них (нет сделок, нет расчёта IV и т.п.).
#[derive(Debug, Clone, PartialEq)]
pub struct OptionQuote {
    /// Код инструмента (SECID).
    pub secid: String,
    /// Код базового актива (обычно фьючерс, ASSETCODE).
    pub underlying: String,
    /// Дата экспирации серии, unix-секунды UTC (00:00 МСК даты исполнения).
    pub expiration_ts: i64,
    /// Страйк.
    pub strike: f64,
    /// Тип опциона (колл/пут).
    pub kind: OptionType,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub last: Option<f64>,
    /// Подразумеваемая волатильность по расчёту биржи, если отдана в доске.
    pub iv: Option<f64>,
    /// Открытый интерес.
    pub oi: Option<f64>,
    /// Теоретическая цена (расчёт НКЦ), если отдана в доске.
    pub theor_price: Option<f64>,
}

/// Снимок доски: котировки конкретного базового актива + (по возможности)
/// его форвард.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OptionsBoardSnapshot {
    pub quotes: Vec<OptionQuote>,
    /// `None`, если цену фьючерса-андерлаинга не удалось получить —
    /// не ошибка, см. модульную документацию.
    pub forward: Option<f64>,
}

/// Разобрать тип опциона из значения колонки `OPTIONTYPE` (`C`/`P`,
/// `Call`/`Put`, без учёта регистра — форма значения `(unverified)`).
fn parse_option_kind(raw: &str) -> Option<OptionType> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "c" | "call" => Some(OptionType::Call),
        "p" | "put" => Some(OptionType::Put),
        _ => None,
    }
}

/// Средняя цена между `bid`/`ask`; если есть только одна сторона — она и
/// берётся; если нет ни одной — `None`.
fn mid_price(bid: Option<f64>, ask: Option<f64>) -> Option<f64> {
    match (bid, ask) {
        (Some(b), Some(a)) => Some(0.5 * (b + a)),
        (Some(b), None) => Some(b),
        (None, Some(a)) => Some(a),
        (None, None) => None,
    }
}

/// Индекс строк `marketdata` по `SECID` для джойна с `securities`.
fn marketdata_index(table: Option<&IssTable>) -> HashMap<&str, RowView<'_>> {
    let mut idx = HashMap::new();
    if let Some(t) = table {
        for row in t.rows_iter() {
            if let Some(secid) = row.str("secid") {
                idx.insert(secid, row);
            }
        }
    }
    idx
}

/// Разобрать таблицы `securities`+`marketdata` в котировки доски,
/// отфильтрованные по коду базового актива `underlying` (без учёта регистра).
///
/// Строки без обязательных полей (`secid`/`assetcode`/`strike`/`optiontype`/
/// даты экспирации) молча пропускаются — как и у остальных парсеров ISS в
/// этом крейте (`parse.rs`).
pub fn parse_options_board(
    securities: &IssTable,
    marketdata: Option<&IssTable>,
    underlying: &str,
) -> Vec<OptionQuote> {
    let md_index = marketdata_index(marketdata);
    securities
        .rows_iter()
        .filter_map(|row| {
            let secid = row.str("secid")?.to_owned();
            let asset = row.str("assetcode").or_else(|| row.str("underlying"))?;
            if !asset.eq_ignore_ascii_case(underlying) {
                return None;
            }
            let strike = row.f64("strike")?;
            let kind = parse_option_kind(row.str("optiontype").or_else(|| row.str("type"))?)?;
            let date = row
                .str("lasttradedate")
                .or_else(|| row.str("expdate"))
                .or_else(|| row.str("lastdeldate"))?;
            let expiration_ts = moex_datetime_to_unix(date, "00:00:00")?;

            let (bid, ask, last, iv, oi, theor_price) = match md_index.get(secid.as_str()) {
                Some(m) => (
                    m.f64("bid"),
                    m.f64("offer").or_else(|| m.f64("ask")),
                    m.f64("last"),
                    m.f64("iv").or_else(|| m.f64("volatility")),
                    m.f64("openposition").or_else(|| m.f64("oi")),
                    m.f64("theorprice").or_else(|| m.f64("theoreticalprice")),
                ),
                None => Default::default(),
            };

            Some(OptionQuote {
                secid,
                underlying: asset.to_owned(),
                expiration_ts,
                strike,
                kind,
                bid,
                ask,
                last,
                iv,
                oi,
                theor_price,
            })
        })
        .collect()
}

/// Одна рыночная точка улыбки из котировки доски, либо `None`, если
/// котировка неликвидна (нет ни `bid`/`ask`, ни положительного `oi`) или
/// IV недостижима (нет ни готового значения, ни цены для решателя).
fn smile_point_from_quote(
    quote: &OptionQuote,
    forward: f64,
    t: f64,
    rate: f64,
) -> Option<SmilePoint> {
    let has_quote = quote.bid.is_some() || quote.ask.is_some();
    let has_oi = quote.oi.map(|v| v > 0.0).unwrap_or(false);
    if !has_quote && !has_oi {
        return None; // неликвид — ни котировок, ни открытого интереса.
    }
    let iv = quote.iv.or_else(|| {
        let price = quote
            .theor_price
            .or_else(|| mid_price(quote.bid, quote.ask))
            .or(quote.last)?;
        implied_vol(
            price,
            forward,
            quote.strike,
            t,
            rate,
            quote.kind,
            PriceModel::Black76,
        )
    })?;
    let weight = quote.oi.unwrap_or(1.0).max(0.0);
    Some(SmilePoint {
        strike: quote.strike,
        iv,
        weight,
    })
}

/// Отобрать серию по экспирации и превратить доску в рыночные точки улыбки
/// для калибратора (`domain::options::SmileModel::calibrate`).
///
/// Правила (`SPEC_0-12.md` `12.4.2`):
/// - точки берутся только из серии `expiration_ts`;
/// - IV — из доски, либо решателем `domain::options::implied_vol` из
///   теоретической цены (приоритет) или средней `bid`/`ask` (фолбэк);
/// - вес — открытый интерес, иначе `1.0` (котировка ликвидна по `bid`/`ask`,
///   но OI не пришёл);
/// - неликвидные строки (нет ни `bid`/`ask`, ни `oi`) отбрасываются.
pub fn board_to_smile_points(
    quotes: &[OptionQuote],
    expiration_ts: i64,
    forward: f64,
    t: f64,
    rate: f64,
) -> Vec<SmilePoint> {
    quotes
        .iter()
        .filter(|q| q.expiration_ts == expiration_ts)
        .filter_map(|q| smile_point_from_quote(q, forward, t, rate))
        .collect()
}

/// Клиент публичного ISS MOEX: доска опционов + best-effort форвард
/// базового актива. В отличие от [`super::client::MoexAlgo`] не хранит
/// секрет — публичные ресурсы `iss.moex.com` не требуют авторизации.
pub struct MoexIss<T: HttpTransport> {
    http: HttpClient<T>,
    base_url: String,
}

impl<T: HttpTransport> MoexIss<T> {
    /// Клиент со стандартным базовым URL ([`DEFAULT_OPTIONS_BASE_URL`]).
    pub fn new(transport: T) -> Self {
        Self::with_base_url(transport, DEFAULT_OPTIONS_BASE_URL)
    }

    /// Клиент с явным базовым URL (тесты/прокси).
    pub fn with_base_url(transport: T, base_url: impl Into<String>) -> Self {
        Self::with_client(HttpClient::new(transport), base_url)
    }

    /// Клиент поверх уже сконфигурированного [`HttpClient`] (свои лимиты/backoff).
    pub fn with_client(http: HttpClient<T>, base_url: impl Into<String>) -> Self {
        Self {
            http,
            base_url: base_url.into(),
        }
    }

    /// Постранично забрать `securities`+`marketdata` по одному URL (страница
    /// добавляет `&start=N`), пока `securities.cursor` сообщает `has_more`.
    async fn fetch_board_pages(&self, url: &str) -> Result<(IssTable, IssTable), DataError> {
        let mut securities = IssTable::default();
        let mut marketdata = IssTable::default();
        let mut start: Option<i64> = None;
        for _page in 0..MAX_PAGES {
            let page_url = match start {
                Some(s) => format!("{url}&start={s}"),
                None => url.to_owned(),
            };
            let value = self
                .http
                .get_json(Method::MoexOptions, &page_url, &[])
                .await?;
            let sec_table = IssTable::find(&value, &["securities"]).ok_or_else(|| {
                DataError::Other(
                    "MOEX ISS: не найден блок securities в ответе (moex_options)".to_owned(),
                )
            })?;
            if securities.columns.is_empty() {
                securities.columns = sec_table.columns.clone();
            }
            securities.rows.extend(sec_table.rows);

            if let Some(md_table) = IssTable::find(&value, &["marketdata"]) {
                if marketdata.columns.is_empty() {
                    marketdata.columns = md_table.columns.clone();
                }
                marketdata.rows.extend(md_table.rows);
            }

            match IssCursor::find(&value, &["securities"]) {
                Some(cursor) if cursor.has_more() => start = Some(cursor.next_start()),
                _ => return Ok((securities, marketdata)),
            }
        }
        Err(DataError::Other(format!(
            "MOEX ISS: превышен лимит страниц пагинации ({MAX_PAGES}) для опционной доски"
        )))
    }

    /// Доска опционов по коду базового актива (без форварда — см.
    /// [`MoexIss::options_board_snapshot`]).
    pub async fn options_board(&self, underlying: &str) -> Result<Vec<OptionQuote>, DataError> {
        let url = format!(
            "{}/engines/futures/markets/options/securities.json?iss.only=securities,marketdata&iss.meta=off",
            self.base_url
        );
        let (securities, marketdata) = self.fetch_board_pages(&url).await?;
        Ok(parse_options_board(
            &securities,
            Some(&marketdata),
            underlying,
        ))
    }

    /// Лучшая попытка определить форвард как последнюю цену фьючерса-
    /// андерлаинга (рынок `forts`). `Ok(None)`, если данные недоступны —
    /// не ошибка (см. модульную документацию).
    pub async fn underlying_forward(&self, underlying: &str) -> Result<Option<f64>, DataError> {
        let url = format!(
            "{}/engines/futures/markets/forts/securities/{underlying}.json?iss.only=marketdata&iss.meta=off",
            self.base_url
        );
        let value = self.http.get_json(Method::MoexOptions, &url, &[]).await?;
        let table = match IssTable::find(&value, &["marketdata"]) {
            Some(t) => t,
            None => return Ok(None),
        };
        let forward = table
            .rows_iter()
            .next()
            .and_then(|row| row.f64("last").or_else(|| row.f64("settleprice")));
        Ok(forward)
    }

    /// Снимок доски: котировки + форвард. Ошибка определения форварда не
    /// проваливает весь вызов (см. [`MoexIss::underlying_forward`]) — только
    /// ошибка загрузки самой доски возвращается вызывающей стороне.
    pub async fn options_board_snapshot(
        &self,
        underlying: &str,
    ) -> Result<OptionsBoardSnapshot, DataError> {
        let quotes = self.options_board(underlying).await?;
        let forward = self.underlying_forward(underlying).await.unwrap_or(None);
        Ok(OptionsBoardSnapshot { quotes, forward })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;

    fn fixture(name: &str) -> Value {
        let path = format!("{}/tests/fixtures/moex/{name}", env!("CARGO_MANIFEST_DIR"));
        let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{path}: невалидный JSON: {e}"))
    }

    fn quote(strike: f64, kind: OptionType) -> OptionQuote {
        OptionQuote {
            secid: format!("Q{strike}"),
            underlying: "RIH5".to_owned(),
            expiration_ts: 1_000,
            strike,
            kind,
            bid: None,
            ask: None,
            last: None,
            iv: None,
            oi: None,
            theor_price: None,
        }
    }

    // ── Парсинг доски на фикстурах ───────────────────────────────────────────

    #[test]
    fn parses_board_fixture_filters_by_underlying_and_drops_bad_rows() {
        let v = fixture("options_board.json");
        let securities = IssTable::find(&v, &["securities"]).unwrap();
        let marketdata = IssTable::find(&v, &["marketdata"]).unwrap();
        let quotes = parse_options_board(&securities, Some(&marketdata), "RIH5");

        // 4 валидные строки RIH5 (5-я — SiH5, 6-я — без страйка, отбрасываются).
        assert_eq!(quotes.len(), 4);
        assert!(quotes.iter().all(|q| q.underlying == "RIH5"));

        let itm_call = quotes.iter().find(|q| q.strike == 50_000.0).unwrap();
        assert_eq!(itm_call.kind, OptionType::Call);
        assert_eq!(itm_call.bid, Some(2500.0));
        assert_eq!(itm_call.iv, Some(0.32));
        assert_eq!(itm_call.oi, Some(1200.0));

        // Пут без сделок/OI — присутствует в разборе (парсер мягкий), но
        // будет отфильтрован маппингом в точки улыбки (см. следующий тест).
        let illiquid_put = quotes.iter().find(|q| q.kind == OptionType::Put).unwrap();
        assert!(illiquid_put.bid.is_none());
        assert!(illiquid_put.oi.is_none() || illiquid_put.oi == Some(0.0));
    }

    #[test]
    fn parses_empty_board_to_empty_vec() {
        let v = fixture("options_board_empty.json");
        let securities = IssTable::find(&v, &["securities"]).unwrap();
        let marketdata = IssTable::find(&v, &["marketdata"]);
        let quotes = parse_options_board(&securities, marketdata.as_ref(), "RIH5");
        assert!(quotes.is_empty());
    }

    #[test]
    fn parses_board_without_marketdata_block_leaves_quote_fields_soft() {
        let v = fixture("options_board.json");
        let securities = IssTable::find(&v, &["securities"]).unwrap();
        // Без marketdata вовсе — все котировочные поля мягко пустые, но
        // разбор не падает (устойчивость к отсутствию блока).
        let quotes = parse_options_board(&securities, None, "RIH5");
        assert_eq!(quotes.len(), 4);
        assert!(quotes.iter().all(|q| q.bid.is_none() && q.iv.is_none()));
    }

    // ── Маппинг доски → точки улыбки ─────────────────────────────────────────

    #[test]
    fn board_to_smile_points_drops_illiquid_and_uses_board_iv() {
        let v = fixture("options_board.json");
        let securities = IssTable::find(&v, &["securities"]).unwrap();
        let marketdata = IssTable::find(&v, &["marketdata"]).unwrap();
        let quotes = parse_options_board(&securities, Some(&marketdata), "RIH5");
        let expiration_ts = moex_datetime_to_unix("2025-01-16", "00:00:00").unwrap();

        let points = board_to_smile_points(&quotes, expiration_ts, 50_500.0, 0.05, 0.0);
        // 4 разобранные строки RIH5, но неликвидный пут (нет bid/ask и OI) отброшен.
        assert_eq!(points.len(), 3);

        // Строка с готовым IV в доске — используется как есть.
        let itm = points.iter().find(|p| p.strike == 50_000.0).unwrap();
        assert!((itm.iv - 0.32).abs() < 1e-12);
        assert!((itm.weight - 1200.0).abs() < 1e-9);
    }

    #[test]
    fn board_to_smile_points_computes_iv_from_theor_price_when_missing() {
        let v = fixture("options_board.json");
        let securities = IssTable::find(&v, &["securities"]).unwrap();
        let marketdata = IssTable::find(&v, &["marketdata"]).unwrap();
        let quotes = parse_options_board(&securities, Some(&marketdata), "RIH5");
        let expiration_ts = moex_datetime_to_unix("2025-01-16", "00:00:00").unwrap();
        let forward = 50_500.0;
        let t = 0.05;

        let points = board_to_smile_points(&quotes, expiration_ts, forward, t, 0.0);
        // Строка 55000-call: нет IV в доске, но есть theor_price — решается через IV-солвер.
        let row = quotes
            .iter()
            .find(|q| q.strike == 55_000.0)
            .expect("55000 call есть в фикстуре");
        assert!(row.iv.is_none());
        let theor = row.theor_price.unwrap();

        let point = points.iter().find(|p| p.strike == 55_000.0).unwrap();
        let expected_iv = implied_vol(
            theor,
            forward,
            55_000.0,
            t,
            0.0,
            OptionType::Call,
            PriceModel::Black76,
        )
        .unwrap();
        assert!((point.iv - expected_iv).abs() < 1e-9);
    }

    #[test]
    fn board_to_smile_points_filters_by_expiration_series() {
        let mut near = quote(100.0, OptionType::Call); // серия 1_000 (см. quote()).
        near.bid = Some(5.0);
        near.ask = Some(6.0);
        near.iv = Some(0.3);
        let mut far = quote(110.0, OptionType::Call);
        far.expiration_ts = 2_000; // другая серия — не должна попасть в точки.
        far.bid = Some(1.0);
        far.iv = Some(0.4);

        let points = board_to_smile_points(&[near, far], 1_000, 100.0, 0.1, 0.0);
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].strike, 100.0);
    }

    #[test]
    fn smile_point_weight_defaults_to_one_without_oi() {
        let mut q = quote(100.0, OptionType::Call);
        q.bid = Some(4.0);
        q.ask = Some(5.0);
        q.iv = Some(0.25);
        let points = board_to_smile_points(&[q], 1_000, 100.0, 0.1, 0.0);
        assert_eq!(points.len(), 1);
        assert!((points[0].weight - 1.0).abs() < 1e-12);
    }

    // ── Клиент: URL/пагинация/best-effort форвард ────────────────────────────

    #[derive(Clone, Default)]
    struct Shared {
        calls: std::sync::Arc<AtomicU32>,
        captured_urls: std::sync::Arc<Mutex<Vec<String>>>,
    }

    struct FakeTransport {
        shared: Shared,
        pages: Vec<Result<&'static str, DataError>>,
    }

    impl FakeTransport {
        fn new(pages: Vec<&'static str>) -> (Self, Shared) {
            Self::with_results(pages.into_iter().map(Ok).collect())
        }

        fn with_results(pages: Vec<Result<&'static str, DataError>>) -> (Self, Shared) {
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
        ) -> Result<crate::http::HttpResponse, DataError> {
            assert!(
                headers.is_empty(),
                "публичный ISS не требует авторизации — заголовков быть не должно"
            );
            self.shared
                .captured_urls
                .lock()
                .unwrap()
                .push(url.to_owned());
            let i = self.shared.calls.fetch_add(1, Ordering::SeqCst) as usize;
            let idx = i.min(self.pages.len() - 1);
            match &self.pages[idx] {
                Ok(body) => Ok(crate::http::HttpResponse {
                    status: 200,
                    body: body.as_bytes().to_vec(),
                }),
                Err(e) => Err(e.clone()),
            }
        }

        async fn post(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            _body: Vec<u8>,
        ) -> Result<crate::http::HttpResponse, DataError> {
            unreachable!("клиент ISS использует только GET")
        }
    }

    fn client(transport: FakeTransport) -> MoexIss<FakeTransport> {
        MoexIss::with_base_url(transport, "https://example.invalid/iss")
    }

    #[tokio::test]
    async fn options_board_sends_no_auth_header_and_filters_by_underlying() {
        let page = r#"{
            "securities": {"columns": ["secid","assetcode","strike","optiontype","lasttradedate"],
                "data": [["RI50000BC5A","RIH5",50000,"C","2025-01-16"],
                         ["SI75000BC5A","SiH5",75000,"C","2025-01-16"]]},
            "marketdata": {"columns": ["secid","bid","offer"],
                "data": [["RI50000BC5A",2500,2600],["SI75000BC5A",1000,1100]]}
        }"#;
        let (transport, shared) = FakeTransport::new(vec![page]);
        let c = client(transport);
        let out = c.options_board("RIH5").await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].secid, "RI50000BC5A");

        let urls = shared.captured_urls.lock().unwrap().clone();
        assert_eq!(urls.len(), 1);
        assert!(urls[0].starts_with(
            "https://example.invalid/iss/engines/futures/markets/options/securities.json?"
        ));
    }

    #[tokio::test]
    async fn options_board_paginates_on_securities_cursor() {
        let p1 = r#"{
            "securities": {"columns": ["secid","assetcode","strike","optiontype","lasttradedate"],
                "data": [["A","RIH5",100,"C","2025-01-16"]]},
            "securities.cursor": {"columns": ["INDEX","TOTAL","PAGESIZE"], "data": [[0, 2, 1]]}
        }"#;
        let p2 = r#"{
            "securities": {"columns": ["secid","assetcode","strike","optiontype","lasttradedate"],
                "data": [["B","RIH5",110,"P","2025-01-16"]]},
            "securities.cursor": {"columns": ["INDEX","TOTAL","PAGESIZE"], "data": [[1, 2, 1]]}
        }"#;
        let (transport, shared) = FakeTransport::new(vec![p1, p2]);
        let c = client(transport);
        let out = c.options_board("RIH5").await.unwrap();
        assert_eq!(out.len(), 2);

        let urls = shared.captured_urls.lock().unwrap().clone();
        assert_eq!(urls.len(), 2);
        assert!(urls[1].contains("start=1"));
    }

    #[tokio::test]
    async fn missing_securities_block_is_an_error_not_a_panic() {
        let (transport, _shared) = FakeTransport::new(vec!["{\"unexpected\": 1}"]);
        let c = client(transport);
        let err = c.options_board("RIH5").await.unwrap_err();
        assert!(matches!(err, DataError::Other(_)));
    }

    #[tokio::test]
    async fn underlying_forward_reads_last_price_from_marketdata() {
        let page = r#"{"marketdata": {"columns": ["secid","last","settleprice"], "data": [["RIH5", 50250.0, 50000.0]]}}"#;
        let (transport, _shared) = FakeTransport::new(vec![page]);
        let c = client(transport);
        let fwd = c.underlying_forward("RIH5").await.unwrap();
        assert_eq!(fwd, Some(50250.0));
    }

    #[tokio::test]
    async fn underlying_forward_falls_back_to_settleprice_without_last() {
        let page = r#"{"marketdata": {"columns": ["secid","last","settleprice"], "data": [["RIH5", null, 50000.0]]}}"#;
        let (transport, _shared) = FakeTransport::new(vec![page]);
        let c = client(transport);
        let fwd = c.underlying_forward("RIH5").await.unwrap();
        assert_eq!(fwd, Some(50000.0));
    }

    #[tokio::test]
    async fn underlying_forward_is_none_without_marketdata_block() {
        let (transport, _shared) = FakeTransport::new(vec!["{\"unrelated\": 1}"]);
        let c = client(transport);
        let fwd = c.underlying_forward("RIH5").await.unwrap();
        assert_eq!(fwd, None);
    }

    #[tokio::test]
    async fn snapshot_keeps_quotes_when_forward_lookup_fails() {
        // Первая страница (доска) валидна; второй вызов (форвард) — сетевая
        // ошибка. Снимок не должен проваливаться целиком: форвард — `None`.
        let board_page = r#"{
            "securities": {"columns": ["secid","assetcode","strike","optiontype","lasttradedate"],
                "data": [["A","RIH5",100,"C","2025-01-16"]]}
        }"#;
        let (transport, _shared) = FakeTransport::with_results(vec![
            Ok(board_page),
            Err(DataError::Transport("сеть недоступна".into())),
        ]);
        let c = MoexIss::with_client(
            HttpClient::with_policy(
                transport,
                crate::RateLimiter::per_minute(1000),
                crate::Backoff::new(std::time::Duration::ZERO, 1.0, std::time::Duration::ZERO, 0),
            ),
            "https://example.invalid/iss",
        );
        let snapshot = c.options_board_snapshot("RIH5").await.unwrap();
        assert_eq!(snapshot.quotes.len(), 1);
        assert_eq!(snapshot.forward, None);
    }
}
