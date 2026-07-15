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

/// Super Candles (датасет ALGOPACK `tradestats`) — расширенная 5-минутная
/// свеча с разбивкой потока покупок/продаж. Ключ включает `market`, т.к.
/// один и тот же SECID может встречаться на разных рынках ALGOPACK
/// (`stock`/`currency`/`futures`/`fo`...).
pub const DDL_ALGO_TRADESTATS: &str = "\
CREATE TABLE IF NOT EXISTS algo_tradestats (
    secid       TEXT NOT NULL,
    ts          BIGINT NOT NULL,     -- начало 5-мин интервала, UNIX-секунды UTC
    market      TEXT NOT NULL,       -- рынок ALGOPACK (stock|currency|futures|fo...)
    pr_open     DOUBLE NOT NULL,
    pr_high     DOUBLE NOT NULL,
    pr_low      DOUBLE NOT NULL,
    pr_close    DOUBLE NOT NULL,
    pr_std      DOUBLE NOT NULL,     -- стандартное отклонение цены внутри интервала
    vol         DOUBLE NOT NULL,
    val         DOUBLE NOT NULL,
    trades      DOUBLE NOT NULL,
    pr_vwap     DOUBLE NOT NULL,
    pr_change   DOUBLE NOT NULL,
    vol_b       DOUBLE NOT NULL,     -- объём покупок
    vol_s       DOUBLE NOT NULL,     -- объём продаж
    val_b       DOUBLE NOT NULL,
    val_s       DOUBLE NOT NULL,
    trades_b    DOUBLE NOT NULL,
    trades_s    DOUBLE NOT NULL,
    disb        DOUBLE NOT NULL,     -- дисбаланс потока (−1..1)
    pr_vwap_b   DOUBLE NOT NULL,
    pr_vwap_s   DOUBLE NOT NULL,
    PRIMARY KEY (secid, ts, market)
);";

/// FUTOI (датасет ALGOPACK `futoi`, рынок `fo`) — открытый интерес по группе
/// клиентов (`clgroup`: fiz|yur). Ключ включает `clgroup`, т.к. в один момент
/// `ts` по инструменту есть отдельная строка на каждую группу.
pub const DDL_ALGO_FUTOI: &str = "\
CREATE TABLE IF NOT EXISTS algo_futoi (
    secid           TEXT NOT NULL,
    ts              BIGINT NOT NULL,
    market          TEXT NOT NULL,       -- рынок ALGOPACK (fo)
    clgroup         TEXT NOT NULL,       -- группа клиентов: fiz|yur
    pos             DOUBLE NOT NULL,     -- суммарная позиция (контрактов)
    pos_long        DOUBLE NOT NULL,
    pos_short       DOUBLE NOT NULL,
    pos_long_num    DOUBLE NOT NULL,     -- число длинных позиций (участников)
    pos_short_num   DOUBLE NOT NULL,
    PRIMARY KEY (secid, ts, market, clgroup)
);";

/// HI2 (датасет ALGOPACK `hi2`) — индекс концентрации участников потока.
pub const DDL_ALGO_HI2: &str = "\
CREATE TABLE IF NOT EXISTS algo_hi2 (
    secid          TEXT NOT NULL,
    ts             BIGINT NOT NULL,
    market         TEXT NOT NULL,
    concentration  DOUBLE NOT NULL,      -- индекс Херфиндаля-подобный, 0..1
    PRIMARY KEY (secid, ts, market)
);";

/// OBSTATS (датасет ALGOPACK `obstats`) — статистика стакана заявок:
/// спред BBO/10 уровней и дисбаланс объёма/стоимости у лучшей котировки.
/// Доменный тип ещё не выделен (см. `10.2.4`), поэтому storage хранит запись
/// «сырых» полей напрямую — см. [`crate::store::AlgoObstatsRecord`].
pub const DDL_ALGO_OBSTATS: &str = "\
CREATE TABLE IF NOT EXISTS algo_obstats (
    secid               TEXT NOT NULL,
    ts                  BIGINT NOT NULL,
    market              TEXT NOT NULL,
    spread_bbo          DOUBLE NOT NULL,  -- спред лучшей котировки (доли цены)
    spread_lv10         DOUBLE NOT NULL,  -- спред по 10 уровням стакана
    levels_b            DOUBLE NOT NULL,  -- число уровней бид/аск
    levels_s            DOUBLE NOT NULL,
    vol_b               DOUBLE NOT NULL,  -- объём в стакане бид/аск
    vol_s               DOUBLE NOT NULL,
    val_b               DOUBLE NOT NULL,  -- стоимость в стакане бид/аск
    val_s               DOUBLE NOT NULL,
    imbalance_vol_bbo   DOUBLE NOT NULL,  -- дисбаланс объёма на BBO (−1..1)
    imbalance_val_bbo   DOUBLE NOT NULL,  -- дисбаланс стоимости на BBO (−1..1)
    PRIMARY KEY (secid, ts, market)
);";

/// ORDERSTATS (датасет ALGOPACK `orderstats`) — статистика заявок:
/// постановка/снятие заявок с разбивкой на покупку/продажу. Доменный тип ещё
/// не выделен (см. `10.2.4`) — см. [`crate::store::AlgoOrderstatsRecord`].
pub const DDL_ALGO_ORDERSTATS: &str = "\
CREATE TABLE IF NOT EXISTS algo_orderstats (
    secid            TEXT NOT NULL,
    ts               BIGINT NOT NULL,
    market           TEXT NOT NULL,
    put_orders_b     DOUBLE NOT NULL,     -- число выставленных заявок на покупку
    put_orders_s     DOUBLE NOT NULL,
    put_val_b        DOUBLE NOT NULL,     -- стоимость выставленных заявок
    put_val_s        DOUBLE NOT NULL,
    put_vol_b        DOUBLE NOT NULL,     -- объём выставленных заявок
    put_vol_s        DOUBLE NOT NULL,
    cancel_orders_b  DOUBLE NOT NULL,     -- число снятых заявок
    cancel_orders_s  DOUBLE NOT NULL,
    cancel_val_b     DOUBLE NOT NULL,
    cancel_val_s     DOUBLE NOT NULL,
    cancel_vol_b     DOUBLE NOT NULL,
    cancel_vol_s     DOUBLE NOT NULL,
    PRIMARY KEY (secid, ts, market)
);";

/// Исторические бары для бэктестера (фаза 11.2): OHLCV плюс опциональные
/// ALGOPACK-колонки (VWAP/дисбаланс/открытый интерес/индекс концентрации).
/// Ключ включает `source` и `tf`, чтобы датасеты разных источников и тайм-
/// фреймов не смешивались (см. [`domain::history::HistoryBar`]). Опциональные
/// поля — `NULL`, если источник их не отдаёт (например, Finam OHLCV).
pub const DDL_HISTORY_BARS: &str = "\
CREATE TABLE IF NOT EXISTS history_bars (
    source  TEXT NOT NULL,          -- finam|moex_algo (DataSource::code)
    secid   TEXT NOT NULL,
    tf      TEXT NOT NULL,          -- m1|m5|m15|h1|d1
    ts      BIGINT NOT NULL,        -- начало бара, UNIX-секунды UTC
    open    DOUBLE NOT NULL,
    high    DOUBLE NOT NULL,
    low     DOUBLE NOT NULL,
    close   DOUBLE NOT NULL,
    volume  DOUBLE NOT NULL,
    vwap    DOUBLE,                 -- ALGOPACK: средневзвешенная цена
    disb    DOUBLE,                 -- ALGOPACK: дисбаланс потока (−1..1)
    oi      DOUBLE,                 -- ALGOPACK: открытый интерес
    hi2     DOUBLE,                 -- ALGOPACK: индекс концентрации
    PRIMARY KEY (source, secid, tf, ts)
);";

/// Каталог локальных исторических датасетов (фаза 11.2): персист
/// [`domain::history::DatasetMeta`] плюс размер на диске. Ключ — (source, secid,
/// tf); покрытый диапазон хранится как `[from_ts, till_ts)`.
pub const DDL_HISTORY_DATASETS: &str = "\
CREATE TABLE IF NOT EXISTS history_datasets (
    source      TEXT NOT NULL,      -- finam|moex_algo
    secid       TEXT NOT NULL,
    tf          TEXT NOT NULL,      -- m1|m5|m15|h1|d1
    from_ts     BIGINT NOT NULL,    -- нижняя граница покрытия (включительно)
    till_ts     BIGINT NOT NULL,    -- верхняя граница покрытия (исключительно)
    bars        BIGINT NOT NULL,    -- число баров в датасете
    size_bytes  BIGINT NOT NULL,    -- размер на диске (например, Parquet-экспорт)
    updated_ts  BIGINT NOT NULL,    -- время последнего обновления, UNIX-секунды
    PRIMARY KEY (source, secid, tf)
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
/// v3 — датасеты MOEX ALGOPACK: `algo_tradestats`/`algo_futoi`/`algo_hi2`/
/// `algo_obstats`/`algo_orderstats` (фаза 10.5).
/// v4 — историзация бэктестера: `history_bars` (OHLCV + опц. ALGOPACK) и
/// `history_datasets` (каталог локальных датасетов) (фаза 11.2).
pub const SCHEMA_VERSION: i32 = 4;

/// Полный набор DDL таблиц данных в порядке применения. Версия схемы
/// (`schema_version`) применяется отдельно миграцией.
pub const ALL_DDL: [&str; 12] = [
    DDL_INSTRUMENTS,
    DDL_BARS,
    DDL_TURNOVER_SNAPSHOTS,
    DDL_TRADES,
    DDL_SECTOR_MAP,
    DDL_ALGO_TRADESTATS,
    DDL_ALGO_FUTOI,
    DDL_ALGO_HI2,
    DDL_ALGO_OBSTATS,
    DDL_ALGO_ORDERSTATS,
    DDL_HISTORY_BARS,
    DDL_HISTORY_DATASETS,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ddl_is_present_and_keyed() {
        assert_eq!(ALL_DDL.len(), 12);
        for ddl in ALL_DDL {
            assert!(ddl.contains("CREATE TABLE IF NOT EXISTS"));
        }
        assert!(DDL_BARS.contains("PRIMARY KEY (symbol, timeframe, ts)"));
        assert!(DDL_TRADES.contains("buyer_initiated"));
    }

    #[test]
    fn history_ddl_keys_and_optional_columns() {
        assert!(DDL_HISTORY_BARS.contains("PRIMARY KEY (source, secid, tf, ts)"));
        // Опциональные ALGOPACK-колонки без NOT NULL.
        for col in ["vwap", "disb", "oi", "hi2"] {
            assert!(DDL_HISTORY_BARS.contains(col));
        }
        assert!(DDL_HISTORY_DATASETS.contains("PRIMARY KEY (source, secid, tf)"));
        assert!(DDL_HISTORY_DATASETS.contains("size_bytes"));
    }

    #[test]
    fn algo_ddl_keys_include_secid_ts_market() {
        assert!(DDL_ALGO_TRADESTATS.contains("PRIMARY KEY (secid, ts, market)"));
        assert!(DDL_ALGO_FUTOI.contains("PRIMARY KEY (secid, ts, market, clgroup)"));
        assert!(DDL_ALGO_HI2.contains("PRIMARY KEY (secid, ts, market)"));
        assert!(DDL_ALGO_OBSTATS.contains("PRIMARY KEY (secid, ts, market)"));
        assert!(DDL_ALGO_ORDERSTATS.contains("PRIMARY KEY (secid, ts, market)"));
    }

    #[test]
    fn schema_version_ddl_present() {
        assert!(DDL_SCHEMA_VERSION.contains("schema_version"));
        assert!(DDL_SCHEMA_VERSION.contains("version"));
    }
}
