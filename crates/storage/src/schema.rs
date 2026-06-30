//! DDL схемы DuckDB. Применяется при инициализации хранилища.
//!
//! Время хранится в UNIX-секундах UTC (`BIGINT`), как отдаёт Finam Trade API;
//! класс актива — `equity|future|bond` (см. [`domain::AssetClass::code`]).

/// Справочник инструментов (обновляется из `AssetsService`).
pub const DDL_INSTRUMENTS: &str = "\
CREATE TABLE IF NOT EXISTS instruments (
    symbol       TEXT PRIMARY KEY,   -- ticker@mic, напр. SBER@MISX
    ticker       TEXT NOT NULL,
    name         TEXT NOT NULL,
    asset_class  TEXT NOT NULL,      -- equity|future|bond
    sector       TEXT,               -- из таблицы классификации
    lot_size     INTEGER NOT NULL,
    isin         TEXT,
    updated_at   BIGINT NOT NULL
);";

/// Бары (свечи) — основа для оборота, money flow и графиков.
pub const DDL_BARS: &str = "\
CREATE TABLE IF NOT EXISTS bars (
    symbol     TEXT NOT NULL,
    timeframe  TEXT NOT NULL,        -- m1|m5|m15|h1|d1
    ts         BIGINT NOT NULL,
    open       DOUBLE NOT NULL,
    high       DOUBLE NOT NULL,
    low        DOUBLE NOT NULL,
    close      DOUBLE NOT NULL,
    volume     DOUBLE NOT NULL,
    PRIMARY KEY (symbol, timeframe, ts)
);";

/// Снимки агрегированного оборота по инструменту на момент сканирования рынка.
/// Из них строятся тренды и перетоки во времени.
pub const DDL_TURNOVER_SNAPSHOTS: &str = "\
CREATE TABLE IF NOT EXISTS turnover_snapshots (
    symbol      TEXT NOT NULL,
    ts          BIGINT NOT NULL,     -- момент снимка
    turnover    DOUBLE NOT NULL,     -- накопленный оборот за день
    net_flow    DOUBLE NOT NULL,
    change      DOUBLE NOT NULL,     -- дневное изменение, доли
    PRIMARY KEY (symbol, ts)
);";

/// Обезличенные сделки (тиковая лента) — основа footprint/дельты и заполнения
/// симулятора исполнения. Append-only: один тик — одна строка. `buyer_initiated`
/// (сторона-агрессор) может быть `NULL`, если биржа её не отдаёт.
pub const DDL_TRADES: &str = "\
CREATE TABLE IF NOT EXISTS trades (
    symbol           TEXT NOT NULL,
    ts               BIGINT NOT NULL,    -- время сделки, UNIX-секунды UTC
    price            DOUBLE NOT NULL,
    size             DOUBLE NOT NULL,
    buyer_initiated  BOOLEAN             -- true=покупка(агрессор-бид), NULL=неизвестно
);";

/// Редактируемая таблица классификации секторов (тикер/ISIN → сектор).
pub const DDL_SECTOR_MAP: &str = "\
CREATE TABLE IF NOT EXISTS sector_map (
    key     TEXT PRIMARY KEY,        -- тикер или ISIN
    sector  TEXT NOT NULL,
    is_isin BOOLEAN NOT NULL
);";

/// Однострочная таблица версии схемы — основа идемпотентных миграций.
pub const DDL_SCHEMA_VERSION: &str = "\
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER NOT NULL
);";

/// Текущая версия схемы. Повышается при изменении DDL, чтобы [`crate::migrate`]
/// знал, нужно ли применять обновления к существующей БД.
///
/// v2 — добавлена таблица `trades` (тиковая лента для footprint/дельты и
/// симулятора исполнения).
pub const SCHEMA_VERSION: i32 = 2;

/// Полный набор DDL таблиц данных в порядке применения. Версия схемы
/// (`schema_version`) применяется отдельно миграцией.
pub const ALL_DDL: [&str; 5] = [
    DDL_INSTRUMENTS,
    DDL_BARS,
    DDL_TURNOVER_SNAPSHOTS,
    DDL_TRADES,
    DDL_SECTOR_MAP,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ddl_is_present_and_keyed() {
        assert_eq!(ALL_DDL.len(), 5);
        for ddl in ALL_DDL {
            assert!(ddl.contains("CREATE TABLE IF NOT EXISTS"));
        }
        assert!(DDL_BARS.contains("PRIMARY KEY (symbol, timeframe, ts)"));
        assert!(DDL_TRADES.contains("buyer_initiated"));
    }

    #[test]
    fn schema_version_ddl_present() {
        assert!(DDL_SCHEMA_VERSION.contains("schema_version"));
        assert!(DDL_SCHEMA_VERSION.contains("version"));
    }
}
