// Пользовательские настройки терминала, сохраняемые в localStorage.
//
// Хранят параметры представления (глубина стакана, размер ленты сделок, лимит
// топ-движений). Это чистый слой доступа: компоненты читают/пишут настройки
// через эти функции и не знают про конкретное хранилище.

export interface Settings {
  /** Сколько последних сделок показывать в ленте Time&Sales. */
  tapeLimit: number;
  /** Глубина стакана (число уровней с каждой стороны). */
  domDepth: number;
  /** Лимит строк в таблице топ-движений. */
  topMoversLimit: number;
}

export const DEFAULTS: Settings = {
  tapeLimit: 50,
  domDepth: 10,
  topMoversLimit: 10,
};

const KEY = "market-terminal:settings";

/** Загрузить настройки (с подстановкой значений по умолчанию). */
export function loadSettings(): Settings {
  if (typeof localStorage === "undefined") return { ...DEFAULTS };
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return { ...DEFAULTS };
    const parsed = JSON.parse(raw) as Partial<Settings>;
    return { ...DEFAULTS, ...parsed };
  } catch {
    return { ...DEFAULTS };
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
