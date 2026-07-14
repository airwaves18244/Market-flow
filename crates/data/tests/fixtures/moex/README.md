# Фикстуры MOEX ALGOPACK (ISS JSON)

**Статус: unverified — требуют сверки живым ключом `MOEX_ALGO_API`.**

Egress к `apim.moex.com` в этой среде закрыт, поэтому все файлы в этом
каталоге синтетические. Структура собрана по публичной документации ISS
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

## Опционная доска (`options.rs`, фаза 12.4) — `(unverified)`, отдельный контракт

В отличие от файлов выше (ALGOPACK, `apim.moex.com`, Bearer-токен), опционная
доска читается с **публичного** ISS `iss.moex.com` (не требует авторизации,
см. `S.3.1`: `apim.moex.com`/`iss.moex.com`/`data.moex.com` — три разных хоста
в egress-allowlist). Egress к `iss.moex.com` в этой среде тоже закрыт, поэтому
`options_board*.json` — синтетические фикстуры по общей документации ISS
(`iss.moex.com/iss/reference/`), со своим набором допущений:

- **Ресурс.** `engines/futures/markets/options/securities.json` (движок
  `futures`, рынок `options` — опционы MOEX торгуются на срочном рынке,
  привязаны к базовому фьючерсу). `?iss.only=securities,marketdata` — по
  общей практике ISS: карточки инструментов (`securities`) и рыночные данные
  (`marketdata`) в одном ответе, отдельными блоками.
- **Блоки.** `securities` — статические поля инструмента (`SECID`,
  `ASSETCODE` — код базового актива, `STRIKE`, `OPTIONTYPE` — `C`/`P` (без
  учёта регистра), `LASTTRADEDATE` — дата экспирации серии); `marketdata` —
  котировки (`BID`, `OFFER`, `LAST`, `IV`, `OPENPOSITION` — открытый интерес,
  `THEORPRICE` — теоретическая цена НКЦ). Точный набор колонок и их
  фактическое присутствие в реальном ответе не подтверждены.
- **Фильтрация по базовому активу.** Выполняется **на стороне клиента**
  (`parse_options_board` сравнивает `ASSETCODE` с искомым `underlying` без
  учёта регистра) — сервер может отдавать весь рынок опционов сразу; какие
  query-параметры ISS принимает для серверной фильтрации по инструменту/
  базовому активу, не проверено, а клиентская фильтрация работает независимо
  от этого.
- **Пагинация.** Курсор `securities.cursor` (`INDEX`/`TOTAL`/`PAGESIZE`, та
  же форма, что и у `IssCursor` из `parse.rs`) ведёт постраничный сбор;
  `marketdata` на каждой странице просто присоединяется без своего курсора
  (допущение: `marketdata` разбита на страницы синхронно с `securities`).
- **Форвард базового актива.** `MoexIss::underlying_forward` запрашивает
  `engines/futures/markets/forts/securities/{underlying}.json?iss.only=marketdata`
  и берёт `LAST`, иначе `SETTLEPRICE`, из первой строки `marketdata`. Это
  best-effort: недоступность (сеть/пустой ответ) не проваливает загрузку
  доски — `OptionsBoardSnapshot::forward` остаётся `None`, и вызывающая
  сторона (`app::api::option_board`) подставляет форвард из входа
  (`forward_hint`).
- **`options_board.json`.** Один базовый актив `RIH5` (4 валидных страйка:
  50000 call/put, 55000 call, 45000 call) + одна строка другого актива
  `SiH5` (проверяет фильтрацию) + одна строка без страйка (`null`, проверяет
  мягкое отбрасывание невалидных строк). Внутри `RIH5`: пут 50000 — без
  `bid`/`ask`/`OI` (неликвид, должен отбрасываться маппингом в точки
  улыбки); колл 55000 — без `IV`, но с `THEORPRICE` (проверяет вычисление
  IV через `domain::options::implied_vol`); остальные — с готовым `IV` в
  доске.
- **`options_board_empty.json`.** Пустые `securities`/`marketdata` — доска
  без данных не должна ронять парсер/маппинг.

### Как перепроверить живым доступом

1. Открыть без авторизации (публичный ресурс, ключ не нужен):
   ```
   curl "https://iss.moex.com/iss/engines/futures/markets/options/securities.json?iss.only=securities,marketdata&iss.meta=off"
   ```
2. Сверить: реальные имена блоков и колонок (регистр не важен — доступ по
   имени без учёта регистра), формат `OPTIONTYPE`/дат экспирации, наличие и
   форму курсора, синхронность страниц `securities`/`marketdata`, а также
   принимает ли ресурс серверную фильтрацию по базовому активу (тогда
   клиентскую фильтрацию можно оставить как страховку, а не единственный
   механизм).
3. Проверить форвард: `curl "https://iss.moex.com/iss/engines/futures/markets/forts/securities/<SECID>.json?iss.only=marketdata&iss.meta=off"`.
4. Обновить `options_board*.json` и, если понадобится, имена колонок/блоков
   в `options.rs` (`parse_options_board`/`MoexIss`) — тесты написаны против
   формы файлов в этом каталоге и не изменятся в логике, только в данных.
