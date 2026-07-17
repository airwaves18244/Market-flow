//! Контракт хранилища: что умеет персистентный слой терминала.
//!
//! [`Store`] — единый интерфейс для записи (ингест) и чтения (запросы,
//! питающие аналитику из `domain`). У него две реализации:
//! - [`crate::mem::MemStore`] — в памяти, всегда доступна, целевая для тестов и
//!   CI на любой платформе;
//! - [`crate::duck::DuckStore`] — на нативном DuckDB (фича `duckdb`).
//!
//! Слой намеренно синхронный и не знает про gRPC/tokio: сетевой источник
//! (`data::MarketData`) и асинхронный планировщик живут выше, в `app`. Сюда
//! приходят уже доменные типы.

use domain::algo::{FutoiPoint, Hi2Point, SuperCandle};
use domain::history::{missing_ranges, Catalog, DataSource, DatasetMeta, HistoryBar, TimeRange};
use domain::{Bar, Instrument, TimeFrame, Trade};
use serde::{Deserialize, Serialize};

use crate::StorageError;

/// Снимок агрегированного оборота инструмента на момент сканирования рынка
/// (строка таблицы `turnover_snapshots`). Из последовательности снимков
/// строятся тренды и перетоки во времени.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TurnoverSnapshot {
    /// Момент снимка, UNIX-секунды UTC.
    pub ts: i64,
    /// Накопленный оборот за день (денежный объём).
    pub turnover: f64,
    /// Чистый денежный поток (направленный оборот вверх − вниз).
    pub net_flow: f64,
    /// Дневное изменение в долях (`0.01` = +1%).
    pub change: f64,
}

/// Запись таблицы классификации секторов (`sector_map`).
///
/// Хранилище держит её в «сыром» виде (ключ + сектор + признак ISIN);
/// построение поисковой структуры `data::classify::SectorMap` — задача
/// адаптера `data`, чтобы хранилище не зависело от сетевого слоя.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectorEntry {
    /// Тикер или ISIN.
    pub key: String,
    /// Название сектора.
    pub sector: String,
    /// `true`, если ключ — ISIN (а не тикер).
    pub is_isin: bool,
}

/// Запись датасета ALGOPACK `obstats` (статистика стакана заявок): спред BBO/
/// 10 уровней и дисбаланс объёма/стоимости у лучшей котировки.
///
/// Доменный тип для obstats ещё не выделен в `domain::algo` (см. SPEC `10.2.4`,
/// добавляется параллельно веткой), поэтому storage хранит «сырую» запись по
/// полям датасета напрямую. Когда доменный тип появится, эту структуру можно
/// будет заменить/сопоставить с ним без изменения схемы БД.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlgoObstatsRecord {
    /// Идентификатор инструмента (SECID).
    pub secid: String,
    /// Метка времени, UNIX-секунды UTC.
    pub ts: i64,
    /// Рынок ALGOPACK (`stock`/`currency`/`futures`/`fo`...).
    pub market: String,
    /// Спред лучшей котировки (доли цены).
    pub spread_bbo: f64,
    /// Спред по 10 уровням стакана.
    pub spread_lv10: f64,
    /// Число уровней бид/аск.
    pub levels_b: f64,
    pub levels_s: f64,
    /// Объём в стакане бид/аск.
    pub vol_b: f64,
    pub vol_s: f64,
    /// Стоимость в стакане бид/аск.
    pub val_b: f64,
    pub val_s: f64,
    /// Дисбаланс объёма на BBO (−1..1).
    pub imbalance_vol_bbo: f64,
    /// Дисбаланс стоимости на BBO (−1..1).
    pub imbalance_val_bbo: f64,
}

/// Запись датасета ALGOPACK `orderstats` (статистика заявок): постановка и
/// снятие заявок с разбивкой на покупку/продажу.
///
/// Как и [`AlgoObstatsRecord`], хранит «сырые» поля датасета — доменный тип
/// ещё не выделен (SPEC `10.2.4`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlgoOrderstatsRecord {
    /// Идентификатор инструмента (SECID).
    pub secid: String,
    /// Метка времени, UNIX-секунды UTC.
    pub ts: i64,
    /// Рынок ALGOPACK.
    pub market: String,
    /// Число выставленных заявок на покупку/продажу.
    pub put_orders_b: f64,
    pub put_orders_s: f64,
    /// Стоимость выставленных заявок.
    pub put_val_b: f64,
    pub put_val_s: f64,
    /// Объём выставленных заявок.
    pub put_vol_b: f64,
    pub put_vol_s: f64,
    /// Число снятых заявок.
    pub cancel_orders_b: f64,
    pub cancel_orders_s: f64,
    /// Стоимость снятых заявок.
    pub cancel_val_b: f64,
    pub cancel_val_s: f64,
    /// Объём снятых заявок.
    pub cancel_vol_b: f64,
    pub cancel_vol_s: f64,
}

/// Персистентный слой: ингест рыночных данных и аналитические запросы.
///
/// Записи идемпотентны по первичным ключам схемы (`INSERT OR REPLACE`):
/// повторный ингест того же бара/снимка не плодит дублей.
pub trait Store {
    /// Применить миграции (создать таблицы, зафиксировать версию схемы).
    /// Идемпотентна — безопасно вызывать при каждом старте.
    fn migrate(&mut self) -> Result<(), StorageError>;

    /// Текущая версия схемы (`None`, если БД ещё не мигрировали).
    fn schema_version(&self) -> Result<Option<i32>, StorageError>;

    /// Вставить/обновить инструменты справочника. Возвращает число строк.
    fn upsert_instruments(&mut self, items: &[Instrument]) -> Result<usize, StorageError>;

    /// Все инструменты справочника (порядок не гарантируется).
    fn instruments(&self) -> Result<Vec<Instrument>, StorageError>;

    /// Вставить бары инструмента в заданном тайм-фрейме. Возвращает число строк.
    fn insert_bars(
        &mut self,
        symbol: &str,
        tf: TimeFrame,
        bars: &[Bar],
    ) -> Result<usize, StorageError>;

    /// Бары инструмента в `[from_ts, to_ts]` (включительно), по возрастанию `ts`.
    fn bars(
        &self,
        symbol: &str,
        tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<Bar>, StorageError>;

    /// Время последнего сохранённого бара (для планирования бэкфилла).
    fn last_bar_ts(&self, symbol: &str, tf: TimeFrame) -> Result<Option<i64>, StorageError>;

    /// Вставить снимок оборота инструмента.
    fn insert_snapshot(
        &mut self,
        symbol: &str,
        snap: &TurnoverSnapshot,
    ) -> Result<(), StorageError>;

    /// Снимки оборота инструмента в `[from_ts, to_ts]`, по возрастанию `ts`.
    fn snapshots(
        &self,
        symbol: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<TurnoverSnapshot>, StorageError>;

    /// Дописать обезличенные сделки (тиковую ленту) инструмента. Append-only:
    /// каждый тик — отдельная строка (в отличие от баров/снимков, тики не
    /// перезаписываются по ключу). Возвращает число записанных строк.
    fn insert_trades(&mut self, symbol: &str, trades: &[Trade]) -> Result<usize, StorageError>;

    /// Сделки инструмента в `[from_ts, to_ts]` (включительно), по возрастанию
    /// `ts`; внутри одной секунды сохраняется порядок поступления.
    fn trades(&self, symbol: &str, from_ts: i64, to_ts: i64) -> Result<Vec<Trade>, StorageError>;

    /// Заменить/дополнить таблицу классификации секторов. Возвращает число строк.
    fn upsert_sector_map(&mut self, entries: &[SectorEntry]) -> Result<usize, StorageError>;

    /// Все записи классификации секторов.
    fn sector_map(&self) -> Result<Vec<SectorEntry>, StorageError>;

    /// Вставить/обновить свечи Super Candles (`algo_tradestats`) для рынка
    /// `market`. Идемпотентно по ключу (secid, ts, market). Возвращает число
    /// строк.
    fn insert_algo_tradestats(
        &mut self,
        market: &str,
        candles: &[SuperCandle],
    ) -> Result<usize, StorageError>;

    /// Свечи Super Candles инструмента `secid` на рынке `market` в
    /// `[from_ts, to_ts]` (включительно), по возрастанию `ts`.
    fn algo_tradestats(
        &self,
        market: &str,
        secid: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<SuperCandle>, StorageError>;

    /// Вставить/обновить точки FUTOI (`algo_futoi`) для рынка `market`.
    /// Идемпотентно по ключу (secid, ts, market, clgroup). Возвращает число
    /// строк.
    fn insert_algo_futoi(
        &mut self,
        market: &str,
        points: &[FutoiPoint],
    ) -> Result<usize, StorageError>;

    /// Точки FUTOI инструмента `secid` на рынке `market` в `[from_ts, to_ts]`,
    /// по возрастанию `ts` (все группы клиентов).
    fn algo_futoi(
        &self,
        market: &str,
        secid: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<FutoiPoint>, StorageError>;

    /// Вставить/обновить точки HI2 (`algo_hi2`) для рынка `market`.
    /// Идемпотентно по ключу (secid, ts, market). Возвращает число строк.
    fn insert_algo_hi2(&mut self, market: &str, points: &[Hi2Point])
        -> Result<usize, StorageError>;

    /// Точки HI2 инструмента `secid` на рынке `market` в `[from_ts, to_ts]`,
    /// по возрастанию `ts`.
    fn algo_hi2(
        &self,
        market: &str,
        secid: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<Hi2Point>, StorageError>;

    /// Последняя (по `ts`) точка HI2 инструмента `secid` на рынке `market`;
    /// `None`, если по ключу нет ни одной точки.
    ///
    /// Отдельный метод, а не `algo_hi2(..).last()` — реализации читают только
    /// последнюю строку по ключу, не поднимая всю историю: панели вроде
    /// ранжирования по концентрации (T11, `MoexAlgoTab`) опрашивают вотчлист
    /// из десятка тикеров, и раньше каждый такой запрос тянул полный диапазон
    /// ради последней точки.
    fn algo_hi2_latest(&self, market: &str, secid: &str) -> Result<Option<Hi2Point>, StorageError>;

    /// Вставить/обновить записи OBSTATS (`algo_obstats`). Идемпотентно по
    /// ключу (secid, ts, market). Возвращает число строк.
    fn insert_algo_obstats(&mut self, records: &[AlgoObstatsRecord])
        -> Result<usize, StorageError>;

    /// Записи OBSTATS инструмента `secid` на рынке `market` в
    /// `[from_ts, to_ts]`, по возрастанию `ts`.
    fn algo_obstats(
        &self,
        market: &str,
        secid: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<AlgoObstatsRecord>, StorageError>;

    /// Вставить/обновить записи ORDERSTATS (`algo_orderstats`). Идемпотентно
    /// по ключу (secid, ts, market). Возвращает число строк.
    fn insert_algo_orderstats(
        &mut self,
        records: &[AlgoOrderstatsRecord],
    ) -> Result<usize, StorageError>;

    /// Записи ORDERSTATS инструмента `secid` на рынке `market` в
    /// `[from_ts, to_ts]`, по возрастанию `ts`.
    fn algo_orderstats(
        &self,
        market: &str,
        secid: &str,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<AlgoOrderstatsRecord>, StorageError>;

    /// Вставить/обновить исторические бары (таблица `history_bars`).
    /// Идемпотентно по ключу (source, secid, tf, ts): повторный ингест того же
    /// бара перезаписывает строку, а не плодит дубли. Источник/тайм-фрейм берутся
    /// из самих баров, поэтому один вызов может нести бары разных ключей.
    /// Возвращает число обработанных строк.
    fn insert_history_bars(&mut self, bars: &[HistoryBar]) -> Result<usize, StorageError>;

    /// Исторические бары ключа (source, secid, tf) в `[from_ts, to_ts]`
    /// (включительно), по возрастанию `ts`.
    fn history_bars(
        &self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<Vec<HistoryBar>, StorageError>;

    /// Время последнего сохранённого исторического бара ключа (для планирования
    /// инкрементальной дозагрузки через [`crate::backfill::plan_backfill`]).
    fn last_history_bar_ts(
        &self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
    ) -> Result<Option<i64>, StorageError>;

    /// Удалить все исторические бары ключа (source, secid, tf). Возвращает число
    /// удалённых строк. Каталог (`history_datasets`) не затрагивается — его
    /// чистит вызывающий слой (см. [`remove_dataset`](Store::remove_dataset)).
    fn delete_history_bars(
        &mut self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
    ) -> Result<usize, StorageError>;

    /// Число исторических баров ключа (source, secid, tf) в `[from_ts, to_ts]`
    /// (включительно) без их материализации — для пересчёта метаданных датасета.
    fn count_history_bars(
        &self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
        from_ts: i64,
        to_ts: i64,
    ) -> Result<u64, StorageError>;

    /// Вставить/обновить метаданные датасета в каталоге (`history_datasets`).
    /// Идемпотентно по ключу (source, secid, tf) — строка перезаписывается
    /// целиком. Слияние диапазонов (при необходимости) выполняет вызывающий слой
    /// через [`domain::history::Catalog::upsert`].
    fn upsert_dataset(&mut self, meta: &DatasetMeta) -> Result<(), StorageError>;

    /// Все датасеты каталога (порядок не гарантируется).
    fn datasets(&self) -> Result<Vec<DatasetMeta>, StorageError>;

    /// Метаданные датасета по ключу (`None`, если датасета нет).
    fn dataset(
        &self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
    ) -> Result<Option<DatasetMeta>, StorageError>;

    /// Удалить датасет из каталога по ключу; `true`, если строка удалена.
    /// Сами бары (`history_bars`) не затрагиваются — их чистит вызывающий слой,
    /// если нужно.
    fn remove_dataset(
        &mut self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
    ) -> Result<bool, StorageError>;

    /// Собрать доменный каталог из строк `history_datasets`.
    fn catalog(&self) -> Result<Catalog, StorageError> {
        Ok(Catalog {
            datasets: self.datasets()?,
        })
    }

    /// Недостающие подсегменты `requested`, ещё не покрытые ключом
    /// (source, secid, tf) — план инкрементальной дозагрузки. Покрытие считается
    /// по фактическим барам (`history_bars`), а не по огибающему `range`
    /// каталога: каждый сохранённый бар покрывает `[ts, ts + tf.seconds())`,
    /// смежные бары сливаются, а внутренние дыры (несмежные догрузки) корректно
    /// попадают в план. Дыры считает [`domain::history::missing_ranges`];
    /// пустой результат — всё покрыто.
    fn history_missing_ranges(
        &self,
        source: DataSource,
        secid: &str,
        tf: TimeFrame,
        requested: TimeRange,
    ) -> Result<Vec<TimeRange>, StorageError> {
        if requested.is_empty() {
            return Ok(Vec::new());
        }
        let step = tf.seconds().max(1);
        let covered: Vec<TimeRange> = self
            .history_bars(source, secid, tf, requested.from, requested.till)?
            .into_iter()
            .map(|b| TimeRange::new(b.ts, b.ts + step))
            .collect();
        Ok(missing_ranges(requested, &covered))
    }

    /// Все инструменты заданного класса активов.
    fn instruments_by_asset_class(
        &self,
        asset_class: &str,
    ) -> Result<Vec<Instrument>, StorageError> {
        let all = self.instruments()?;
        Ok(all
            .into_iter()
            .filter(|i| i.asset_class.code() == asset_class)
            .collect())
    }
}
