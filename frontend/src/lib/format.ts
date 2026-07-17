// Общие функции форматирования чисел для UI-компонентов. Раньше каждый
// компонент объявлял собственную локальную `fmt`/`fmtInt` — часто с
// одинаковым телом (см. историю PR #23) — сюда вынесены только те варианты,
// что были дословно идентичны в нескольких компонентах, либо параметризованы
// так, чтобы покрыть отличающиеся (но эквивалентные по смыслу) копии, не
// меняя видимое поведение ни одного из мест использования.

/**
 * `toFixed`-форматирование без разделителей разрядов. `null` → «∞»
 * (используется для «неограниченного» риска/прибыли в опционных выплатах,
 * см. `StrategyBuilder.svelte`). `decimals` по умолчанию — 2.
 */
export function fmtFixed(x: number | null, decimals = 2): string {
  return x == null ? "∞" : x.toFixed(decimals);
}

/**
 * Число в локали `ru-RU` (разряды через пробел) максимум с `decimals`
 * знаками после запятой — без выравнивания хвостовыми нулями.
 */
export function fmtRu(n: number, decimals = 2): string {
  return n.toLocaleString("ru-RU", { maximumFractionDigits: decimals });
}

/**
 * Число в локали `ru-RU` ровно с `decimals` знаками после запятой
 * (дополняется нулями). В отличие от {@link fmtRu} — фиксированная ширина,
 * удобно для табличных колонок.
 */
export function fmtRuFixed(n: number, decimals = 2): string {
  return Number(n).toLocaleString("ru-RU", {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });
}

/** Целое число в локали `ru-RU` (округление до целого, разряды через пробел). */
export function fmtInt(n: number): string {
  return Math.round(n).toLocaleString("ru-RU");
}
