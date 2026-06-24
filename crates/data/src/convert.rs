//! Преобразование protobuf-сообщений Finam в доменные типы.
//!
//! Чистые функции без сети — это тестируемое ядро адаптера. Цены приходят как
//! `google.type.Decimal` (строка), время — `google.protobuf.Timestamp`.

use finam_proto::pb::google::r#type::{Decimal, Interval};
use finam_proto::{assets, marketdata, Side};
use prost_types::Timestamp;

use crate::TimeFrame;

/// Разобрать `google.type.Decimal` в `f64`. Пустое/битое значение → `0.0`.
pub(crate) fn dec(value: &Option<Decimal>) -> f64 {
    value
        .as_ref()
        .and_then(|d| d.value.parse::<f64>().ok())
        .unwrap_or(0.0)
}

/// Время в UNIX-секундах UTC из `Timestamp`. Отсутствие → `0`.
pub(crate) fn unix(ts: &Option<Timestamp>) -> i64 {
    ts.as_ref().map(|t| t.seconds).unwrap_or(0)
}

/// Интервал `[from, to]` (UNIX-секунды) для запросов истории.
pub(crate) fn interval(from_ts: i64, to_ts: i64) -> Interval {
    Interval {
        start_time: Some(Timestamp {
            seconds: from_ts,
            nanos: 0,
        }),
        end_time: Some(Timestamp {
            seconds: to_ts,
            nanos: 0,
        }),
    }
}

/// Доменный тайм-фрейм → protobuf-перечисление `MarketDataService.TimeFrame`.
pub(crate) fn timeframe(tf: TimeFrame) -> marketdata::TimeFrame {
    match tf {
        TimeFrame::M1 => marketdata::TimeFrame::M1,
        TimeFrame::M5 => marketdata::TimeFrame::M5,
        TimeFrame::M15 => marketdata::TimeFrame::M15,
        TimeFrame::H1 => marketdata::TimeFrame::H1,
        TimeFrame::D1 => marketdata::TimeFrame::D,
    }
}

/// Класс актива из строкового типа Finam (`EQUITIES`/`FUTURES`/`BONDS`/…).
/// Неизвестные типы относим к акциям (v1 покрывает три класса).
pub(crate) fn asset_class(type_str: &str) -> domain::AssetClass {
    let t = type_str.to_ascii_uppercase();
    if t.contains("FUTUR") {
        domain::AssetClass::Future
    } else if t.contains("BOND") {
        domain::AssetClass::Bond
    } else {
        domain::AssetClass::Equity
    }
}

/// `marketdata.Bar` → [`domain::Bar`].
pub(crate) fn bar(b: &marketdata::Bar) -> domain::Bar {
    domain::Bar {
        ts: unix(&b.timestamp),
        open: dec(&b.open),
        high: dec(&b.high),
        low: dec(&b.low),
        close: dec(&b.close),
        volume: dec(&b.volume),
    }
}

/// `marketdata.Quote` → [`domain::Quote`].
pub(crate) fn quote(q: &marketdata::Quote) -> domain::Quote {
    domain::Quote {
        ts: unix(&q.timestamp),
        last: dec(&q.last),
        bid: dec(&q.bid),
        ask: dec(&q.ask),
        volume: dec(&q.volume),
    }
}

/// `marketdata.Trade` → [`domain::Trade`]. Сторона: BUY → `Some(true)`,
/// SELL → `Some(false)`, не указана → `None`.
pub(crate) fn trade(t: &marketdata::Trade) -> domain::Trade {
    let buyer_initiated = match t.side() {
        Side::Buy => Some(true),
        Side::Sell => Some(false),
        Side::Unspecified => None,
    };
    domain::Trade {
        ts: unix(&t.timestamp),
        price: dec(&t.price),
        size: dec(&t.size),
        buyer_initiated,
    }
}

/// `assets.Asset` → [`domain::Instrument`]. Сектор заполняется позже
/// классификатором; лот в `Asset` отсутствует (уточняется через GetAssetParams),
/// поэтому по умолчанию `1`.
pub(crate) fn instrument(a: &assets::Asset) -> domain::Instrument {
    domain::Instrument {
        symbol: a.symbol.clone(),
        ticker: a.ticker.clone(),
        name: a.name.clone(),
        asset_class: asset_class(&a.r#type),
        sector: None,
        lot_size: 1,
        isin: (!a.isin.is_empty()).then(|| a.isin.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Option<Decimal> {
        Some(Decimal { value: s.to_string() })
    }
    fn t(sec: i64) -> Option<Timestamp> {
        Some(Timestamp { seconds: sec, nanos: 0 })
    }

    #[test]
    fn decimal_parsing_handles_empty_and_invalid() {
        assert_eq!(dec(&d("123.45")), 123.45);
        assert_eq!(dec(&d("-0.5")), -0.5);
        assert_eq!(dec(&None), 0.0);
        assert_eq!(dec(&d("")), 0.0);
        assert_eq!(dec(&d("nan-ish")), 0.0);
    }

    #[test]
    fn timestamp_to_unix() {
        assert_eq!(unix(&t(1_700_000_000)), 1_700_000_000);
        assert_eq!(unix(&None), 0);
    }

    #[test]
    fn bar_maps_all_fields() {
        let b = marketdata::Bar {
            timestamp: t(1000),
            open: d("100"),
            high: d("110"),
            low: d("95"),
            close: d("105"),
            volume: d("12.5"),
        };
        let got = bar(&b);
        assert_eq!(got.ts, 1000);
        assert_eq!(got.open, 100.0);
        assert_eq!(got.high, 110.0);
        assert_eq!(got.low, 95.0);
        assert_eq!(got.close, 105.0);
        assert_eq!(got.volume, 12.5);
    }

    #[test]
    fn trade_side_maps_to_buyer_initiated() {
        let mk = |side: Side| marketdata::Trade {
            trade_id: "1".into(),
            mpid: String::new(),
            timestamp: t(1),
            price: d("10"),
            size: d("2"),
            side: side as i32,
            open_interest: None,
        };
        assert_eq!(trade(&mk(Side::Buy)).buyer_initiated, Some(true));
        assert_eq!(trade(&mk(Side::Sell)).buyer_initiated, Some(false));
        assert_eq!(trade(&mk(Side::Unspecified)).buyer_initiated, None);
    }

    #[test]
    fn asset_class_classification() {
        assert_eq!(asset_class("FUTURES"), domain::AssetClass::Future);
        assert_eq!(asset_class("BONDS"), domain::AssetClass::Bond);
        assert_eq!(asset_class("EQUITIES"), domain::AssetClass::Equity);
        assert_eq!(asset_class("etf"), domain::AssetClass::Equity);
    }

    #[test]
    fn instrument_maps_and_blanks_empty_isin() {
        let a = assets::Asset {
            symbol: "SBER@MISX".into(),
            id: "1".into(),
            ticker: "SBER".into(),
            mic: "MISX".into(),
            isin: String::new(),
            r#type: "EQUITIES".into(),
            name: "Сбербанк".into(),
            is_archived: false,
        };
        let i = instrument(&a);
        assert_eq!(i.symbol, "SBER@MISX");
        assert_eq!(i.ticker, "SBER");
        assert_eq!(i.asset_class, domain::AssetClass::Equity);
        assert_eq!(i.isin, None);
        assert_eq!(i.sector, None);
    }

    #[test]
    fn timeframe_day_maps_to_proto_d() {
        assert_eq!(timeframe(TimeFrame::D1), marketdata::TimeFrame::D);
        assert_eq!(timeframe(TimeFrame::M15), marketdata::TimeFrame::M15);
    }

    #[test]
    fn interval_builds_bounds() {
        let iv = interval(100, 200);
        assert_eq!(iv.start_time.unwrap().seconds, 100);
        assert_eq!(iv.end_time.unwrap().seconds, 200);
    }
}
