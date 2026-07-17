# Фикстуры MOEX ALGOPACK (ISS JSON)

**Статус: сверено живым ключом `MOEX_ALGO_API` (T14, 2026-07-17).**
Все файлы приведены к форме живых ответов; смоук — `example algopack_check`
(`cargo run -p data --features "moex,llm" --example algopack_check`):
tradestats/orderstats/obstats/hi2/futoi/candles по одному живому запросу
через боевой клиент. Итоги сверки:

- **Блок и пагинация datashop.** `tradestats`/`orderstats`/`obstats`/`hi2`
  оборачиваются в блок `data` + курсор `data.cursor`
  (`INDEX`/`TOTAL`/`PAGESIZE`, PAGESIZE=1000) + блок `data.dates`
  (доступный период). Параметры `from`/`till`/`start`/`iss.meta=off`
  работают. Колонки строчными, кроме `SYSTIME`; `SYSTIME` — строка
  `YYYY-MM-DD HH:MM:SS` (не unix) — время берётся из
  `tradedate`+`tradetime` (MSK).
- **`futoi` живёт вне `datashop`**: ресурс
  `iss/analyticalproducts/futoi/securities/{ticker}.json` (тикер — код
  актива: `Si`, `RTS`; путь `datashop/.../fo/futoi` — 404). Блок `futoi` +
  `futoi.dates`, инструмент — колонка **`ticker`** (не `secid`), `clgroup`
  ЗАГЛАВНЫМИ (`FIZ`/`YUR`), short-позиции отрицательные.
- **`hi2` — длинный формат**: строка на пару (инструмент, метрика), колонки
  `metric`/`value`/`reference`; метрики — семейство `hhi_*`
  (`hhi_volume`, `hhi_buy`, `hhi_sell`, `hhi_agressive[_buy|_sell]`,
  `hhi_passive[_buy|_sell]`, `hhi_netflow_[buy|sell]`) в шкале `0..10 000`.
  Парсер берёт `hhi_volume` и нормирует к `0..1`.
- **Свечей в `datashop` нет** (404): живой ресурс — стандартный ISS
  `iss/engines/{engine}/markets/{market}/securities/{SECID}/candles.json`
  с параметром `interval` (`1`/`10`/`60` мин, `24` день, `7` неделя,
  `31` месяц); блок `candles`, колонки
  `open/close/high/low/value/volume/begin/end`, время — `begin`
  (`YYYY-MM-DD HH:MM:SS`, MSK).
- **Открытый вопрос (единицы)**: живые `spread_bbo`/`spread_lv10`/
  `spread_1mio` в `obstats` приходят величинами порядка `1.8`/`13.3` —
  похоже на б.п., а не долю; пороги Mega Alerts трактуют спред как
  относительную величину — перепроверить при выводе живого obstats в
  аналитику. Структура собрана по публичной документации ISS
(`iss.moex.com/iss/reference/`) и по описанию датасетов ALGOPACK в открытой
документации `moexalgo`/data.moex.com, но **точные имена колонок, регистр,
имя блока-обёртки и параметры запроса не подтверждены живым ответом API**.
Парсер (`crates/data/src/moex/parse.rs`) остаётся устойчивым к вариациям
(имена блоков — кандидатами, колонки — по имени без учёта регистра, время —
`ts`/`systime` → `tradedate`+`tradetime` → `begin`), поэтому изменения формы
на стороне биржи деградируют мягко, а не паникой.

## Файлы

| Файл | Датасет | Ресурс | Особенность |
|---|---|---|---|
| `tradestats_eq.json` | tradestats (Super Candles) | `datashop/algopack/eq` | 2 строки, 1 страница |
| `futoi_fo.json` | futoi | `analyticalproducts/futoi/securities` | блок `futoi`, колонка `ticker`, `FIZ`/`YUR` + неизвестная группа (пропускается) |
| `hi2_eq.json` | hi2 (длинный формат) | `datashop/algopack/eq` | `hhi_volume` берётся, прочие `hhi_*` пропускаются |
| `obstats_eq.json` | obstats | `datashop/algopack/eq` | вторая строка без `spread_1mio` (`null`) — мягкий `Option` |
| `orderstats_eq.json` | orderstats | `datashop/algopack/eq` | вторая строка без `cancel_*` (`null`) — мягкий `Option` |
| `candles_eq.json` | свечи | `engines/stock/markets/shares` | время из `begin` (MSK) |
| `options_board.json` | опционная доска (фаза 12.4) | `engines/futures/markets/options` | см. раздел ниже |
| `options_board_empty.json` | опционная доска | — | пустые `securities`/`marketdata` (без строк) |

## Как перепроверить живым ключом

`MOEX_ALGO_API` (подписка ALGOPACK на data.moex.com → APIKEY) и
`OPENROUTER_API_KEY` в `.env` (см. `.env.example`), затем:

```bash
cargo run -p data --features "moex,llm" --example algopack_check
```

Смоук дёргает каждый датасет одним живым запросом через боевой клиент и
один вызов LLM; при расхождении формы — правки в `parse.rs`/`client.rs` и
обновление файлов этого каталога.

## Опционная доска (`options.rs`, фаза 12.4) — **сверено живым ответом (T14, 2026-07-17)**

В отличие от файлов выше (ALGOPACK, `apim.moex.com`, Bearer-токен), опционная
доска читается с **публичного** ISS `iss.moex.com` (не требует авторизации).
`options_board*.json` приведены к форме живого ответа. Итоги сверки:

- **Ресурс.** `engines/futures/markets/options/securities.json?iss.only=securities,marketdata&iss.meta=off`
  — подтверждён: блоки `securities` и `marketdata` в одном ответе, строки
  выровнены по `SECID`, колонки ЗАГЛАВНЫМИ.
- **Колонки `securities`** (29 в живом ответе; в фикстурах — значимое
  подмножество): `SECID`, `BOARDID` (`ROPD`), `SHORTNAME`,
  `PREVSETTLEPRICE`, `LASTTRADEDATE` (`YYYY-MM-DD` — дата экспирации),
  `ASSETCODE` (код актива, напр. `AFLT`), `OPTIONTYPE` (`C`/`P`), `STRIKE`,
  `CENTRALSTRIKE`, `UNDERLYINGASSET` (**SECID фьючерса**, напр. `AFU6`),
  `UNDERLYINGTYPE` (`F`), `UNDERLYINGSETTLEPRICE`.
- **Колонки `marketdata`** (33 в живом ответе): `BID`, `OFFER`, `LAST`,
  `SETTLEPRICE`, `OPENPOSITION`, `SYSTIME`, `TRADE_SESSION_DATE`, ...
  **`IV` и `THEORPRICE` в живом ответе ОТСУТСТВУЮТ** — IV решается из цены
  (`theor_price` → mid `bid`/`ask` → `last`; первые два пункта — задел на
  случай появления расчёта биржи, живой путь — mid).
- **Серверная фильтрация.** Параметр `assets=<код актива>` работает
  (`assets=AFLT` → 138 строк вместо 34 420 / ~19 МБ всего рынка); варианты
  `assetcode=`/`asset_code=` игнорируются. Клиент шлёт `assets=` и сохраняет
  клиентскую фильтрацию по `ASSETCODE` как страховку.
- **Пагинация.** Полный ответ (34 тыс. строк) приходит **одной страницей без
  блока `securities.cursor`** — `IssCursor::find` возвращает `None`, клиент
  не пагинирует; поддержка курсора оставлена защитным клапаном.
- **Форвард.** `forts/securities/{SECID}.json` работает **только по SECID
  фьючерса** из `UNDERLYINGASSET` (`AFU6` → строка с `LAST`/`SETTLEPRICE`);
  по коду актива (`AFLT`) — 0 строк. Порядок в `options_board_snapshot`:
  `LAST`/`SETTLEPRICE` с forts по `UNDERLYINGASSET`, иначе
  `UNDERLYINGSETTLEPRICE` из самой доски, иначе `None` (тогда `app`
  подставляет `forward_hint`).
- **`options_board.json`.** Актив `RTS` (фьючерс `RIH5`; 4 валидных строки:
  50000 call/put, 55000 call, 45000 call) + строка другого актива `Si`
  (проверяет фильтрацию) + строка без страйка (`null`, мягкое отбрасывание).
  Пут 50000 — без `bid`/`ask`/OI (неликвид, отбрасывается маппингом в точки
  улыбки).
- **`options_board_empty.json`.** Пустые `securities`/`marketdata` — доска
  без данных не должна ронять парсер/маппинг.
