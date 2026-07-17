// Пользовательские настройки терминала.
//
// Хранят параметры представления (глубина стакана, размер ленты сделок, лимит
// топ-движений), паспорт MOEX ALGO (рынки/вотчлист), конфигурацию LLM-резюме,
// историзации и опционов. Это чистый слой доступа: компоненты читают/пишут
// настройки через эти функции и не знают про конкретное хранилище.
//
// Персист (T3/10.5.3/S.2.2): под Tauri единый источник истины — JSON-файл в
// ядре (`app::settings::SettingsStore`), localStorage остаётся синхронным
// локальным кэшем (чтение быстрое, без await в каждом месте использования) и
// фолбэком браузерного режима (без Tauri — localStorage единственное
// хранилище, как раньше). `saveSettings()` пишет в кэш синхронно и (под
// Tauri) фоново отправляет в ядро; `syncSettingsWithCore()` — одноразовая
// миграция localStorage → ядро при первом запуске под Tauri плюс подтяжка
// кэша из ядра при каждом старте (вызывается один раз из `App.svelte`).
//
// ВАЖНО: секреты (ключ провайдера LLM, токен ALGOPACK) здесь НЕ хранятся —
// только флаг наличия. Реальные секреты живут в ОС-keyring / .env на стороне
// Rust-ядра (`data::SecretStore`).

import { ipc, inTauri } from "./ipc";
import type { SettingsDto } from "./types";

export type LlmProvider = "openrouter" | "anthropic" | "openai";
export type DataSourceId = "finam" | "moex_algo";
export type PricingModelId = "black76" | "bachelier";
export type SmileModelId = "moex" | "sabr" | "svi" | "kalen";

export interface Settings {
  /** Сколько последних сделок показывать в ленте Time&Sales. */
  tapeLimit: number;
  /** Глубина стакана (число уровней с каждой стороны). */
  domDepth: number;
  /** Лимит строк в таблице топ-движений. */
  topMoversLimit: number;

  // ── Паспорт MOEX ALGO ──────────────────────────────────────────────────
  /** Активные рынки ALGOPACK (акции/фьючерсы/валюта). */
  markets: { eq: boolean; fo: boolean; fx: boolean };
  /** Вотчлист ALGOPACK: тикер → включён. */
  watchlist: Record<string, boolean>;

  // ── LLM · ИИ-резюме ────────────────────────────────────────────────────
  llmProvider: LlmProvider;
  llmModel: string;
  /** Признак того, что ключ провайдера задан (сам ключ не хранится в UI). */
  llmHasKey: boolean;
  llmTokenLimit: number;
  /** Авто-обновление резюме при смене периода/инструмента. */
  llmAuto: boolean;
  /** Период анализа по умолчанию. */
  defaultPeriod: "1h" | "1d" | "1w" | "1m" | "3m";

  // ── Данные / Историзация ───────────────────────────────────────────────
  dataSource: DataSourceId;
  dataDir: string;
  concurrency: number;

  // ── Опционы ────────────────────────────────────────────────────────────
  pricingModel: PricingModelId;
  rate: number;
  defaultSmile: SmileModelId;
}

export const DEFAULTS: Settings = {
  tapeLimit: 50,
  domDepth: 10,
  topMoversLimit: 10,

  markets: { eq: true, fo: true, fx: false },
  watchlist: { SBER: true, GAZP: true, LKOH: true, GMKN: false, ROSN: true, VTBR: false },

  llmProvider: "openrouter",
  llmModel: "anthropic/claude-sonnet-5",
  llmHasKey: false,
  llmTokenLimit: 2000,
  llmAuto: true,
  defaultPeriod: "1h",

  dataSource: "finam",
  dataDir: "~/.market-terminal/history",
  concurrency: 4,

  pricingModel: "black76",
  rate: 0,
  defaultSmile: "moex",
};

const KEY = "market-terminal:settings";
/** Метка одноразовой миграции localStorage → ядро (см. `syncSettingsWithCore`). */
const MIGRATED_KEY = "market-terminal:settings-core-migrated";

/** Загрузить настройки (с подстановкой значений по умолчанию, в т.ч. для
 * вложенных объектов — старые записи localStorage могут не иметь новых полей). */
export function loadSettings(): Settings {
  if (typeof localStorage === "undefined") return structuredCopy(DEFAULTS);
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return structuredCopy(DEFAULTS);
    const parsed = JSON.parse(raw) as Partial<Settings>;
    return {
      ...DEFAULTS,
      ...parsed,
      markets: { ...DEFAULTS.markets, ...(parsed.markets ?? {}) },
      watchlist: { ...DEFAULTS.watchlist, ...(parsed.watchlist ?? {}) },
    };
  } catch {
    return structuredCopy(DEFAULTS);
  }
}

/** Записать в локальный кэш (localStorage), не трогая ядро. Ошибки хранилища
 * (приватный режим браузера) игнорируются. */
function writeLocalCache(s: Settings): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(KEY, JSON.stringify(s));
  } catch {
    /* недоступно — не критично */
  }
}

/** Сохранить настройки: локальный кэш обновляется всегда синхронно (как
 * раньше — вызывающий код не становится асинхронным), плюс под Tauri —
 * фоновая асинхронная запись в ядро (единый источник истины, 10.5.3/S.2.2).
 * Ошибки ядра (недоступно/оффлайн) не блокируют локальное сохранение. */
export function saveSettings(s: Settings): void {
  writeLocalCache(s);
  if (inTauri()) {
    ipc.settingsSet(toDto(s)).catch(() => {
      /* ядро недоступно — локальный кэш остаётся источником до следующей попытки */
    });
  }
}

/** Глубокая копия дефолтов (без мутации общего объекта вложенными правками). */
function structuredCopy(s: Settings): Settings {
  return { ...s, markets: { ...s.markets }, watchlist: { ...s.watchlist } };
}

// ── T3 — Персист в ядро: конвертация DTO ↔ Settings, миграция ────────────────

/** `Settings` → DTO ядра. Поля 1:1 (DTO лишь ослабляет литеральные union до
 * `string`), поэтому конвертация — плоское копирование вложенных объектов. */
function toDto(s: Settings): SettingsDto {
  return { ...s, markets: { ...s.markets }, watchlist: { ...s.watchlist } };
}

function isOneOf<T extends string>(v: string, opts: readonly T[]): v is T {
  return (opts as readonly string[]).includes(v);
}

const LLM_PROVIDERS = ["openrouter", "anthropic", "openai"] as const;
const PERIODS = ["1h", "1d", "1w", "1m", "3m"] as const;
const DATA_SOURCES = ["finam", "moex_algo"] as const;
const PRICING_MODELS = ["black76", "bachelier"] as const;
const SMILE_MODELS = ["moex", "sabr", "svi", "kalen"] as const;

/** DTO ядра → `Settings`, подставляя дефолты для значений, не входящих в
 * известные перечисления (защита от ручной правки файла настроек в ядре). */
function fromDto(d: SettingsDto): Settings {
  return {
    ...DEFAULTS,
    ...d,
    markets: { ...DEFAULTS.markets, ...d.markets },
    watchlist: { ...DEFAULTS.watchlist, ...d.watchlist },
    llmProvider: isOneOf(d.llmProvider, LLM_PROVIDERS) ? d.llmProvider : DEFAULTS.llmProvider,
    defaultPeriod: isOneOf(d.defaultPeriod, PERIODS) ? d.defaultPeriod : DEFAULTS.defaultPeriod,
    dataSource: isOneOf(d.dataSource, DATA_SOURCES) ? d.dataSource : DEFAULTS.dataSource,
    pricingModel: isOneOf(d.pricingModel, PRICING_MODELS)
      ? d.pricingModel
      : DEFAULTS.pricingModel,
    defaultSmile: isOneOf(d.defaultSmile, SMILE_MODELS) ? d.defaultSmile : DEFAULTS.defaultSmile,
  };
}

/** Одноразовая миграция localStorage → ядро при первом запуске под Tauri, а
 * затем (при каждом старте) — подтяжка локального кэша из ядра, единого
 * источника истины (10.5.3/S.2.2). В браузере без Tauri — no-op, localStorage
 * остаётся единственным хранилищем, как раньше. Вызывается один раз при
 * старте приложения (см. `App.svelte`); ошибки (ядро недоступно) — не
 * критичны, работа продолжается на локальном кэше. */
export async function syncSettingsWithCore(): Promise<void> {
  if (!inTauri()) return;
  try {
    const alreadyMigrated =
      typeof localStorage !== "undefined" && localStorage.getItem(MIGRATED_KEY) === "1";
    if (!alreadyMigrated) {
      const hadLocalState =
        typeof localStorage !== "undefined" && localStorage.getItem(KEY) !== null;
      if (hadLocalState) {
        await ipc.settingsSet(toDto(loadSettings()));
      }
      if (typeof localStorage !== "undefined") localStorage.setItem(MIGRATED_KEY, "1");
    }
    const core = await ipc.settingsGet();
    writeLocalCache(fromDto(core));
  } catch {
    /* ядро недоступно — остаёмся на локальном кэше */
  }
}
