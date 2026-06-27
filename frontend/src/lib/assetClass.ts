// Отображение классов активов: русские подписи и цвета для графиков Фазы 6.

export const ASSET_LABEL: Record<string, string> = {
  equity: "Акции",
  future: "Фьючерсы",
  bond: "Облигации",
};

export const ASSET_COLOR: Record<string, string> = {
  equity: "#4f9cf9",
  future: "#26a69a",
  bond: "#f5a623",
};

export function assetLabel(code: string): string {
  return ASSET_LABEL[code] ?? code;
}

export function assetColor(code: string): string {
  return ASSET_COLOR[code] ?? "#8b949e";
}
