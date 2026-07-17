# Фикстуры MOEX ALGOPACK (ISS JSON)

**Статус ALGOPACK-файлов: unverified — требуют сверки живым ключом
`MOEX_ALGO_API`.** Живой смоук T14 (2026-07-17): egress к `apim.moex.com`
открыт, эндпоинт подтверждён (`GET /iss/datashop/algopack/eq/tradestats/SBER.json`
без ключа → `401 {"message":"Unauthorized"}` — URL и JSON-обёртка ошибок
живые), но ключа `MOEX_ALGO_API` в окружении нет, поэтому форма успешного
ответа по-прежнему не сверена. Файлы `options_board*.json` (публичный ISS)
сверены живым ответом — см. раздел «Опционная доска».

ALGOPACK-файлы синтетические. Структура собрана по публичной документации ISS
(`iss.moex.com/iss/reference/`) и по описанию датасетов ALGOPACK в открытой
документации `moexalgo`/data.moex.com, но **точные имена колонок, регистр,
имя блока-обёртки и параметры запроса не подтверждены живым ответом API**.
Парсер (`crates/data/src/moex/parse.rs`) намеренно устойчив к расхождениям:

- **Имя блока.** ISS обычно оборачивает таблицу в блок `data`
  (`{"data": {"columns": [...], "data": [...]}}`), но у отдельных ресурсов
  `datashop` блок может называться по датасету (`tradestats`, `futoi`, ...).
  `IssTable::find`/`IssCursor::find` пробуют кандидатов по порядку
  (`&["data", "<dataset>"]`) — реальный ключ подтверждается только на живом
  ответе.
- **Регистр и имена колонок.** Доступ — по имени без учёта регистра
  (`RowView::f64/i64/str`). Фикстуры используют строчные имена
  (`secid`, `pr_close`, ...) по аналогии с нормализованными именами
  `moexalgo`; сырой ISS-ответ может отдавать `SECID`/`PR_CLOSE` заглавными —
  парсер это не сломает, но **набор колонок** (какие поля вообще есть) не
  проверен.
- **Время.** Строки `tradedate`/`tradetime` предполагаются раздельными
  (`YYYY-MM-DD`/`HH:MM:SS`), московское время без перевода часов (UTC+3).
  Конвертация в unix-секунды UTC — `parse::moex_datetime_to_unix`. Если
  живой ответ отдаёт единое поле (`ts`/`systime` как unix-время), парсер
  использует его в приоритете (`row_ts` сначала пробует `ts`/`systime`).
- **Пагинация.** Курсор `<блок>.cursor` с колонками `INDEX`/`TOTAL`/
  `PAGESIZE` — по общей конвенции ISS (`iss.meta=off` не убирает курсор,
  так как он не в `metadata`, а отдельный блок). Не подтверждено, что
  ALGOPACK-ресурсы вообще возвращают курсор при `iss.meta=off` — возможно,
  для одностраничных ответов блок `.cursor` отсутствует; `IssCursor::find`
  тогда просто возвращает `None`, и клиент не пагинирует (см.
  `client.rs::fetch_all`).
- **Параметры запроса.** `date`/`from`/`till`/`start`/`iss.meta=off` — по
  общей практике ISS REST; какие именно параметры принимает каждый ресурс
  `datashop/algopack/*` (и обязательны ли `from`/`till` для `tradestats` per
  тикеру) — не проверено.

## Файлы

| Файл | Датасет | Рынок | Особенность |
|---|---|---|---|
| `tradestats_eq.json` | tradestats (Super Candles) | eq | 2 строки, 1 страница |
| `futoi_fo.json` | futoi | fo (только `fo`) | fiz/yur + неизвестная группа (пропускается парсером) |
| `hi2_eq.json` | hi2 | eq | 2 строки |
| `obstats_eq.json` | obstats | eq | вторая строка без `spread_1mio` (`null`) — проверяет мягкий `Option` |
| `orderstats_eq.json` | orderstats | eq | вторая строка без `cancel_*` (`null`) — проверяет мягкий `Option` |
| `options_board.json` | опционная доска (фаза 12.4) | `engines/futures/markets/options` | см. раздел ниже |
| `options_board_empty.json` | опционная доска | — | пустые `securities`/`marketdata` (без строк) |

## Как перепроверить живым ключом

1. Получить `MOEX_ALGO_API` (подписка ALGOPACK на data.moex.com → APIKEY) и
   положить в `.env` (см. `.env.example`).
2. Выполнить вручную (или временным скриптом) запрос вида:
   ```
   curl -H "Authorization: Bearer $MOEX_ALGO_API" \
     "https://apim.moex.com/iss/datashop/algopack/eq/tradestats/SBER.json?iss.meta=off"
   ```
3. Сверить: имя блока-обёртки, регистр и полный список колонок, формат
   `tradedate`/`tradetime` (или наличие `ts`), наличие и форму блока
   `<блок>.cursor`, обязательность `from`/`till`.
4. Обновить фикстуры и, если понадобится, кандидатов блока в
   `client.rs`/`parse.rs` (сейчас `&["data", "<dataset>"]`) — тесты парсера
   написаны против формы файлов в этом каталоге и не изменятся в логике,
   только в данных.

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
