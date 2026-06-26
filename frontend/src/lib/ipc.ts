// Типизированный клиент IPC к Rust-ядру.
//
// В среде Tauri вызывает реальные команды через `@tauri-apps/api`; в обычном
// браузере (разработка/сборка без бэкенда) — отдаёт мок-данные. Аргументы
// именуются camelCase: Tauri преобразует их в snake_case параметры команд.

import type {
  BarPoint,
  BreadthDto,
  InstrumentDto,
  RrgSectorDto,
  SectorEntryDto,
  SectorRow,
  TimeFrame,
  TopMoverDto,
  TurnoverPoint,
} from "./types";
import * as mock from "./mock";

function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri()) {
    return mock.handle<T>(cmd, args);
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export const ipc = {
  instruments: () => invoke<InstrumentDto[]>("instruments"),

  bars: (symbol: string, timeframe: TimeFrame, fromTs: number, toTs: number) =>
    invoke<BarPoint[]>("bars", { symbol, timeframe, fromTs, toTs }),

  turnoverSeries: (symbol: string, fromTs: number, toTs: number) =>
    invoke<TurnoverPoint[]>("turnover_series", { symbol, fromTs, toTs }),

  sectorRollup: (fromTs: number, toTs: number) =>
    invoke<SectorRow[]>("sector_rollup", { fromTs, toTs }),

  sectorMap: () => invoke<SectorEntryDto[]>("sector_map"),

  breadthData: (fromTs: number, toTs: number) =>
    invoke<BreadthDto>("breadth_data", { fromTs, toTs }),

  topMovers: (fromTs: number, toTs: number, limit?: number) =>
    invoke<TopMoverDto[]>("top_movers", { fromTs, toTs, limit }),

  rrgSectors: (fromTs: number, toTs: number) =>
    invoke<RrgSectorDto[]>("rrg_sectors", { fromTs, toTs }),
};
