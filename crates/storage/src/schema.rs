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

/// Редактируемая таблица классификации секторов (тикер/ISIN → сектор).
pub const DDL_SECTOR_MAP: &str = "\
CREATE TABLE IF NOT EXISTS sector_map (
    key     TEXT PRIMARY KEY,        -- тикер или ISIN
    sector  TEXT NOT NULL,
    is_isin BOOLEAN NOT NULL
);";

/// Журнал применённых миграций (версионирование схемы).
pub const DDL_SCHEMA_MIGRATIONS: &str = "\
CREATE TABLE IF NOT EXISTS schema_migrations (
    version     BIGINT PRIMARY KEY,
    applied_at  BIGINT NOT NULL
);";

/// Полный набор DDL в порядке применения.
pub const ALL_DDL: [&str; 4] = [
    DDL_INSTRUMENTS,
    DDL_BARS,
    DDL_TURNOVER_SNAPSHOTS,
    DDL_SECTOR_MAP,
];

/// Одна миграция схемы: номер версии и упорядоченный список DDL-операторов.
pub struct Migration {
    pub version: i64,
    pub statements: &'static [&'static str],
}

/// Упорядоченные миграции. Накат идемпотентен: применяются только версии,
/// которых ещё нет в `schema_migrations`. Новые изменения схемы добавляются
/// следующими элементами с возрастающей версией (старые не редактируются).
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    statements: &ALL_DDL,
}];

/// Целевая (последняя) версия схемы.
pub fn target_version() -> i64 {
    MIGRATIONS.iter().map(|m| m.version).max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ddl_is_present_and_keyed() {
        assert_eq!(ALL_DDL.len(), 4);
        for ddl in ALL_DDL {
            assert!(ddl.contains("CREATE TABLE IF NOT EXISTS"));
        }
        assert!(DDL_BARS.contains("PRIMARY KEY (symbol, timeframe, ts)"));
    }

    #[test]
    fn migrations_are_monotonic_and_target_is_last() {
        let mut prev = 0;
        for m in MIGRATIONS {
            assert!(m.version > prev, "версии миграций должны возрастать");
            prev = m.version;
        }
        assert_eq!(target_version(), prev);
    }
}
