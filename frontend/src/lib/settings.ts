// Пользовательские настройки терминала, сохраняемые в localStorage.
//
// Хранят параметры представления (глубина стакана, размер ленты сделок, лимит
// топ-движений), паспорт MOEX ALGO (рынки/вотчлист), конфигурацию LLM-резюме,
// историзации и опционов. Это чистый слой доступа: компоненты читают/пишут
// настройки через эти функции и не знают про конкретное хранилище.
//
// ВАЖНО: секреты (ключ провайдера LLM, токен ALGOPACK) здесь НЕ хранятся —
// только флаг наличия. Реальные секреты живут в ОС-keyring / .env на стороне
// Rust-ядра (`data::SecretStore`).

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
  llmModel: "anthropic/claude-3.5-sonnet",
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

/** Сохранить настройки. Ошибки хранилища (приватный режим) игнорируются. */
export function saveSettings(s: Settings): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(KEY, JSON.stringify(s));
  } catch {
    /* недоступно — не критично */
  }
}

/** Глубокая копия дефолтов (без мутации общего объекта вложенными правками). */
function structuredCopy(s: Settings): Settings {
  return { ...s, markets: { ...s.markets }, watchlist: { ...s.watchlist } };
}
